//! Tauri commands. Each forwards to the simulation thread's queue, except
//! `frame`, which reads the latest snapshot.

use std::sync::mpsc::Sender;

use glife_engine::{Edges, MAX_SIDE, MIN_SIDE};
use tauri::ipc::Response;
use tauri::State;

use crate::frame;
use crate::sim::{os_seed, Command, Latest};

pub struct Host {
    pub queue: Sender<Command>,
    pub latest: Latest,
}

impl Host {
    fn send(&self, command: Command) -> Result<(), String> {
        self.queue.send(command).map_err(|_| "simulation thread stopped".to_string())
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

#[tauri::command]
pub fn edit(host: State<Host>, cells: Vec<(u32, u32)>, alive: bool) -> Result<(), String> {
    host.send(Command::Edit { cells, alive })
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
