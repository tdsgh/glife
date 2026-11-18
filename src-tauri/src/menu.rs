//! The menu bar, its events and the Edit-menu swap while a text field has focus.

use std::sync::Mutex;

use glife_engine::Tiling;
use serde::Deserialize;
use tauri::menu::{IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, State, Wry};

use crate::commands::Host;
use crate::files::confirm_discard_flow;

pub struct MenuState {
    app: AppHandle,
    edit: Submenu<Wry>,
    /// The Edit items for the universe, in order.
    universe_items: Vec<MenuItemKind<Wry>>,
    undo: Vec<MenuItem<Wry>>,
    redo: Vec<MenuItem<Wry>>,
    selection_items: Vec<MenuItem<Wry>>,
    /// Import RLE and Export RLE, which only the square grid can use.
    rle_items: Vec<MenuItem<Wry>>,
    text_target: Mutex<bool>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuFlags {
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_selection: bool,
    pub playing: bool,
    pub square: bool,
}

fn item(app: &AppHandle, id: &str, text: &str, accel: Option<&str>) -> tauri::Result<MenuItem<Wry>> {
    MenuItem::with_id(app, id, text, true, accel)
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let mac = cfg!(target_os = "macos");

    let new = item(app, "new", "New…", Some("CmdOrCtrl+N"))?;
    let open = item(app, "open", "Open…", Some("CmdOrCtrl+O"))?;
    let save = item(app, "save", "Save", Some("CmdOrCtrl+S"))?;
    let save_as = item(app, "save_as", "Save As…", Some("CmdOrCtrl+Shift+S"))?;
    let import = item(app, "import_rle", "Import RLE…", None)?;
    let export = item(app, "export_rle", "Export RLE…", None)?;
    let import_pattern = item(app, "import_pattern", "Import Pattern…", None)?;
    let export_pattern = item(app, "export_pattern", "Export Pattern…", None)?;
    let quit = item(app, "quit", if mac { "Quit glife" } else { "Quit" }, Some("CmdOrCtrl+Q"))?;
    let sep = || PredefinedMenuItem::separator(app);

    let undo = item(app, "undo", "Undo", Some("CmdOrCtrl+Z"))?;
    let redo = item(app, "redo", "Redo", Some("CmdOrCtrl+Shift+Z"))?;
    // Windows and Linux also take Ctrl+Y; a menu item holds one accelerator.
    let redo_alt = item(app, "redo_alt", "Redo (Ctrl+Y)", Some("Ctrl+Y"))?;
    let cut = item(app, "cut", "Cut", Some("CmdOrCtrl+X"))?;
    let copy = item(app, "copy", "Copy", Some("CmdOrCtrl+C"))?;
    let paste = item(app, "paste", "Paste", Some("CmdOrCtrl+V"))?;
    let delete = item(app, "delete", "Delete", None)?;
    let select_all = item(app, "select_all", "Select All", Some("CmdOrCtrl+A"))?;
    let flip_h = item(app, "flip_h", "Flip Horizontally", None)?;
    let flip_v = item(app, "flip_v", "Flip Vertically", None)?;

    let mut universe_items: Vec<MenuItemKind<Wry>> = vec![undo.kind(), redo.kind()];
    let mut redos = vec![redo];
    if !mac {
        universe_items.push(redo_alt.kind());
        redos.push(redo_alt);
    }
    universe_items.push(sep()?.kind());
    for i in [&cut, &copy, &paste, &delete] {
        universe_items.push(i.kind());
    }
    universe_items.push(sep()?.kind());
    universe_items.push(select_all.kind());
    universe_items.push(sep()?.kind());
    universe_items.push(flip_h.kind());
    universe_items.push(flip_v.kind());

    let edit = Submenu::with_id(app, "edit", "Edit", true)?;
    for i in &universe_items {
        edit.append(i)?;
    }

    let file = Submenu::with_id(app, "file", "File", true)?;
    file.append_items(&[&new, &open, &sep()?, &save, &save_as, &sep()?, &import, &export, &sep()?, &import_pattern, &export_pattern])?;

    let menu = Menu::new(app)?;
    if mac {
        let app_menu = Submenu::with_id(app, "app", "glife", true)?;
        app_menu.append_items(&[
            &PredefinedMenuItem::about(app, None, None)?,
            &sep()?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &sep()?,
            &quit,
        ])?;
        menu.append(&app_menu)?;
    } else {
        file.append_items(&[&sep()?, &quit])?;
    }
    menu.append_items(&[&file, &edit])?;
    if mac {
        let window = Submenu::with_id_and_items(
            app,
            "window",
            "Window",
            true,
            &[&PredefinedMenuItem::minimize(app, None)?, &PredefinedMenuItem::maximize(app, None)?],
        )?;
        menu.append(&window)?;
    }

    app.manage(MenuState {
        app: app.clone(),
        edit,
        universe_items,
        undo: vec![undo],
        redo: redos,
        selection_items: vec![cut, copy, delete, flip_h, flip_v],
        rle_items: vec![import, export],
        text_target: Mutex::new(false),
    });
    Ok(menu)
}

/// Forwards a click to the UI, or starts the quit flow.
pub fn on_event(app: &AppHandle, id: &str) {
    if id == "quit" {
        request_quit(app.clone());
    } else {
        let _ = app.emit("menu", if id == "redo_alt" { "redo" } else { id });
    }
}

/// Runs the unsaved-changes prompt off the main thread, then exits.
pub fn request_quit(app: AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let host = app.state::<Host>();
        match confirm_discard_flow(&app, &host) {
            Ok(true) => {
                host.quitting.store(true, std::sync::atomic::Ordering::SeqCst);
                app.exit(0);
            }
            Ok(false) => {}
            Err(e) => eprintln!("quit: {e}"),
        }
    });
}

fn clear_edit(edit: &Submenu<Wry>) -> tauri::Result<()> {
    for item in edit.items()? {
        edit.remove(&item)?;
    }
    Ok(())
}

impl MenuState {
    fn set_text_target(&self, text: bool) -> tauri::Result<()> {
        let mut current = self.text_target.lock().unwrap();
        if *current == text {
            return Ok(());
        }
        clear_edit(&self.edit)?;
        if text {
            let app = &self.app;
            self.edit.append_items(&[
                &PredefinedMenuItem::undo(app, None)?,
                &PredefinedMenuItem::redo(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::cut(app, None)?,
                &PredefinedMenuItem::copy(app, None)?,
                &PredefinedMenuItem::paste(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::select_all(app, None)?,
            ])?;
        } else {
            for i in &self.universe_items {
                self.edit.append(i)?;
            }
        }
        *current = text;
        Ok(())
    }

    fn set_flags(&self, f: MenuFlags) -> tauri::Result<()> {
        for i in &self.undo {
            i.set_enabled(f.can_undo && !f.playing)?;
        }
        for i in &self.redo {
            i.set_enabled(f.can_redo && !f.playing)?;
        }
        for i in &self.selection_items {
            i.set_enabled(f.has_selection)?;
        }
        for i in &self.rle_items {
            i.set_enabled(f.square)?;
        }
        Ok(())
    }
}

/// `"text"` swaps the Edit menu for the platform's text items; `"universe"` restores it.
#[tauri::command]
pub fn set_edit_target(menu: State<MenuState>, target: String) -> Result<(), String> {
    let text = match target.as_str() {
        "text" => true,
        "universe" => false,
        other => return Err(format!("unknown edit target {other:?}")),
    };
    menu.set_text_target(text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_menu_state(
    menu: State<MenuState>,
    can_undo: bool,
    can_redo: bool,
    has_selection: bool,
    playing: bool,
    tiling: Option<String>,
) -> Result<(), String> {
    let square = match tiling.as_deref() {
        None => true,
        Some(id) => Tiling::parse(id).ok_or_else(|| format!("unknown grid {id:?}"))? == Tiling::Square,
    };
    menu.set_flags(MenuFlags { can_undo, can_redo, has_selection, playing, square }).map_err(|e| e.to_string())
}
