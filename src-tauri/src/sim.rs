//! The simulation thread's state machine. All logic lives in `Sim::handle` and
//! `Sim::tick`; `run` is a thin loop around them. Time is a `Duration` since
//! the thread started, so tests drive it with a fake clock.

use std::collections::VecDeque;
use std::hash::{BuildHasher, Hasher};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use glife_engine::{Edges, Universe};

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
}

/// The latest snapshot, swapped under a mutex.
pub type Latest = Arc<Mutex<Arc<Snapshot>>>;

pub enum Command {
    NewUniverse { width: u32, height: u32, edges: Edges },
    Play,
    Pause,
    Step,
    /// `None` means as fast as possible.
    SetSpeed(Option<f64>),
    SetEdges(Edges),
    Edit { cells: Vec<(u32, u32)>, alive: bool },
    Clear,
    /// `density` is a fraction, 0.0 to 1.0.
    RandomFill { density: f64, seed: u64 },
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
    dirty: bool,
    latest: Latest,
}

impl Sim {
    /// A 256 x 256 wrapping universe, paused; with `bench`, a 2048 x 2048
    /// universe filled to 25 % and playing as fast as possible.
    pub fn launch(bench: bool) -> Sim {
        let mut sim = if bench {
            let mut universe = Universe::new(2048, 2048, Edges::Wrap).expect("valid size");
            universe.random_fill(0.25, os_seed());
            Sim::new(universe, Duration::ZERO)
        } else {
            Sim::new(Universe::new(256, 256, Edges::Wrap).expect("valid size"), Duration::ZERO)
        };
        if bench {
            sim.handle(Command::SetSpeed(None), Duration::ZERO);
            sim.handle(Command::Play, Duration::ZERO);
        }
        sim
    }

    pub fn new(universe: Universe, now: Duration) -> Sim {
        let snapshot = Arc::new(Snapshot {
            version: 1,
            universe: universe.clone(),
            playing: false,
            gens_per_sec: 0.0,
        });
        Sim {
            universe,
            playing: false,
            speed: Some(10.0),
            deadline: now,
            steps: VecDeque::new(),
            version: 1,
            last_publish: now,
            dirty: false,
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

    /// Applies one command between generations, then publishes.
    pub fn handle(&mut self, command: Command, now: Duration) {
        match command {
            Command::NewUniverse { width, height, edges } => {
                // The caller validates the size; an invalid one changes nothing.
                if let Ok(universe) = Universe::new(width, height, edges) {
                    self.universe = universe;
                    self.set_playing(false, now);
                }
            }
            Command::Play => self.set_playing(true, now),
            Command::Pause => self.set_playing(false, now),
            Command::Step => {
                if !self.playing {
                    self.universe.step();
                }
            }
            Command::SetSpeed(speed) => {
                self.speed = speed;
                self.deadline = now;
            }
            Command::SetEdges(edges) => self.universe.set_edges(edges),
            Command::Edit { cells, alive } => {
                for (x, y) in cells {
                    self.universe.set(x, y, alive);
                }
            }
            Command::Clear => self.universe.clear(),
            Command::RandomFill { density, seed } => self.universe.random_fill(density, seed),
        }
        self.publish(now);
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
        if self.dirty && now >= self.last_publish + PUBLISH_INTERVAL {
            self.publish(now);
        }
        if !self.playing {
            return None;
        }
        let publish_due = self.dirty.then(|| self.last_publish + PUBLISH_INTERVAL);
        let generation_due = self.speed.map_or(now, |_| self.deadline);
        Some(publish_due.map_or(generation_due, |p| p.min(generation_due)))
    }

    fn generation(&mut self, now: Duration) {
        self.universe.step();
        self.steps.push_back(now);
        self.dirty = true;
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
        let mut latest = self.latest.lock().unwrap();
        // Reuse the previous snapshot's buffers unless a reader still holds it.
        if let Some(snapshot) = Arc::get_mut(&mut latest) {
            snapshot.version = self.version;
            snapshot.universe.clone_from(&self.universe);
            snapshot.playing = self.playing;
            snapshot.gens_per_sec = gens_per_sec;
        } else {
            *latest = Arc::new(Snapshot {
                version: self.version,
                universe: self.universe.clone(),
                playing: self.playing,
                gens_per_sec,
            });
        }
        drop(latest);
        self.last_publish = now;
        self.dirty = false;
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
        Sim::new(Universe::new(16, 16, Edges::Wrap).unwrap(), Duration::ZERO)
    }

    fn blinker(sim: &mut Sim) {
        let cells = vec![(5, 5), (6, 5), (7, 5)];
        sim.handle(Command::Edit { cells, alive: true }, Duration::ZERO);
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
        sim.handle(Command::Edit { cells: vec![(1, 1), (2, 2)], alive: true }, Duration::ZERO);
        sim.handle(Command::Edit { cells: vec![(1, 1)], alive: false }, Duration::ZERO);
        sim.handle(Command::Edit { cells: vec![(1, 1)], alive: true }, Duration::ZERO);
        sim.handle(Command::Edit { cells: vec![(2, 2)], alive: false }, Duration::ZERO);
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
        wrapped.handle(Command::Edit { cells: cells.clone(), alive: true }, Duration::ZERO);
        wrapped.handle(Command::Step, Duration::ZERO);
        assert_eq!(wrapped.universe.live_count(), 3);

        let mut dead = sim();
        dead.handle(Command::Edit { cells, alive: true }, Duration::ZERO);
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
        sim.handle(Command::Edit { cells: vec![], alive: true }, MS(2500));
        assert_eq!(sim.snapshot().gens_per_sec, 10.0);
        sim.handle(Command::Pause, MS(2500));
        assert_eq!(sim.snapshot().gens_per_sec, 0.0);
    }

    #[test]
    fn new_universe_is_empty_paused_and_replaces_the_old_one() {
        let mut sim = sim();
        sim.handle(Command::Play, Duration::ZERO);
        sim.handle(Command::NewUniverse { width: 40, height: 30, edges: Edges::Dead }, Duration::ZERO);
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

    #[test]
    fn launch_defaults() {
        let sim = Sim::launch(false);
        let s = sim.snapshot();
        assert_eq!((s.universe.width(), s.universe.height()), (256, 256));
        assert_eq!(s.universe.edges(), Edges::Wrap);
        assert!(!s.playing);
    }
}
