//! Copy, cut and paste through the system clipboard (RLE on the square grid,
//! pattern text on the others), and the pending-pattern
//! commands that fill the slot from the clipboard or a library.

use glife_engine::{Pattern, Rect, Rule, Tiling};
use serde::Deserialize;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::commands::{Host, PendingInfo, RectArg};
use crate::pattern_file::PatternFile;
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

/// A library pattern, or the pending one, with its rule, name and comment.
pub struct Sourced {
    pub pattern: Pattern,
    pub rule: Rule,
    pub name: String,
    pub comment: String,
}

/// The cells of the selected rectangle in the latest snapshot, and the
/// universe's rule.
pub fn selection_pattern(host: &Host, rect: Rect) -> Result<(Pattern, Rule), String> {
    let rect = host.selection(rect)?;
    let snapshot = host.latest.lock().unwrap().clone();
    Ok((snapshot.universe.extract(rect), snapshot.universe.rule()))
}

/// What copy puts on the clipboard for the selected rectangle of the latest
/// snapshot, dead margin included: RLE on the square grid, pattern text on
/// the others.
pub fn selection_text(host: &Host, rect: Rect) -> Result<String, String> {
    let (pattern, rule) = selection_pattern(host, rect)?;
    if pattern.tiling() == Tiling::Square {
        Ok(rle::write(&pattern, rule, None, None, None))
    } else {
        PatternFile::pattern_text(&pattern, rule, "", None)
    }
}

/// Reads RLE text into the pending slot. Refused off the square grid. A rule
/// that is not in `B…/S…` notation loads with a notice and the grid's default.
pub fn pending_from_rle(host: &Host, text: &str) -> Result<PendingInfo, String> {
    let parsed = rle::parse(text)?;
    let (tiling, _) = host.grid();
    rle::square_only(tiling)?;
    let mut notices = parsed.notices;
    let rule = rle::rule_of(&parsed.rule, tiling).unwrap_or_else(|_| {
        let default = tiling.default_rule();
        notices.push(format!("The file's rule is {}, which glife cannot read. Its cells were loaded as {default}.", parsed.rule));
        default
    });
    host.set_pending(parsed.pattern, rule, parsed.name.unwrap_or_default(), parsed.comment.unwrap_or_default(), notices)
}

/// Reads a pattern file's cells into the pending slot.
pub fn pending_from_file(host: &Host, file: PatternFile) -> Result<PendingInfo, String> {
    let (pattern, rule) = file.cells()?;
    host.set_pending(pattern, rule, file.name, file.comment.unwrap_or_default(), Vec::new())
}

/// Parses clipboard text, pattern text or RLE, and fills the pending slot.
pub fn paste_text(host: &Host, text: &str) -> Result<PendingInfo, String> {
    match PatternFile::parse_pattern_text(text)? {
        Some(file) => pending_from_file(host, file),
        None => pending_from_rle(host, text),
    }
}

pub fn source_pattern(host: &Host, source: &Source) -> Result<Sourced, String> {
    match source {
        Source::Builtin(id) => host
            .builtin
            .iter()
            .find(|e| &e.id == id)
            .map(|e| Sourced { pattern: e.pattern.clone(), rule: e.rule, name: e.name.clone(), comment: e.description.clone() })
            .ok_or_else(|| format!("There is no built-in pattern {id:?}.")),
        Source::Personal(name) => {
            let wanted = name.to_lowercase();
            host.personal
                .list()
                .0
                .into_iter()
                .find(|e| e.name.to_lowercase() == wanted)
                .map(|e| Sourced { pattern: e.pattern, rule: e.rule, name: e.name, comment: e.comment.unwrap_or_default() })
                .ok_or_else(|| format!("There is no pattern {name:?} in your library."))
        }
        Source::Pending => {
            let pattern = host.pending()?;
            let (name, comment, rule) = host.pending_meta.lock().unwrap().clone();
            Ok(Sourced { pattern: (*pattern).clone(), rule, name, comment })
        }
    }
}

#[tauri::command]
pub fn copy_selection(app: AppHandle, host: State<Host>, x: u32, y: u32, w: u32, h: u32) -> Result<(), String> {
    let text = selection_text(&host, RectArg { x, y, w, h }.into())?;
    app.clipboard().write_text(text).map_err(|e| format!("Could not write to the clipboard: {e}"))
}

/// Copies, then kills the selection.
#[tauri::command]
pub fn cut_selection(app: AppHandle, host: State<Host>, x: u32, y: u32, w: u32, h: u32) -> Result<(), String> {
    let rect: Rect = RectArg { x, y, w, h }.into();
    let text = selection_text(&host, rect)?;
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
    let s = source_pattern(&host, &source)?;
    host.set_pending(s.pattern, s.rule, s.name, s.comment, Vec::new())
}
