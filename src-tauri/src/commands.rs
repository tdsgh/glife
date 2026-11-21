//! Tauri commands. Changes to the universe go through the simulation thread's
//! queue; read-only commands read the latest snapshot or the pending slot. → ADR-011

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use glife_engine::{Axis, Edges, PasteMode, Pattern, PlaceError, Rect, Rule, SizeError, Tiling, MAX_SIDE, MIN_SIDE};
use tauri::ipc::Response;
use tauri::State;

use serde::{Deserialize, Serialize};

use crate::frame;
use crate::library::builtin;
use crate::library::personal::Library;
use crate::library::Problem;
use crate::sim::{self, os_seed, Command, Latest};

/// A pattern waiting to be placed: its size, grid, rule, name, comment and notices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingInfo {
    pub width: u32,
    pub height: u32,
    pub tiling: &'static str,
    /// The rule the pattern was made under, as text.
    pub rule: String,
    pub name: String,
    pub comment: String,
    pub notices: Vec<String>,
}

/// Where a file-backed universe came from.
#[derive(Debug, Default, Clone)]
pub struct FileState {
    /// The `.glife` file last saved to or opened from.
    pub path: Option<PathBuf>,
    /// A name for Save As when there is no path, from an opened RLE file.
    pub suggested: Option<String>,
}

pub struct Host {
    pub queue: Sender<Command>,
    pub latest: Latest,
    /// The pattern being placed.
    pub pending: Mutex<Option<Arc<Pattern>>>,
    /// Name, comment and rule of the pending pattern.
    pub pending_meta: Mutex<(String, String, Rule)>,
    pub file: Mutex<FileState>,
    pub builtin: Vec<builtin::Entry>,
    pub builtin_problems: Vec<Problem>,
    pub personal: Library,
    /// Set once the unsaved-changes prompt has been answered at quit.
    pub quitting: AtomicBool,
}

impl Host {
    pub fn new(queue: Sender<Command>, latest: Latest, builtin_dir: Option<&Path>, personal_dir: &Path) -> Result<Host, String> {
        let (builtin, builtin_problems) = match builtin_dir {
            Some(dir) => builtin::load(dir),
            None => (
                Vec::new(),
                vec![Problem { file: "patterns".into(), reason: "The built-in pattern folder was not found.".into() }],
            ),
        };
        Ok(Host {
            queue,
            latest,
            pending: Mutex::new(None),
            pending_meta: Mutex::new((String::new(), String::new(), Tiling::Square.default_rule())),
            file: Mutex::new(FileState::default()),
            builtin,
            builtin_problems,
            personal: Library::open(personal_dir)?,
            quitting: AtomicBool::new(false),
        })
    }

    /// The grid and rule of the universe as of the latest snapshot.
    pub fn grid(&self) -> (Tiling, Rule) {
        let snapshot = self.latest.lock().unwrap().clone();
        (snapshot.universe.tiling(), snapshot.universe.rule())
    }

    /// Fills the pending slot, refusing a pattern of another grid or larger
    /// than the grid. A pattern made under another rule of the grid is
    /// accepted with a notice naming its rule.
    pub fn set_pending(&self, pattern: Pattern, rule: Rule, name: String, comment: String, mut notices: Vec<String>) -> Result<PendingInfo, String> {
        let (w, h) = (pattern.width(), pattern.height());
        let (grid, grid_tiling, grid_rule) = {
            let snapshot = self.latest.lock().unwrap().clone();
            let u = &snapshot.universe;
            ((u.width(), u.height()), u.tiling(), u.rule())
        };
        if pattern.tiling() != grid_tiling || w > grid.0 || h > grid.1 {
            return Err(PlaceError { pattern: (w, h), grid, pattern_tiling: pattern.tiling(), grid_tiling }.to_string());
        }
        if rule != grid_rule {
            notices.push(format!("This pattern was made under {rule}; this universe runs {grid_rule}."));
        }
        let info = PendingInfo { width: w, height: h, tiling: pattern.tiling().id(), rule: rule.to_string(), name, comment, notices };
        *self.pending.lock().unwrap() = Some(Arc::new(pattern));
        *self.pending_meta.lock().unwrap() = (info.name.clone(), info.comment.clone(), rule);
        Ok(info)
    }

    pub fn send(&self, command: Command) -> Result<(), String> {
        self.queue.send(command).map_err(|_| "simulation thread stopped".to_string())
    }

    pub fn pending(&self) -> Result<Arc<Pattern>, String> {
        self.pending.lock().unwrap().clone().ok_or_else(|| "no pattern is being placed".to_string())
    }

    /// `rect` if it lies inside the grid as of the latest snapshot.
    pub fn selection(&self, rect: Rect) -> Result<Rect, String> {
        let snapshot = self.latest.lock().unwrap().clone();
        let u = &snapshot.universe;
        if sim::inside(rect, u) {
            Ok(rect)
        } else {
            let Rect { x, y, w, h } = rect;
            Err(format!("selection {w} x {h} at ({x}, {y}) is outside the {} x {} grid", u.width(), u.height()))
        }
    }
}

fn parse_edges(edges: &str) -> Result<Edges, String> {
    match edges {
        "wrap" => Ok(Edges::Wrap),
        "dead" => Ok(Edges::Dead),
        other => Err(format!("unknown edge mode {other:?}")),
    }
}

fn parse_tiling(id: &str) -> Result<Tiling, String> {
    Tiling::parse(id).ok_or_else(|| format!("unknown grid {id:?}"))
}

/// The refusal for a `width` by `height` universe of `tiling` with `edges`, if any.
fn check_size(tiling: Tiling, width: u32, height: u32, edges: Edges) -> Result<(), String> {
    let sides = MIN_SIDE..=MAX_SIDE;
    if !sides.contains(&width) || !sides.contains(&height) {
        return Err(SizeError::Range { width, height }.to_string());
    }
    let (even_width, even_height) = tiling.wrap_needs_even();
    if edges == Edges::Wrap && ((even_width && width % 2 == 1) || (even_height && height % 2 == 1)) {
        return Err(SizeError::OddWrap { tiling, width, height }.to_string());
    }
    Ok(())
}

/// A rule as text, or the default of `tiling` when none is given.
fn parse_rule(text: Option<&str>, tiling: Tiling) -> Result<Rule, String> {
    match text {
        Some(text) => Rule::parse(text, tiling).map_err(|e| e.to_string()),
        None => Ok(tiling.default_rule()),
    }
}

/// A rule with its masks as the UI reads them.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInfo {
    pub text: String,
    pub birth: u16,
    pub survival: u16,
}

impl From<Rule> for RuleInfo {
    fn from(rule: Rule) -> RuleInfo {
        RuleInfo { text: rule.to_string(), birth: rule.birth, survival: rule.survival }
    }
}

#[derive(Debug, Serialize)]
pub struct PresetInfo {
    pub name: &'static str,
    pub rule: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TilingInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub neighbors: u32,
    pub default_rule: RuleInfo,
    pub presets: Vec<PresetInfo>,
    pub wrap_even_width: bool,
    pub wrap_even_height: bool,
}

/// Managed state: whether the process runs as a benchmark report (`GLIFE_BENCH_REPORT=1`).
pub struct BenchReportMode(pub bool);

#[tauri::command]
pub fn bench_report_mode(mode: State<BenchReportMode>) -> bool {
    mode.0
}

/// Prints the report line on stdout and exits: code 0 when `ok`, 1 otherwise. Does nothing outside report mode.
#[tauri::command]
pub fn bench_report(mode: State<BenchReportMode>, line: String, ok: bool) {
    if !mode.0 {
        return;
    }
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "GLIFE_BENCH_REPORT {}", line.replace(['\r', '\n'], " "));
    let _ = out.flush();
    std::process::exit(if ok { 0 } else { 1 });
}

/// Every grid, with its default rule and presets.
#[tauri::command]
pub fn tilings() -> Vec<TilingInfo> {
    Tiling::ALL
        .into_iter()
        .map(|t| TilingInfo {
            id: t.id(),
            name: t.name(),
            neighbors: t.neighbors(),
            default_rule: t.default_rule().into(),
            presets: t.presets().iter().map(|&(name, rule)| PresetInfo { name, rule: rule.to_string() }).collect(),
            wrap_even_width: t.wrap_needs_even().0,
            wrap_even_height: t.wrap_needs_even().1,
        })
        .collect()
}

/// `tiling` defaults to `"square"` and `rule` to the grid's default rule.
#[tauri::command]
pub fn new_universe(
    host: State<Host>,
    width: u32,
    height: u32,
    edges: String,
    tiling: Option<String>,
    rule: Option<String>,
) -> Result<(), String> {
    let tiling = tiling.as_deref().map_or(Ok(Tiling::Square), parse_tiling)?;
    let edges = parse_edges(&edges)?;
    check_size(tiling, width, height, edges)?;
    let rule = parse_rule(rule.as_deref(), tiling)?;
    *host.file.lock().unwrap() = FileState::default();
    host.send(Command::NewUniverse { width, height, edges, tiling, rule })
}

#[tauri::command]
pub fn play(host: State<Host>) -> Result<(), String> {
    host.send(Command::Play)
}

#[tauri::command]
pub fn pause(host: State<Host>) -> Result<(), String> {
    host.send(Command::Pause)
}

#[tauri::command]
pub fn step(host: State<Host>) -> Result<(), String> {
    host.send(Command::Step)
}

/// `gens_per_sec` null means as fast as possible.
#[tauri::command]
pub fn set_speed(host: State<Host>, gens_per_sec: Option<f64>) -> Result<(), String> {
    if gens_per_sec.is_some_and(|g| !(g.is_finite() && g > 0.0)) {
        return Err("speed must be a positive number or null".into());
    }
    host.send(Command::SetSpeed(gens_per_sec))
}

#[tauri::command]
pub fn set_edges(host: State<Host>, edges: String) -> Result<(), String> {
    let edges = parse_edges(&edges)?;
    let snapshot = host.latest.lock().unwrap().clone();
    let u = &snapshot.universe;
    check_size(u.tiling(), u.width(), u.height(), edges)?;
    host.send(Command::SetEdges(edges))
}

/// Takes effect from the next generation; not an undo step.
#[tauri::command]
pub fn set_rule(host: State<Host>, rule: String) -> Result<(), String> {
    let tiling = host.latest.lock().unwrap().universe.tiling();
    host.send(Command::SetRule(parse_rule(Some(&rule), tiling)?))
}

/// Batches with the same `stroke` and no other command between them undo as
/// one step; without a `stroke`, each batch is its own step.
#[tauri::command]
pub fn edit(host: State<Host>, cells: Vec<(u32, u32)>, alive: bool, stroke: Option<u64>) -> Result<(), String> {
    host.send(Command::Edit { cells, alive, stroke })
}

#[tauri::command]
pub fn undo(host: State<Host>) -> Result<(), String> {
    host.send(Command::Undo)
}

#[tauri::command]
pub fn redo(host: State<Host>) -> Result<(), String> {
    host.send(Command::Redo)
}

#[tauri::command]
pub fn delete_selection(host: State<Host>, x: u32, y: u32, w: u32, h: u32) -> Result<(), String> {
    host.send(Command::KillRect(host.selection(Rect { x, y, w, h })?))
}

/// `axis` is `"h"` (left to right) or `"v"` (top to bottom).
#[tauri::command]
pub fn flip_selection(host: State<Host>, x: u32, y: u32, w: u32, h: u32, axis: String) -> Result<(), String> {
    let axis = match axis.as_str() {
        "h" => Axis::Horizontal,
        "v" => Axis::Vertical,
        other => return Err(format!("unknown flip axis {other:?}")),
    };
    host.send(Command::FlipRect(host.selection(Rect { x, y, w, h })?, axis))
}

/// The pattern flipped as asked.
fn flipped(mut pattern: Arc<Pattern>, flip_h: bool, flip_v: bool) -> Arc<Pattern> {
    if flip_h {
        pattern = Arc::new(pattern.flipped(Axis::Horizontal));
    }
    if flip_v {
        pattern = Arc::new(pattern.flipped(Axis::Vertical));
    }
    pattern
}

/// The placement buffer for `pattern` with its center cell at `(x, y)`:
/// `[width, height, left, top]`, where `left` and `top` are the `i32` offsets
/// of the stamped top-left cell from the anchor, then the pattern's rows.
fn placement_words(pattern: &Pattern, x: i64, y: i64) -> Vec<u32> {
    let (stamp, left, top) = pattern.placement(x, y);
    let mut words = vec![stamp.width(), stamp.height(), (left - x) as i32 as u32, (top - y) as i32 as u32];
    let mut rows = Vec::new();
    stamp.to_words(&mut rows);
    words.extend_from_slice(&rows[2..]);
    words
}

/// Stamps the pending pattern, flipped as asked, with its center cell
/// `(⌊w/2⌋, ⌊h/2⌋)` at `(x, y)`, lined up with the grid as the engine's
/// `placement` says. `mode` is `"or"` or `"copy"`.
#[tauri::command]
pub fn place(host: State<Host>, x: i64, y: i64, flip_h: bool, flip_v: bool, mode: String) -> Result<(), String> {
    let mode = match mode.as_str() {
        "or" => PasteMode::Or,
        "copy" => PasteMode::Copy,
        other => return Err(format!("unknown paste mode {other:?}")),
    };
    let pattern = flipped(host.pending()?, flip_h, flip_v);
    let (stamp, _, _) = pattern.placement(x, y);
    let (grid, grid_tiling) = {
        let snapshot = host.latest.lock().unwrap().clone();
        ((snapshot.universe.width(), snapshot.universe.height()), snapshot.universe.tiling())
    };
    if pattern.tiling() != grid_tiling || stamp.width() > grid.0 || stamp.height() > grid.1 {
        return Err(PlaceError { pattern: (stamp.width(), stamp.height()), grid, pattern_tiling: pattern.tiling(), grid_tiling }.to_string());
    }
    let (w, h) = (pattern.width(), pattern.height());
    host.send(Command::Place { pattern, x: x - (w / 2) as i64, y: y - (h / 2) as i64, mode })
}

/// The pending pattern, flipped as asked and lined up for an anchor at
/// `(x, y)`, as a placement buffer: exactly the cells `place` then stamps.
#[tauri::command]
pub fn pending_cells(host: State<Host>, flip_h: bool, flip_v: bool, x: i64, y: i64) -> Result<Response, String> {
    let pattern = flipped(host.pending()?, flip_h, flip_v);
    Ok(Response::new(placement_words(&pattern, x, y).iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>()))
}

#[tauri::command]
pub fn clear(host: State<Host>) -> Result<(), String> {
    host.send(Command::Clear)
}

/// `density` is a percent, 1 to 99, default 25.
#[tauri::command]
pub fn random_fill(host: State<Host>, density: Option<f64>) -> Result<(), String> {
    let percent = density.unwrap_or(25.0);
    if !(1.0..=99.0).contains(&percent) {
        return Err("density must be between 1 and 99 percent".into());
    }
    host.send(Command::RandomFill { density: percent / 100.0, seed: os_seed() })
}

#[tauri::command]
pub fn frame(host: State<Host>, x0: f64, y0: f64, w: f64, h: f64, since: u64, layers: u32) -> Response {
    let snapshot = host.latest.lock().unwrap().clone();
    Response::new(frame::encode(&snapshot, x0, y0, w, h, since, layers))
}

/// A rectangle as the UI sends it.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RectArg {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl From<RectArg> for Rect {
    fn from(r: RectArg) -> Rect {
        Rect { x: r.x, y: r.y, w: r.w, h: r.h }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Sim;
    use glife_engine::Universe;
    use std::time::Duration;

    /// An asymmetric pattern of `tiling`, 5 wide and 4 high, whose offset is set where the grid has one.
    fn pattern(tiling: Tiling, offset: bool) -> Pattern {
        let mut pattern = Pattern::new(5, 4, tiling, offset);
        for (x, y) in [(0, 0), (4, 0), (1, 1), (3, 1), (2, 2), (0, 3), (4, 3)] {
            pattern.set(x, y, true);
        }
        pattern
    }

    #[test]
    fn pending_cells_is_what_place_then_stamps_at_anchors_of_both_parities() {
        let words = |pattern: &Pattern, x, y| placement_words(pattern, x, y);
        for tiling in Tiling::ALL {
            for offset in [false, true] {
                if offset && tiling == Tiling::Square {
                    continue;
                }
                for (flip_h, flip_v) in [(false, false), (true, false), (false, true), (true, true)] {
                    let pending = Arc::new(pattern(tiling, offset));
                    let pattern = flipped(pending, flip_h, flip_v);
                    for (x, y) in [(10, 10), (11, 10), (10, 11), (11, 11)] {
                        let buffer = words(&pattern, x, y);
                        let (w, h, left, top) = (buffer[0], buffer[1], buffer[2] as i32, buffer[3] as i32);
                        assert_eq!(buffer.len(), 4 + (w.div_ceil(32) * h) as usize);

                        let universe = Universe::new(32, 32, Edges::Dead, tiling).unwrap();
                        let mut sim = Sim::new(universe, Duration::ZERO);
                        let (pw, ph) = (pattern.width(), pattern.height());
                        let command = Command::Place { pattern: pattern.clone(), x: x - (pw / 2) as i64, y: y - (ph / 2) as i64, mode: PasteMode::Copy };
                        sim.handle(command, Duration::ZERO);
                        let u = sim.latest().lock().unwrap().universe.clone();
                        assert_eq!(u.live_count(), pattern.live_count(), "{tiling:?} {x},{y}");
                        for gy in 0..32i64 {
                            for gx in 0..32i64 {
                                let (bx, by) = (gx - x - left as i64, gy - y - top as i64);
                                let in_buffer = (0..w as i64).contains(&bx) && (0..h as i64).contains(&by);
                                let expected = in_buffer && {
                                    let word = buffer[4 + by as usize * w.div_ceil(32) as usize + bx as usize / 32];
                                    word >> (bx % 32) & 1 == 1
                                };
                                assert_eq!(u.get(gx as u32, gy as u32), expected, "{tiling:?} offset {offset} flips {flip_h}{flip_v} anchor {x},{y} cell {gx},{gy}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_hexagonal_anchor_of_the_other_parity_widens_and_shifts_the_buffer() {
        let pattern = pattern(Tiling::Hexagonal, false);
        // Row 0 sits on an unshifted row at an even anchor; at an odd one the pattern is realigned.
        let even = placement_words(&pattern, 10, 10);
        let odd = placement_words(&pattern, 10, 11);
        assert_eq!((even[0], even[1]), (5, 4));
        assert_eq!((odd[0], odd[1]), (6, 4));
        // The top-left is the anchor minus (⌊w/2⌋, ⌊h/2⌋).
        assert_eq!((even[2] as i32, even[3] as i32), (-2, -2));
    }

    #[test]
    fn the_size_rules_name_the_reason() {
        assert!(check_size(Tiling::Square, 15, 15, Edges::Wrap).is_ok());
        assert!(check_size(Tiling::Hexagonal, 15, 15, Edges::Dead).is_ok());
        assert!(check_size(Tiling::Hexagonal, 15, 16, Edges::Wrap).is_ok());
        assert!(check_size(Tiling::Hexagonal, 16, 15, Edges::Wrap).unwrap_err().contains("even height"));
        assert!(check_size(Tiling::Triangular12, 15, 16, Edges::Wrap).unwrap_err().contains("even width"));
        assert!(check_size(Tiling::Triangular3, 2, 16, Edges::Dead).unwrap_err().contains("outside"));
    }

    #[test]
    fn rules_are_read_per_grid() {
        assert_eq!(parse_rule(None, Tiling::Hexagonal), Ok(Tiling::Hexagonal.default_rule()));
        assert!(parse_rule(Some("B36/S23"), Tiling::Square).is_ok());
        assert!(parse_rule(Some("B7/S"), Tiling::Hexagonal).unwrap_err().contains("6 neighbors"));
        assert!(parse_rule(Some("B0/S"), Tiling::Square).unwrap_err().contains("birth on 0"));
        assert!(parse_tiling("octagonal").is_err());
    }

    #[test]
    fn tilings_lists_every_grid_with_its_default_and_presets() {
        let info = tilings();
        assert_eq!(info.iter().map(|t| t.id).collect::<Vec<_>>(), ["square", "hexagonal", "triangular-12", "triangular-3"]);
        let hex = &info[1];
        assert_eq!((hex.neighbors, hex.default_rule.text.as_str(), hex.wrap_even_width, hex.wrap_even_height), (6, "B2/S35", false, true));
        assert_eq!(hex.presets[0].rule, "B2/S35");
        let json = serde_json::to_value(&info[0]).unwrap();
        assert_eq!(json["defaultRule"]["birth"], 8);
        assert_eq!(json["defaultRule"]["survival"], 12);
        assert_eq!(json["wrapEvenWidth"], false);
    }
}
