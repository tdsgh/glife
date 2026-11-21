//! The simulation thread's state machine. All logic lives in `Sim::handle` and
//! `Sim::tick`; `run` is a thin loop around them. Time is a `Duration` since
//! the thread started, so tests drive it with a fake clock.

use std::collections::VecDeque;
use std::hash::{BuildHasher, Hasher};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use glife_engine::{Axis, Edges, PasteMode, Pattern, Rect, Rule, Tiling, Universe};

use crate::history::{Cells, Entry, History};

/// While running, a snapshot is published at most this often.
const PUBLISH_INTERVAL: Duration = Duration::from_millis(8);
/// Window over which achieved generations per second are counted.
const RATE_WINDOW: Duration = Duration::from_secs(1);

/// An immutable copy of the universe and the run state, as of one moment.
pub struct Snapshot {
    pub version: u64,
    pub universe: Universe,
    pub playing: bool,
    pub gens_per_sec: f32,
    pub revision: u64,
    /// The universe differs from the one last saved or opened.
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

/// The latest snapshot, swapped under a mutex.
pub type Latest = Arc<Mutex<Arc<Snapshot>>>;

pub enum Command {
    /// Ignored when the size or the rule does not suit the tiling; the
    /// command validates both first.
    NewUniverse { width: u32, height: u32, edges: Edges, tiling: Tiling, rule: Rule },
    Play,
    Pause,
    Step,
    /// `None` means as fast as possible.
    SetSpeed(Option<f64>),
    SetEdges(Edges),
    SetRule(Rule),
    /// Batches with the same `stroke` and no other command between them are
    /// one undo step.
    Edit { cells: Vec<(u32, u32)>, alive: bool, stroke: Option<u64> },
    /// `x`, `y` is the pattern's top-left cell and may lie outside the grid.
    Place { pattern: Arc<Pattern>, x: i64, y: i64, mode: PasteMode },
    KillRect(Rect),
    FlipRect(Rect, Axis),
    Clear,
    /// `density` is a fraction, 0.0 to 1.0.
    RandomFill { density: f64, seed: u64 },
    Undo,
    Redo,
    /// Replaces the universe with an opened one, paused.
    Load(Universe),
    /// The universe at `revision` was saved.
    MarkSaved { revision: u64 },
}

/// Whether `rect` is non-empty and lies inside the grid.
pub fn inside(rect: Rect, universe: &Universe) -> bool {
    rect.w > 0
        && rect.h > 0
        && rect.x as u64 + rect.w as u64 <= universe.width() as u64
        && rect.y as u64 + rect.h as u64 <= universe.height() as u64
}

pub struct Sim {
    universe: Universe,
    playing: bool,
    /// Generations per second; `None` is as fast as possible.
    speed: Option<f64>,
    /// When the next generation is due while playing at a set speed.
    deadline: Duration,
    /// Times of the generations inside the sliding rate window.
    steps: VecDeque<Duration>,
    version: u64,
    last_publish: Duration,
    /// The universe changed since the last publish.
    unpublished: bool,
    history: History,
    /// The universe's revision. Every change takes a new one from
    /// `last_revision`; undo and redo restore an earlier one.
    revision: u64,
    /// The highest revision given out; it only goes up.
    last_revision: u64,
    /// The revision last saved or opened; 0 when none matches the universe.
    saved_revision: u64,
    latest: Latest,
}

impl Sim {
    /// A 256 x 256 wrapping square universe, paused; with `bench`, a
    /// 2048 x 2048 wrapping universe of that grid and rule filled to 25 % and
    /// playing as fast as possible.
    pub fn launch(bench: Option<(Tiling, Rule)>) -> Sim {
        let Some((tiling, rule)) = bench else {
            return Sim::new(Universe::new(256, 256, Edges::Wrap, Tiling::Square).expect("valid size"), Duration::ZERO);
        };
        let mut universe = Universe::new(2048, 2048, Edges::Wrap, tiling).expect("valid size");
        universe.set_rule(rule).expect("rule suits the grid");
        universe.random_fill(0.25, os_seed());
        let mut sim = Sim::new(universe, Duration::ZERO);
        sim.handle(Command::SetSpeed(None), Duration::ZERO);
        sim.handle(Command::Play, Duration::ZERO);
        sim
    }

    pub fn new(universe: Universe, now: Duration) -> Sim {
        let snapshot = Arc::new(Snapshot {
            version: 1,
            universe: universe.clone(),
            playing: false,
            gens_per_sec: 0.0,
            revision: 1,
            dirty: false,
            can_undo: false,
            can_redo: false,
        });
        Sim {
            universe,
            playing: false,
            speed: Some(10.0),
            deadline: now,
            steps: VecDeque::new(),
            version: 1,
            last_publish: now,
            unpublished: false,
            history: History::default(),
            revision: 1,
            last_revision: 1,
            saved_revision: 1,
            latest: Arc::new(Mutex::new(snapshot)),
        }
    }

    /// The slot the latest snapshot is published to.
    pub fn latest(&self) -> Latest {
        self.latest.clone()
    }

    #[cfg(test)]
    fn snapshot(&self) -> Arc<Snapshot> {
        self.latest.lock().unwrap().clone()
    }

    /// Applies one command between generations, then publishes. Commands
    /// that come from the UI are validated by the caller; one that does not
    /// fit the current grid changes nothing.
    pub fn handle(&mut self, command: Command, now: Duration) {
        let open_stroke = self.history.stroke();
        if !matches!(command, Command::Edit { stroke: Some(id), .. } if Some(id) == open_stroke) {
            self.history.end_stroke();
        }
        match command {
            Command::NewUniverse { width, height, edges, tiling, rule } => {
                if let Ok(mut universe) = Universe::new(width, height, edges, tiling) {
                    if universe.set_rule(rule).is_ok() {
                        self.replace(universe, now);
                    }
                }
            }
            Command::Load(universe) => self.replace(universe, now),
            Command::Play => self.set_playing(true, now),
            Command::Pause => self.set_playing(false, now),
            Command::Step => {
                if !self.playing {
                    self.universe.step();
                    self.advanced();
                }
            }
            Command::SetSpeed(speed) => {
                self.speed = speed;
                self.deadline = now;
            }
            Command::SetEdges(edges) => {
                if edges != self.universe.edges() && self.universe.set_edges(edges).is_ok() {
                    self.new_revision();
                    // Undo cannot take the edge mode back, so no revision in
                    // the history matches the saved universe any more.
                    self.saved_revision = 0;
                }
            }
            Command::SetRule(rule) => {
                if rule != self.universe.rule() && self.universe.set_rule(rule).is_ok() {
                    self.new_revision();
                    // As with the edge mode, undo cannot take it back.
                    self.saved_revision = 0;
                }
            }
            Command::Edit { cells, alive, stroke } => self.edit(cells, alive, stroke),
            Command::Place { pattern, x, y, mode } => {
                // `x`, `y` is the top-left cell before the pattern is lined up with the grid.
                let (stamp, x, y) = pattern.placement(x + (pattern.width() / 2) as i64, y + (pattern.height() / 2) as i64);
                let (w, h) = (stamp.width(), stamp.height());
                if pattern.tiling() == self.universe.tiling() && w <= self.universe.width() && h <= self.universe.height() {
                    let rects = self.universe.footprint(w, h, x, y);
                    if !rects.is_empty() {
                        self.change(&rects, |u| {
                            u.place(&stamp, x, y, mode).expect("pattern fits the grid")
                        });
                    }
                }
            }
            Command::KillRect(rect) => {
                if inside(rect, &self.universe) {
                    self.change(&[rect], |u| u.kill_rect(rect));
                }
            }
            Command::FlipRect(rect, axis) => {
                if inside(rect, &self.universe) {
                    let rects = self.universe.flip_footprint(rect, axis);
                    self.change(&rects, |u| u.flip_rect(rect, axis));
                }
            }
            Command::Clear => self.change(&[self.whole_grid()], Universe::clear),
            Command::RandomFill { density, seed } => {
                self.change(&[self.whole_grid()], |u| u.random_fill(density, seed));
            }
            Command::Undo => {
                if !self.playing {
                    if let Some(entry) = self.history.undo() {
                        entry.before.restore(&mut self.universe);
                        self.universe.set_generation(entry.generation[0]);
                        self.revision = entry.revision[0];
                    }
                }
            }
            Command::Redo => {
                if !self.playing {
                    if let Some(entry) = self.history.redo() {
                        entry.after.restore(&mut self.universe);
                        self.universe.set_generation(entry.generation[1]);
                        self.revision = entry.revision[1];
                    }
                }
            }
            Command::MarkSaved { revision } => self.saved_revision = revision,
        }
        self.publish(now);
    }

    /// Sets or kills `cells`. While paused the batch is recorded, joining
    /// the open stroke when it carries that stroke's id.
    fn edit(&mut self, cells: Vec<(u32, u32)>, alive: bool, stroke: Option<u64>) {
        let (width, height) = (self.universe.width(), self.universe.height());
        let cells: Vec<(u32, u32)> = cells.into_iter().filter(|&(x, y)| x < width && y < height).collect();
        if cells.is_empty() {
            return;
        }
        let capture = |u: &Universe| cells.iter().map(|&(x, y)| u.capture_cell(x, y)).collect::<Vec<_>>();
        let before = (!self.playing).then(|| capture(&self.universe));
        let revision = self.revision;
        for &(x, y) in &cells {
            self.universe.set(x, y, alive);
        }
        self.new_revision();
        match before {
            Some(before) => {
                let generation = self.universe.generation();
                let after = capture(&self.universe);
                self.history.record_cells(
                    stroke,
                    &cells,
                    before,
                    after,
                    [generation, generation],
                    [revision, self.revision],
                );
            }
            None => self.history.clear(),
        }
    }

    /// Applies a change confined to `rects`, which must not overlap. While
    /// paused it is recorded as one undo step; while playing it is not, and
    /// the history is dropped, since its steps no longer lead back.
    fn change(&mut self, rects: &[Rect], apply: impl FnOnce(&mut Universe)) {
        let before = (!self.playing).then(|| self.universe.capture(rects));
        let (generation, revision) = (self.universe.generation(), self.revision);
        apply(&mut self.universe);
        self.new_revision();
        match before {
            Some(before) => self.history.push(Entry {
                before: Cells::Rects(before),
                after: Cells::Rects(self.universe.capture(rects)),
                generation: [generation, self.universe.generation()],
                revision: [revision, self.revision],
            }),
            None => self.history.clear(),
        }
    }

    fn whole_grid(&self) -> Rect {
        Rect { x: 0, y: 0, w: self.universe.width(), h: self.universe.height() }
    }

    /// A new or opened universe: paused, with an empty history, and saved as
    /// it stands.
    fn replace(&mut self, universe: Universe, now: Duration) {
        self.universe = universe;
        self.set_playing(false, now);
        self.history.clear();
        self.new_revision();
        self.saved_revision = self.revision;
    }

    /// After a generation: the history no longer applies.
    fn advanced(&mut self) {
        self.history.clear();
        self.new_revision();
    }

    fn new_revision(&mut self) {
        self.last_revision += 1;
        self.revision = self.last_revision;
    }

    /// Runs at most one generation if one is due. Returns when to call again,
    /// or `None` when there is nothing to wait for but a command.
    pub fn tick(&mut self, now: Duration) -> Option<Duration> {
        if self.playing {
            match self.speed {
                None => self.generation(now),
                Some(rate) if now >= self.deadline => {
                    let interval = Duration::from_secs_f64(1.0 / rate);
                    self.generation(now);
                    // More than one interval behind: do not catch up in a burst.
                    self.deadline = if now - self.deadline > interval {
                        now + interval
                    } else {
                        self.deadline + interval
                    };
                }
                Some(_) => {}
            }
        }
        if self.unpublished && now >= self.last_publish + PUBLISH_INTERVAL {
            self.publish(now);
        }
        if !self.playing {
            return None;
        }
        let publish_due = self.unpublished.then(|| self.last_publish + PUBLISH_INTERVAL);
        let generation_due = self.speed.map_or(now, |_| self.deadline);
        Some(publish_due.map_or(generation_due, |p| p.min(generation_due)))
    }

    fn generation(&mut self, now: Duration) {
        self.universe.step();
        self.advanced();
        self.steps.push_back(now);
        self.unpublished = true;
    }

    fn set_playing(&mut self, playing: bool, now: Duration) {
        self.playing = playing;
        self.deadline = now;
        self.steps.clear();
    }

    fn publish(&mut self, now: Duration) {
        while self.steps.front().is_some_and(|&t| now.saturating_sub(t) >= RATE_WINDOW) {
            self.steps.pop_front();
        }
        self.version += 1;
        let gens_per_sec = self.steps.len() as f32;
        let dirty = self.revision != self.saved_revision;
        let can_undo = !self.playing && self.history.can_undo();
        let can_redo = !self.playing && self.history.can_redo();
        let mut latest = self.latest.lock().unwrap();
        // Reuse the previous snapshot's buffers unless a reader still holds it.
        if let Some(snapshot) = Arc::get_mut(&mut latest) {
            snapshot.version = self.version;
            snapshot.universe.clone_from(&self.universe);
            snapshot.playing = self.playing;
            snapshot.gens_per_sec = gens_per_sec;
            snapshot.revision = self.revision;
            snapshot.dirty = dirty;
            snapshot.can_undo = can_undo;
            snapshot.can_redo = can_redo;
        } else {
            *latest = Arc::new(Snapshot {
                version: self.version,
                universe: self.universe.clone(),
                playing: self.playing,
                gens_per_sec,
                revision: self.revision,
                dirty,
                can_undo,
                can_redo,
            });
        }
        drop(latest);
        self.last_publish = now;
        self.unpublished = false;
    }
}

/// Reads `GLIFE_BENCH`: unset or empty is no benchmark, `1` the square grid
/// under B3/S23, and `<tiling id>[:<rule>]` another grid, under its default
/// rule when none is given.
pub fn parse_bench(value: Option<&str>) -> Result<Option<(Tiling, Rule)>, String> {
    let value = match value {
        None | Some("") => return Ok(None),
        Some("1") => return Ok(Some((Tiling::Square, Tiling::Square.default_rule()))),
        Some(value) => value,
    };
    let (id, rule) = match value.split_once(':') {
        Some((id, rule)) => (id, Some(rule)),
        None => (value, None),
    };
    let tiling = Tiling::parse(id).ok_or_else(|| format!("GLIFE_BENCH: unknown grid {id:?}"))?;
    let rule = match rule {
        Some(text) => Rule::parse(text, tiling).map_err(|e| format!("GLIFE_BENCH: {e}"))?,
        None => tiling.default_rule(),
    };
    Ok(Some((tiling, rule)))
}

/// Reads `GLIFE_BENCH_REPORT`: unset, empty or `1`; `1` is valid only with a
/// `GLIFE_BENCH` benchmark.
pub fn parse_bench_report(value: Option<&str>, bench: bool) -> Result<bool, String> {
    match value {
        None | Some("") => Ok(false),
        Some("1") if bench => Ok(true),
        Some("1") => Err("GLIFE_BENCH_REPORT=1 needs GLIFE_BENCH".into()),
        Some(other) => Err(format!("GLIFE_BENCH_REPORT: expected 1, got {other:?}")),
    }
}

/// A seed from the OS: `RandomState` is keyed from OS randomness.
pub fn os_seed() -> u64 {
    std::collections::hash_map::RandomState::new().build_hasher().finish()
}

/// The simulation thread body: handles commands as they arrive and ticks at
/// the wake-up times `Sim::tick` asks for. Returns when the queue closes.
pub fn run(mut sim: Sim, queue: Receiver<Command>) {
    let start = Instant::now();
    loop {
        let now = start.elapsed();
        let received = match sim.tick(now) {
            Some(wake) => queue.recv_timeout(wake.saturating_sub(now)),
            None => queue.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match received {
            Ok(command) => sim.handle(command, start.elapsed()),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    fn sim() -> Sim {
        Sim::new(Universe::new(16, 16, Edges::Wrap, Tiling::Square).unwrap(), Duration::ZERO)
    }

    fn blinker(sim: &mut Sim) {
        let cells = vec![(5, 5), (6, 5), (7, 5)];
        sim.handle(Command::Edit { cells, alive: true, stroke: None }, Duration::ZERO);
    }

    #[test]
    fn speed_10_gives_10_generations_per_simulated_second() {
        let mut sim = sim();
        sim.handle(Command::SetSpeed(Some(10.0)), Duration::ZERO);
        sim.handle(Command::Play, Duration::ZERO);
        let mut now = Duration::ZERO;
        while now < Duration::from_secs(1) {
            sim.tick(now);
            now += MS(1);
        }
        // Deadlines at 0, 100, ..., 900 ms.
        assert_eq!(sim.universe.generation(), 10);
        sim.tick(MS(1000));
        assert_eq!(sim.universe.generation(), 11);
    }

    #[test]
    fn falling_behind_does_not_burst() {
        let mut sim = sim();
        sim.handle(Command::SetSpeed(Some(10.0)), Duration::ZERO);
        sim.handle(Command::Play, Duration::ZERO);
        sim.tick(Duration::ZERO);
        assert_eq!(sim.universe.generation(), 1);
        // Ten intervals late: one generation, then a fresh deadline.
        let wake = sim.tick(Duration::from_secs(1));
        assert_eq!(sim.universe.generation(), 2);
        assert_eq!(wake, Some(MS(1100)));
        sim.tick(MS(1000));
        assert_eq!(sim.universe.generation(), 2);
        sim.tick(MS(1100));
        assert_eq!(sim.universe.generation(), 3);
    }

    #[test]
    fn as_fast_as_possible_steps_on_every_tick() {
        let mut sim = sim();
        sim.handle(Command::SetSpeed(None), Duration::ZERO);
        sim.handle(Command::Play, Duration::ZERO);
        for _ in 0..50 {
            sim.tick(Duration::ZERO);
        }
        assert_eq!(sim.universe.generation(), 50);
    }

    #[test]
    fn edits_between_ticks_apply_in_order() {
        let mut sim = sim();
        sim.handle(Command::Edit { cells: vec![(1, 1), (2, 2)], alive: true, stroke: None }, Duration::ZERO);
        sim.handle(Command::Edit { cells: vec![(1, 1)], alive: false, stroke: None }, Duration::ZERO);
        sim.handle(Command::Edit { cells: vec![(1, 1)], alive: true, stroke: None }, Duration::ZERO);
        sim.handle(Command::Edit { cells: vec![(2, 2)], alive: false, stroke: None }, Duration::ZERO);
        assert!(sim.universe.get(1, 1));
        assert!(!sim.universe.get(2, 2));
        let snapshot = sim.snapshot();
        assert!(snapshot.universe.get(1, 1) && !snapshot.universe.get(2, 2));
    }

    #[test]
    fn step_is_ignored_while_playing() {
        let mut sim = sim();
        sim.handle(Command::Step, Duration::ZERO);
        assert_eq!(sim.universe.generation(), 1);
        sim.handle(Command::Play, Duration::ZERO);
        sim.handle(Command::Step, Duration::ZERO);
        assert_eq!(sim.universe.generation(), 1);
        sim.handle(Command::Pause, Duration::ZERO);
        sim.handle(Command::Step, Duration::ZERO);
        assert_eq!(sim.universe.generation(), 2);
    }

    #[test]
    fn fill_and_clear_reset_the_generation() {
        let mut sim = sim();
        sim.handle(Command::Step, Duration::ZERO);
        sim.handle(Command::Step, Duration::ZERO);
        sim.handle(Command::RandomFill { density: 0.5, seed: 7 }, Duration::ZERO);
        assert_eq!(sim.universe.generation(), 0);
        assert!(sim.universe.live_count() > 0);
        sim.handle(Command::Step, Duration::ZERO);
        sim.handle(Command::Clear, Duration::ZERO);
        assert_eq!(sim.universe.generation(), 0);
        assert_eq!(sim.universe.live_count(), 0);
    }

    #[test]
    fn set_edges_applies_from_the_next_generation() {
        // A vertical blinker across the top seam: it survives with wrapping
        // edges and dies with dead edges.
        let cells = vec![(5, 15), (5, 0), (5, 1)];
        let mut wrapped = sim();
        wrapped.handle(Command::Edit { cells: cells.clone(), alive: true, stroke: None }, Duration::ZERO);
        wrapped.handle(Command::Step, Duration::ZERO);
        assert_eq!(wrapped.universe.live_count(), 3);

        let mut dead = sim();
        dead.handle(Command::Edit { cells, alive: true, stroke: None }, Duration::ZERO);
        dead.handle(Command::SetEdges(Edges::Dead), Duration::ZERO);
        assert_eq!(dead.universe.live_count(), 3);
        assert_eq!(dead.universe.generation(), 0);
        dead.handle(Command::Step, Duration::ZERO);
        assert_eq!(dead.universe.live_count(), 0);
    }

    #[test]
    fn snapshots_publish_per_command_and_at_most_every_8_ms_while_running() {
        let mut sim = sim();
        blinker(&mut sim);
        let v = sim.snapshot().version;
        sim.handle(Command::SetSpeed(None), Duration::ZERO);
        sim.handle(Command::Play, Duration::ZERO);
        assert_eq!(sim.snapshot().version, v + 2);
        sim.tick(MS(1));
        sim.tick(MS(5));
        assert_eq!(sim.snapshot().version, v + 2);
        assert_eq!(sim.snapshot().universe.generation(), 0);
        sim.tick(MS(8));
        assert_eq!(sim.snapshot().version, v + 3);
        assert!(sim.snapshot().playing);
        assert_eq!(sim.snapshot().universe.generation(), 3);
    }

    #[test]
    fn achieved_rate_counts_a_sliding_window() {
        let mut sim = sim();
        sim.handle(Command::SetSpeed(Some(10.0)), Duration::ZERO);
        sim.handle(Command::Play, Duration::ZERO);
        let mut now = Duration::ZERO;
        while now <= MS(2500) {
            sim.tick(now);
            now += MS(1);
        }
        sim.handle(Command::Edit { cells: vec![], alive: true, stroke: None }, MS(2500));
        assert_eq!(sim.snapshot().gens_per_sec, 10.0);
        sim.handle(Command::Pause, MS(2500));
        assert_eq!(sim.snapshot().gens_per_sec, 0.0);
    }

    #[test]
    fn new_universe_is_empty_paused_and_replaces_the_old_one() {
        let mut sim = sim();
        sim.handle(Command::Play, Duration::ZERO);
        sim.handle(Command::NewUniverse { width: 40, height: 30, edges: Edges::Dead, tiling: Tiling::Square, rule: Tiling::Square.default_rule() }, Duration::ZERO);
        let s = sim.snapshot();
        assert!(!s.playing);
        assert_eq!((s.universe.width(), s.universe.height()), (40, 30));
        assert_eq!(s.universe.edges(), Edges::Dead);
        assert_eq!(s.universe.generation(), 0);
    }

    #[test]
    fn publish_reuses_the_snapshot_only_when_no_reader_holds_it() {
        let mut sim = sim();
        blinker(&mut sim);
        let ptr = Arc::as_ptr(&sim.snapshot());
        // The temporary above is dropped, so the buffers are reused.
        sim.handle(Command::Step, Duration::ZERO);
        let held = sim.snapshot();
        assert_eq!(Arc::as_ptr(&held), ptr);
        assert_eq!(held.universe.generation(), 1);
        // A reader holds the snapshot: it stays intact and a new one is made.
        sim.handle(Command::Step, Duration::ZERO);
        assert_ne!(Arc::as_ptr(&sim.snapshot()), ptr);
        assert_eq!(held.universe.generation(), 1);
        assert_eq!(sim.snapshot().universe.generation(), 2);
    }

    const ZERO: Duration = Duration::ZERO;

    fn paint(sim: &mut Sim, cells: &[(u32, u32)], stroke: Option<u64>) {
        sim.handle(Command::Edit { cells: cells.to_vec(), alive: true, stroke }, ZERO);
    }

    /// Every cell's alive state, and its age when alive, row by row. A dead
    /// cell's age only colors its trail, which undo does not bring back.
    fn state(u: &Universe) -> Vec<(bool, u16)> {
        (0..u.height())
            .flat_map(|y| (0..u.width()).map(move |x| (u.get(x, y), if u.get(x, y) { u.age(x, y) } else { 0 })))
            .collect()
    }

    fn no_trail(u: &Universe) -> bool {
        (0..u.height()).all(|y| (0..u.width()).all(|x| u.trail(x, y) == 0))
    }

    const BLOCK: [(u32, u32); 4] = [(1, 1), (2, 1), (1, 2), (2, 2)];

    #[test]
    fn a_stroke_is_one_undo_step() {
        let mut sim = sim();
        paint(&mut sim, &[(1, 5)], Some(1));
        paint(&mut sim, &[(2, 5), (3, 5)], Some(1));
        paint(&mut sim, &[(4, 5), (2, 5)], Some(1));
        assert_eq!(sim.history.len(), (1, 0));
        sim.handle(Command::Undo, ZERO);
        assert_eq!(sim.universe.live_count(), 0);
        sim.handle(Command::Redo, ZERO);
        assert_eq!(sim.universe.live_count(), 4);

        // A new stroke id starts a new step.
        paint(&mut sim, &[(1, 8)], Some(2));
        assert_eq!(sim.history.len(), (2, 0));
        // So does any command between two batches of one stroke.
        sim.handle(Command::SetSpeed(Some(5.0)), ZERO);
        paint(&mut sim, &[(2, 8)], Some(2));
        assert_eq!(sim.history.len(), (3, 0));
        sim.handle(Command::Undo, ZERO);
        assert!(sim.universe.get(1, 8) && !sim.universe.get(2, 8));
        // A batch without a stroke id is a step of its own.
        paint(&mut sim, &[(3, 8)], None);
        paint(&mut sim, &[(4, 8)], None);
        assert_eq!(sim.history.len(), (4, 0));
    }

    #[test]
    fn a_stroke_restores_each_cell_as_it_was_before_the_stroke_touched_it() {
        let mut sim = sim();
        paint(&mut sim, &BLOCK, None);
        for _ in 0..3 {
            sim.handle(Command::Step, ZERO);
        }
        assert_eq!(sim.universe.age(1, 1), 3);
        // One stroke kills a block cell, then sets it alive again at age 0.
        sim.handle(Command::Edit { cells: vec![(1, 1)], alive: false, stroke: Some(7) }, ZERO);
        paint(&mut sim, &[(1, 1), (6, 6)], Some(7));
        assert_eq!((sim.universe.get(1, 1), sim.universe.age(1, 1)), (true, 0));
        sim.handle(Command::Undo, ZERO);
        assert_eq!((sim.universe.get(1, 1), sim.universe.age(1, 1)), (true, 3));
        assert!(!sim.universe.get(6, 6));
        sim.handle(Command::Redo, ZERO);
        assert_eq!((sim.universe.age(1, 1), sim.universe.get(6, 6)), (0, true));
    }

    #[test]
    fn undoing_a_random_fill_restores_cells_ages_and_generation() {
        let mut sim = sim();
        paint(&mut sim, &BLOCK, None);
        paint(&mut sim, &[(8, 7), (9, 8), (7, 9), (8, 9), (9, 9)], None);
        for _ in 0..5 {
            sim.handle(Command::Step, ZERO);
        }
        let before = state(&sim.universe);
        assert!(!no_trail(&sim.universe));
        sim.handle(Command::RandomFill { density: 0.5, seed: 7 }, ZERO);
        let filled = state(&sim.universe);
        assert_eq!(sim.universe.generation(), 0);

        sim.handle(Command::Undo, ZERO);
        assert_eq!(state(&sim.universe), before);
        assert_eq!(sim.universe.generation(), 5);
        assert!(no_trail(&sim.universe));
        assert_eq!(sim.snapshot().universe.generation(), 5);

        sim.handle(Command::Redo, ZERO);
        assert_eq!(state(&sim.universe), filled);
        assert_eq!(sim.universe.generation(), 0);
    }

    #[test]
    fn a_change_after_undo_discards_redo_and_the_51st_step_drops_the_oldest() {
        let mut sim = sim();
        for i in 0..51 {
            paint(&mut sim, &[(i % 16, i / 16)], None);
        }
        assert_eq!(sim.history.len(), (50, 0));
        for _ in 0..51 {
            sim.handle(Command::Undo, ZERO);
        }
        // The first cell's step was dropped, so it stays.
        assert_eq!(sim.universe.live_count(), 1);
        assert!(sim.universe.get(0, 0));
        assert_eq!(sim.history.len(), (0, 50));

        sim.handle(Command::Redo, ZERO);
        paint(&mut sim, &[(10, 10)], None);
        assert_eq!(sim.history.len(), (2, 0));
        assert!(!sim.snapshot().can_redo);
        sim.handle(Command::Redo, ZERO);
        assert_eq!(sim.universe.live_count(), 3);
    }

    #[test]
    fn generations_clear_the_history_and_set_edges_keeps_it() {
        let mut sim = sim();
        blinker(&mut sim);
        sim.handle(Command::SetEdges(Edges::Dead), ZERO);
        assert!(sim.snapshot().can_undo);
        sim.handle(Command::Step, ZERO);
        assert_eq!(sim.history.len(), (0, 0));
        assert!(!sim.snapshot().can_undo);

        paint(&mut sim, &[(10, 10)], None);
        paint(&mut sim, &[(12, 12)], None);
        sim.handle(Command::Undo, ZERO);
        // Play and Pause with no generation between them keep the history.
        sim.handle(Command::Play, ZERO);
        assert!(!sim.snapshot().can_undo && !sim.snapshot().can_redo);
        sim.handle(Command::Pause, ZERO);
        assert!(sim.snapshot().can_undo && sim.snapshot().can_redo);
        // The first generation while playing clears it.
        sim.handle(Command::Play, ZERO);
        sim.tick(ZERO);
        assert_eq!(sim.universe.generation(), 2);
        sim.handle(Command::Pause, ZERO);
        assert_eq!(sim.history.len(), (0, 0));
        assert!(!sim.snapshot().can_undo && !sim.snapshot().can_redo);
    }

    #[test]
    fn while_playing_edits_apply_unrecorded_and_undo_does_nothing() {
        let mut sim = sim();
        paint(&mut sim, &[(3, 3)], None);
        sim.handle(Command::Play, ZERO);
        // A change made while playing is not recorded, and the steps before it
        // no longer lead back, even before the first generation.
        paint(&mut sim, &[(5, 3)], Some(1));
        assert!(sim.universe.get(5, 3));
        assert_eq!(sim.history.len(), (0, 0));
        sim.handle(Command::KillRect(Rect { x: 0, y: 0, w: 4, h: 4 }), ZERO);
        sim.handle(Command::Undo, ZERO);
        assert!(!sim.universe.get(3, 3) && sim.universe.get(5, 3));
        assert!(!sim.snapshot().can_undo);
    }

    #[test]
    fn the_unsaved_mark_follows_the_saved_revision() {
        let mut sim = sim();
        assert!(!sim.snapshot().dirty);
        paint(&mut sim, &[(1, 1)], None);
        assert!(sim.snapshot().dirty);
        sim.handle(Command::MarkSaved { revision: sim.snapshot().revision }, ZERO);
        assert!(!sim.snapshot().dirty);

        paint(&mut sim, &[(2, 2)], None);
        assert!(sim.snapshot().dirty);
        sim.handle(Command::Undo, ZERO);
        assert!(!sim.snapshot().dirty);
        sim.handle(Command::Redo, ZERO);
        assert!(sim.snapshot().dirty);
        sim.handle(Command::Undo, ZERO);
        sim.handle(Command::Undo, ZERO);
        assert!(sim.snapshot().dirty);
        sim.handle(Command::Redo, ZERO);
        assert!(!sim.snapshot().dirty);

        // A generation after a save is a change too.
        sim.handle(Command::Step, ZERO);
        assert!(sim.snapshot().dirty);
        sim.handle(Command::MarkSaved { revision: sim.snapshot().revision }, ZERO);
        assert!(!sim.snapshot().dirty);
        // New and opened universes start saved.
        paint(&mut sim, &[(4, 4)], None);
        let (tiling, rule) = (Tiling::Square, Tiling::Square.default_rule());
        sim.handle(Command::NewUniverse { width: 20, height: 20, edges: Edges::Wrap, tiling, rule }, ZERO);
        assert!(!sim.snapshot().dirty);
        paint(&mut sim, &[(4, 4)], None);
        sim.handle(Command::Load(Universe::new(30, 30, Edges::Dead, Tiling::Square).unwrap()), ZERO);
        assert!(!sim.snapshot().dirty);
        assert_eq!(sim.history.len(), (0, 0));
    }

    #[test]
    fn changing_edges_marks_unsaved_changes_that_undo_cannot_remove() {
        let mut sim = sim();
        paint(&mut sim, &[(1, 1)], None);
        sim.handle(Command::MarkSaved { revision: sim.snapshot().revision }, ZERO);
        sim.handle(Command::SetEdges(Edges::Wrap), ZERO);
        assert!(!sim.snapshot().dirty);
        sim.handle(Command::SetEdges(Edges::Dead), ZERO);
        assert!(sim.snapshot().dirty);
        sim.handle(Command::Undo, ZERO);
        sim.handle(Command::Redo, ZERO);
        assert!(sim.snapshot().dirty);
    }

    #[test]
    fn saving_an_older_revision_while_playing_leaves_the_universe_dirty() {
        let mut sim = sim();
        blinker(&mut sim);
        sim.handle(Command::SetSpeed(None), ZERO);
        sim.handle(Command::Play, ZERO);
        let saved = sim.snapshot().revision;
        sim.tick(ZERO);
        sim.handle(Command::MarkSaved { revision: saved }, ZERO);
        assert!(sim.snapshot().dirty);
        sim.handle(Command::Pause, ZERO);
        assert!(sim.snapshot().dirty);
    }

    #[test]
    fn a_placement_across_a_wrapping_corner_undoes_exactly() {
        let mut sim = sim();
        // A block that ages, and a lone cell that leaves a trail.
        paint(&mut sim, &[(0, 0), (1, 0), (0, 1), (1, 1), (14, 15)], None);
        sim.handle(Command::Step, ZERO);
        sim.handle(Command::Step, ZERO);
        let before = state(&sim.universe);

        let mut pattern = Pattern::new(4, 3, Tiling::Square, false);
        for (x, y) in [(0, 0), (3, 0), (1, 1), (2, 2)] {
            pattern.set(x, y, true);
        }
        let place = Command::Place { pattern: Arc::new(pattern.clone()), x: 14, y: 15, mode: PasteMode::Copy };
        sim.handle(place, ZERO);
        // Columns 14, 15, 0, 1 and rows 15, 0, 1.
        for (py, gy) in [15, 0, 1].into_iter().enumerate() {
            for (px, gx) in [14, 15, 0, 1].into_iter().enumerate() {
                assert_eq!(sim.universe.get(gx, gy), pattern.get(px as u32, py as u32), "({gx}, {gy})");
            }
        }
        let placed = state(&sim.universe);
        assert_eq!(sim.history.len(), (1, 0));

        sim.handle(Command::Undo, ZERO);
        assert_eq!(state(&sim.universe), before);
        assert_eq!(sim.universe.generation(), 2);
        sim.handle(Command::Redo, ZERO);
        assert_eq!(state(&sim.universe), placed);
    }

    #[test]
    fn delete_and_flip_of_a_selection_are_one_step_each() {
        let mut sim = sim();
        paint(&mut sim, &BLOCK, None);
        paint(&mut sim, &[(5, 1), (6, 1), (7, 1)], None);
        sim.handle(Command::Step, ZERO);
        let before = state(&sim.universe);
        let rect = Rect { x: 0, y: 0, w: 8, h: 4 };
        sim.handle(Command::FlipRect(rect, Axis::Horizontal), ZERO);
        assert_eq!(sim.universe.age(6, 1), 1);
        assert!(sim.universe.get(1, 0) && !sim.universe.get(6, 0));
        sim.handle(Command::KillRect(rect), ZERO);
        assert_eq!(sim.universe.live_count(), 0);
        assert_eq!(sim.history.len(), (2, 0));
        sim.handle(Command::Undo, ZERO);
        sim.handle(Command::Undo, ZERO);
        assert_eq!(state(&sim.universe), before);
    }

    #[test]
    fn commands_that_do_not_fit_the_grid_change_nothing() {
        let mut sim = sim();
        let revision = sim.revision;
        sim.handle(Command::KillRect(Rect { x: 10, y: 0, w: 7, h: 1 }), ZERO);
        sim.handle(Command::FlipRect(Rect { x: 0, y: 0, w: 0, h: 4 }, Axis::Vertical), ZERO);
        sim.handle(Command::Place { pattern: Arc::new(Pattern::new(17, 2, Tiling::Square, false)), x: 0, y: 0, mode: PasteMode::Or }, ZERO);
        sim.handle(Command::SetEdges(Edges::Dead), ZERO);
        let revision_after_edges = sim.revision;
        assert_ne!(revision_after_edges, revision);
        sim.handle(Command::Place { pattern: Arc::new(Pattern::new(4, 4, Tiling::Square, false)), x: -10, y: 3, mode: PasteMode::Copy }, ZERO);
        sim.handle(Command::Edit { cells: vec![(16, 0), (0, 99)], alive: true, stroke: None }, ZERO);
        assert_eq!(sim.revision, revision_after_edges);
        assert_eq!(sim.history.len(), (0, 0));
    }

    /// Run with `cargo test --release -p glife-app -- --ignored --nocapture undo_memory`.
    #[test]
    #[ignore]
    fn undo_memory_of_50_whole_grid_fills_at_2048() {
        let mut universe = Universe::new(2048, 2048, Edges::Wrap, Tiling::Square).unwrap();
        universe.random_fill(0.25, 1);
        let mut sim = Sim::new(universe, ZERO);
        sim.handle(Command::SetSpeed(None), ZERO);
        sim.handle(Command::Play, ZERO);
        for _ in 0..1000 {
            sim.tick(ZERO);
        }
        sim.handle(Command::Pause, ZERO);
        assert_eq!(sim.universe.generation(), 1000);
        let start = std::time::Instant::now();
        for seed in 0..50 {
            sim.handle(Command::RandomFill { density: 0.25, seed }, ZERO);
        }
        let fills = start.elapsed();
        let bytes = sim.history.bytes();
        println!(
            "undo history after 50 fills of 2048 x 2048: {:.1} MiB; {:.1} ms per fill",
            bytes as f64 / (1 << 20) as f64,
            fills.as_secs_f64() * 1000.0 / 50.0
        );
        assert!(bytes < 256 << 20);
        for _ in 0..50 {
            sim.handle(Command::Undo, ZERO);
        }
        assert_eq!(sim.universe.generation(), 1000);
    }

    fn grid_sim(tiling: Tiling, width: u32, height: u32, edges: Edges) -> Sim {
        Sim::new(Universe::new(width, height, edges, tiling).unwrap(), ZERO)
    }

    #[test]
    fn a_new_universe_of_each_grid_takes_its_tiling_and_rule() {
        for tiling in Tiling::ALL {
            let mut sim = sim();
            let rule = tiling.default_rule();
            sim.handle(Command::NewUniverse { width: 20, height: 12, edges: Edges::Wrap, tiling, rule }, ZERO);
            let u = sim.snapshot();
            let u = &u.universe;
            assert_eq!((u.tiling(), u.rule(), u.width(), u.height()), (tiling, rule, 20, 12));
            assert!(!sim.snapshot().dirty);
        }
        // A rule the grid cannot run, or an odd side that cannot wrap, changes nothing.
        let mut sim = sim();
        let hex = Tiling::Hexagonal;
        let rule = Rule::parse("B3/S23", Tiling::Square).unwrap();
        let too_many = Rule { birth: 1 << 7, survival: 0 };
        sim.handle(Command::NewUniverse { width: 20, height: 12, edges: Edges::Wrap, tiling: hex, rule: too_many }, ZERO);
        sim.handle(Command::NewUniverse { width: 20, height: 11, edges: Edges::Wrap, tiling: hex, rule }, ZERO);
        assert_eq!((sim.universe.width(), sim.universe.tiling()), (16, Tiling::Square));
    }

    #[test]
    fn set_edges_on_an_odd_side_is_refused_for_the_grids_that_need_even() {
        for (tiling, width, height, refused) in [
            (Tiling::Square, 15, 15, false),
            (Tiling::Hexagonal, 15, 15, true),
            (Tiling::Hexagonal, 15, 16, false),
            (Tiling::Triangular12, 16, 15, true),
            (Tiling::Triangular3, 15, 16, true),
            (Tiling::Triangular3, 16, 16, false),
        ] {
            let mut sim = grid_sim(tiling, width, height, Edges::Dead);
            sim.handle(Command::SetEdges(Edges::Wrap), ZERO);
            let wrapped = sim.universe.edges() == Edges::Wrap;
            assert_eq!(wrapped, !refused, "{tiling:?} {width} x {height}");
        }
    }

    #[test]
    fn set_rule_applies_marks_unsaved_and_refuses_what_the_grid_cannot_run() {
        let mut sim = grid_sim(Tiling::Square, 16, 16, Edges::Wrap);
        let highlife = Rule::parse("B36/S23", Tiling::Square).unwrap();
        sim.handle(Command::SetRule(highlife), ZERO);
        assert_eq!(sim.universe.rule(), highlife);
        assert!(sim.snapshot().dirty);
        assert_eq!(sim.snapshot().universe.rule(), highlife);
        // Undo cannot take it back, so the universe stays unsaved.
        sim.handle(Command::MarkSaved { revision: sim.snapshot().revision }, ZERO);
        assert!(!sim.snapshot().dirty);
        sim.handle(Command::SetRule(Tiling::Square.default_rule()), ZERO);
        assert!(sim.snapshot().dirty);
        sim.handle(Command::Undo, ZERO);
        assert!(sim.snapshot().dirty);
        // Setting the rule it already has changes nothing.
        let revision = sim.revision;
        sim.handle(Command::SetRule(Tiling::Square.default_rule()), ZERO);
        assert_eq!(sim.revision, revision);

        let mut hex = grid_sim(Tiling::Hexagonal, 16, 16, Edges::Wrap);
        let revision = hex.revision;
        hex.handle(Command::SetRule(Rule { birth: 1 << 7, survival: 0 }), ZERO);
        hex.handle(Command::SetRule(Rule { birth: 1, survival: 0 }), ZERO);
        assert_eq!((hex.universe.rule(), hex.revision), (Tiling::Hexagonal.default_rule(), revision));
    }

    #[test]
    fn the_new_rule_runs_from_the_next_generation() {
        // A lone pair dies under B3/S23 and under B2/S, a third cell next to them is born only under B2.
        let mut sim = grid_sim(Tiling::Square, 16, 16, Edges::Dead);
        paint(&mut sim, &[(5, 5), (6, 5)], None);
        sim.handle(Command::SetRule(Rule::parse("B2/S", Tiling::Square).unwrap()), ZERO);
        sim.handle(Command::Step, ZERO);
        assert!(sim.universe.get(5, 4) && sim.universe.get(6, 6) && !sim.universe.get(5, 5));
    }

    #[test]
    fn undo_after_a_hexagonal_flip_that_reaches_past_the_selection_restores_every_cell() {
        let mut sim = grid_sim(Tiling::Hexagonal, 16, 16, Edges::Wrap);
        // Rows 0 and 2 are unshifted: a horizontal flip sends column x to 5 - x,
        // so column 0 lands one past the selection, on column 7 where a cell sits.
        paint(&mut sim, &[(2, 0), (4, 0), (3, 1), (2, 2), (7, 0), (7, 2), (7, 1)], None);
        let before = state(&sim.universe);
        let rect = Rect { x: 2, y: 0, w: 5, h: 4 };
        assert_eq!(sim.universe.flip_footprint(rect, Axis::Horizontal), vec![Rect { w: 6, ..rect }]);
        sim.handle(Command::FlipRect(rect, Axis::Horizontal), ZERO);
        let flipped = state(&sim.universe);
        assert_ne!(flipped, before);
        assert!(!sim.universe.get(2, 0), "column 2 of an unshifted row mirrors to 7");
        assert!(sim.universe.get(7, 0));
        sim.handle(Command::Undo, ZERO);
        assert_eq!(state(&sim.universe), before);
        assert!(no_trail(&sim.universe));
        sim.handle(Command::Redo, ZERO);
        assert_eq!(state(&sim.universe), flipped);
    }

    #[test]
    fn a_pattern_as_wide_as_a_non_square_grid_is_placed_or_ignored_never_a_panic() {
        // 8 wide at an anchor of the other parity becomes 9 wide on the hexagonal grid.
        let mut hex = grid_sim(Tiling::Hexagonal, 8, 8, Edges::Wrap);
        let mut pattern = Pattern::new(8, 2, Tiling::Hexagonal, false);
        pattern.set(0, 0, true);
        // Top-left (0, 1): row 1 is shifted, so the pattern needs a ninth column.
        hex.handle(Command::Place { pattern: Arc::new(pattern.clone()), x: 0, y: 1, mode: PasteMode::Or }, ZERO);
        assert_eq!(hex.universe.live_count(), 0);
        assert_eq!(hex.history.len(), (0, 0));
        hex.handle(Command::Place { pattern: Arc::new(pattern), x: 0, y: 0, mode: PasteMode::Or }, ZERO);
        assert!(hex.universe.get(0, 0));

        // A pattern of another tiling is ignored.
        let mut sim = sim();
        let triangles = Pattern::new(2, 2, Tiling::Triangular3, false);
        sim.handle(Command::Place { pattern: Arc::new(triangles), x: 0, y: 0, mode: PasteMode::Copy }, ZERO);
        assert_eq!(sim.history.len(), (0, 0));
    }

    #[test]
    fn bench_values_name_a_grid_and_a_rule() {
        let square = Some((Tiling::Square, Tiling::Square.default_rule()));
        assert_eq!(parse_bench(None), Ok(None));
        assert_eq!(parse_bench(Some("")), Ok(None));
        assert_eq!(parse_bench(Some("1")), Ok(square));
        assert_eq!(
            parse_bench(Some("hexagonal")),
            Ok(Some((Tiling::Hexagonal, Tiling::Hexagonal.default_rule())))
        );
        assert_eq!(
            parse_bench(Some("triangular-12:B4/S456")),
            Ok(Some((Tiling::Triangular12, Rule::parse("B4/S456", Tiling::Triangular12).unwrap())))
        );
        assert!(parse_bench(Some("octagonal")).is_err());
        assert!(parse_bench(Some("triangular-3:B4/S2")).is_err());
    }

    #[test]
    fn bench_report_is_one_and_needs_a_benchmark() {
        assert_eq!(parse_bench_report(None, true), Ok(false));
        assert_eq!(parse_bench_report(Some(""), false), Ok(false));
        assert_eq!(parse_bench_report(Some("1"), true), Ok(true));
        assert!(parse_bench_report(Some("1"), false).is_err());
        assert!(parse_bench_report(Some("0"), true).is_err());
        assert!(parse_bench_report(Some("yes"), true).is_err());
    }

    #[test]
    fn bench_launch_opens_a_2048_wrapping_universe_of_the_grid_playing() {
        let rule = Rule::parse("B3/S23", Tiling::Hexagonal).unwrap();
        let sim = Sim::launch(Some((Tiling::Hexagonal, rule)));
        let s = sim.snapshot();
        let u = &s.universe;
        assert_eq!((u.width(), u.height(), u.edges(), u.tiling(), u.rule()), (2048, 2048, Edges::Wrap, Tiling::Hexagonal, rule));
        assert!(s.playing);
        let share = u.live_count() as f64 / (2048.0 * 2048.0);
        assert!((0.24..0.26).contains(&share), "{share}");
    }

    #[test]
    fn launch_defaults() {
        let sim = Sim::launch(None);
        let s = sim.snapshot();
        assert_eq!((s.universe.width(), s.universe.height()), (256, 256));
        assert_eq!(s.universe.edges(), Edges::Wrap);
        assert!(!s.playing);
    }
}
