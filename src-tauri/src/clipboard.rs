//! Copy, cut and paste through the system clipboard, and the pending-pattern
//! commands that fill the slot from the clipboard or a library.

use glife_engine::{Pattern, Rect};
use serde::Deserialize;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::commands::{Host, PendingInfo, RectArg};
use crate::rle;
use crate::sim::Command;

/// Where a pattern comes from.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Builtin(String),
    Personal(String),
    Pending,
}

/// The RLE text of the selected rectangle of the latest snapshot, dead margin
/// included.
pub fn selection_rle(host: &Host, rect: Rect) -> Result<String, String> {
    Ok(rle::write(&selection_pattern(host, rect)?, None, None, None))
}

/// The cells of the selected rectangle in the latest snapshot.
pub fn selection_pattern(host: &Host, rect: Rect) -> Result<Pattern, String> {
    let rect = host.selection(rect)?;
    let snapshot = host.latest.lock().unwrap().clone();
    Ok(snapshot.universe.extract(rect))
}

/// Parses clipboard text and fills the pending slot.
pub fn paste_text(host: &Host, text: &str) -> Result<PendingInfo, String> {
    let parsed = rle::parse(text)?;
    host.set_pending(parsed.pattern, parsed.name.unwrap_or_default(), parsed.comment.unwrap_or_default(), parsed.notices)
}

/// A library pattern, or the pending one, with its name and comment.
pub fn source_pattern(host: &Host, source: &Source) -> Result<(Pattern, String, String), String> {
    match source {
        Source::Builtin(id) => host
            .builtin
            .iter()
            .find(|e| &e.id == id)
            .map(|e| (e.pattern.clone(), e.name.clone(), e.description.clone()))
            .ok_or_else(|| format!("There is no built-in pattern {id:?}.")),
        Source::Personal(name) => {
            let wanted = name.to_lowercase();
            host.personal
                .list()
                .0
                .into_iter()
                .find(|e| e.name.to_lowercase() == wanted)
                .map(|e| (e.pattern, e.name, e.comment.unwrap_or_default()))
                .ok_or_else(|| format!("There is no pattern {name:?} in your library."))
        }
        Source::Pending => {
            let pattern = host.pending()?;
            let (name, comment) = host.pending_meta.lock().unwrap().clone();
            Ok(((*pattern).clone(), name, comment))
        }
    }
}

#[tauri::command]
pub fn copy_selection(app: AppHandle, host: State<Host>, x: u32, y: u32, w: u32, h: u32) -> Result<(), String> {
    let text = selection_rle(&host, RectArg { x, y, w, h }.into())?;
    app.clipboard().write_text(text).map_err(|e| format!("Could not write to the clipboard: {e}"))
}

/// Copies, then kills the selection.
#[tauri::command]
pub fn cut_selection(app: AppHandle, host: State<Host>, x: u32, y: u32, w: u32, h: u32) -> Result<(), String> {
    let rect: Rect = RectArg { x, y, w, h }.into();
    let text = selection_rle(&host, rect)?;
    app.clipboard().write_text(text).map_err(|e| format!("Could not write to the clipboard: {e}"))?;
    host.send(Command::KillRect(rect))
}

#[tauri::command]
pub fn paste(app: AppHandle, host: State<Host>) -> Result<PendingInfo, String> {
    let text = app.clipboard().read_text().map_err(|_| "The clipboard holds no text.".to_string())?;
    paste_text(&host, &text)
}

#[tauri::command]
pub fn begin_place(host: State<Host>, source: Source) -> Result<PendingInfo, String> {
    let (pattern, name, comment) = source_pattern(&host, &source)?;
    host.set_pending(pattern, name, comment, Vec::new())
}
