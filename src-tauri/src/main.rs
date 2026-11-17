#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod frame;
#[allow(dead_code)] // wired to commands in WP8
mod glife_file;
mod history;
#[allow(dead_code)]
mod library;
#[allow(dead_code)]
mod open_rle;
#[allow(dead_code)]
mod pattern_file;
#[allow(dead_code)]
mod rle;
mod sim;

use std::sync::{mpsc, Mutex};

fn main() {
    let sim = sim::Sim::launch(std::env::var_os("GLIFE_BENCH").is_some_and(|v| v == "1"));
    let (queue, receiver) = mpsc::channel();
    let host = commands::Host { queue, latest: sim.latest(), pending: Mutex::new(None) };
    std::thread::Builder::new()
        .name("simulation".into())
        .spawn(move || sim::run(sim, receiver))
        .expect("spawn simulation thread");

    tauri::Builder::default()
        .manage(host)
        .invoke_handler(tauri::generate_handler![
            commands::new_universe,
            commands::play,
            commands::pause,
            commands::step,
            commands::set_speed,
            commands::set_edges,
            commands::edit,
            commands::undo,
            commands::redo,
            commands::delete_selection,
            commands::flip_selection,
            commands::place,
            commands::pending_cells,
            commands::clear,
            commands::random_fill,
            commands::frame,
        ])
        .run(tauri::generate_context!())
        .expect("error while running glife");
}
