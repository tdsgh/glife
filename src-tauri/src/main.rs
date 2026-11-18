#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod commands;
mod files;
mod frame;
mod glife_file;
mod history;
mod library;
mod library_cmds;
mod menu;
mod open_rle;
mod pattern_file;
mod rle;
mod sim;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use tauri::{Manager, RunEvent, WindowEvent};

/// The built-in pattern folder: the bundled resource, then next to the
/// executable (an unbundled release build), then the source folder.
fn builtin_dir(app: &tauri::App) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join("patterns"));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.join("patterns"))) {
        candidates.push(dir);
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("patterns"));
    candidates.into_iter().find(|dir| dir.is_dir())
}

fn main() {
    let bench = sim::parse_bench(std::env::var("GLIFE_BENCH").ok().as_deref()).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let sim = sim::Sim::launch(bench);
    let (queue, receiver) = mpsc::channel();
    let latest = sim.latest();
    std::thread::Builder::new()
        .name("simulation".into())
        .spawn(move || sim::run(sim, receiver))
        .expect("spawn simulation thread");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let builtin = builtin_dir(app);
            eprintln!("built-in patterns: {}", builtin.as_deref().map_or("not found".into(), |p| p.display().to_string()));
            let personal = app.path().app_data_dir()?.join("patterns");
            app.manage(commands::Host::new(queue, latest, builtin.as_deref(), &personal)?);
            app.set_menu(menu::build(app.handle())?)?;
            Ok(())
        })
        .on_menu_event(|app, event| menu::on_event(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if !window.state::<commands::Host>().quitting.load(Ordering::SeqCst) {
                    api.prevent_close();
                    menu::request_quit(window.app_handle().clone());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::tilings,
            commands::new_universe,
            commands::play,
            commands::pause,
            commands::step,
            commands::set_speed,
            commands::set_edges,
            commands::set_rule,
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
            clipboard::copy_selection,
            clipboard::cut_selection,
            clipboard::paste,
            clipboard::begin_place,
            library_cmds::library_list,
            library_cmds::pattern_preview,
            library_cmds::library_save,
            library_cmds::library_copy_builtin,
            library_cmds::library_rename,
            library_cmds::library_set_comment,
            library_cmds::library_delete,
            library_cmds::library_show_folder,
            files::confirm_discard,
            files::save,
            files::save_as,
            files::open,
            files::import_rle,
            files::export_rle,
            files::import_pattern,
            files::export_pattern,
            menu::set_edit_target,
            menu::set_menu_state,
        ])
        .build(tauri::generate_context!())
        .expect("error while building glife")
        .run(|app, event| {
            // The OS asked to quit (Cmd+Q from the Dock, logout): run the prompt first.
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                if !app.state::<commands::Host>().quitting.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    menu::request_quit(app.clone());
                }
            }
        });
}
