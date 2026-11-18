//! Tauri commands. Changes to the universe go through the simulation thread's
//! queue; read-only commands read the latest snapshot or the pending slot. → ADR-011

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use glife_engine::{Axis, Edges, PasteMode, Pattern, PlaceError, Rect, MAX_SIDE, MIN_SIDE};
use tauri::ipc::Response;
use tauri::State;

use serde::{Deserialize, Serialize};

use crate::frame;
use crate::library::builtin;
use crate::library::personal::Library;
use crate::library::Problem;
use crate::sim::{self, os_seed, Command, Latest};

/// A pattern waiting to be placed: its size, name, comment and notices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingInfo {
    pub width: u32,
    pub height: u32,
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
    /// Name and comment of the pending pattern.
    pub pending_meta: Mutex<(String, String)>,
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
            pending_meta: Mutex::new(Default::default()),
            file: Mutex::new(FileState::default()),
            builtin,
            builtin_problems,
            personal: Library::open(personal_dir)?,
            quitting: AtomicBool::new(false),
        })
    }

    /// Fills the pending slot, refusing a pattern larger than the grid.
    pub fn set_pending(&self, pattern: Pattern, name: String, comment: String, notices: Vec<String>) -> Result<PendingInfo, String> {
        let (w, h) = (pattern.width(), pattern.height());
        let (grid, grid_tiling) = {
            let snapshot = self.latest.lock().unwrap().clone();
            ((snapshot.universe.width(), snapshot.universe.height()), snapshot.universe.tiling())
        };
        if w > grid.0 || h > grid.1 {
            return Err(PlaceError { pattern: (w, h), grid, pattern_tiling: pattern.tiling(), grid_tiling }.to_string());
        }
        *self.pending.lock().unwrap() = Some(Arc::new(pattern));
        *self.pending_meta.lock().unwrap() = (name.clone(), comment.clone());
        Ok(PendingInfo { width: w, height: h, name, comment, notices })
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

#[tauri::command]
pub fn new_universe(host: State<Host>, width: u32, height: u32, edges: String) -> Result<(), String> {
    let sides = MIN_SIDE..=MAX_SIDE;
    if !sides.contains(&width) || !sides.contains(&height) {
        return Err(format!("universe size {width} x {height} is outside {MIN_SIDE}..={MAX_SIDE} per side"));
    }
    *host.file.lock().unwrap() = FileState::default();
    host.send(Command::NewUniverse { width, height, edges: parse_edges(&edges)? })
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
    host.send(Command::SetEdges(parse_edges(&edges)?))
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

/// Stamps the pending pattern, flipped as asked, with its center cell
/// `(⌊w/2⌋, ⌊h/2⌋)` at `(x, y)`. `mode` is `"or"` or `"copy"`.
#[tauri::command]
pub fn place(host: State<Host>, x: i64, y: i64, flip_h: bool, flip_v: bool, mode: String) -> Result<(), String> {
    let mode = match mode.as_str() {
        "or" => PasteMode::Or,
        "copy" => PasteMode::Copy,
        other => return Err(format!("unknown paste mode {other:?}")),
    };
    let mut pattern = host.pending()?;
    if flip_h {
        pattern = Arc::new(pattern.flipped(Axis::Horizontal));
    }
    if flip_v {
        pattern = Arc::new(pattern.flipped(Axis::Vertical));
    }
    let (w, h) = (pattern.width(), pattern.height());
    let (grid, grid_tiling) = {
        let snapshot = host.latest.lock().unwrap().clone();
        ((snapshot.universe.width(), snapshot.universe.height()), snapshot.universe.tiling())
    };
    if w > grid.0 || h > grid.1 {
        return Err(PlaceError { pattern: (w, h), grid, pattern_tiling: pattern.tiling(), grid_tiling }.to_string());
    }
    host.send(Command::Place { pattern, x: x - (w / 2) as i64, y: y - (h / 2) as i64, mode })
}

/// The pending pattern as a pattern buffer.
#[tauri::command]
pub fn pending_cells(host: State<Host>) -> Result<Response, String> {
    let mut words = Vec::new();
    host.pending()?.to_words(&mut words);
    Ok(Response::new(words.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>()))
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
