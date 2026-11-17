//! Saving, opening, importing and exporting, and the unsaved-changes prompt.
//! The logic sits in functions that take paths and the `Host`; the commands
//! around them only run dialogs.

use std::path::{Path, PathBuf};

use glife_engine::{Pattern, Rect};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath, MessageDialogButtons, MessageDialogKind, MessageDialogResult};

use crate::clipboard::{selection_rle, source_pattern, Source};
use crate::commands::{Host, PendingInfo, RectArg};
use crate::glife_file;
use crate::open_rle::universe_from_rle;
use crate::rle::{self, Bounded};
use crate::sim::Command;

const MAGIC: &[u8] = b"GLIFE\0\r\n";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInfo {
    pub file_name: String,
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SelectionSource {
    Selection(RectArg),
    Pending,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportSource {
    Selection(RectArg),
    Universe,
    Builtin(String),
    Personal(String),
    Pending,
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Writes the latest snapshot to `path` and marks that revision saved.
pub fn save_to(host: &Host, path: &Path) -> Result<String, String> {
    let snapshot = host.latest.lock().unwrap().clone();
    glife_file::write(path, &snapshot.universe)?;
    host.send(Command::MarkSaved { revision: snapshot.revision })?;
    let mut file = host.file.lock().unwrap();
    file.path = Some(path.to_path_buf());
    file.suggested = None;
    Ok(file_name(path))
}

/// Replaces the universe with the glife or RLE file at `path`. The new
/// universe starts saved: `Load` gives it a clean mark.
pub fn open_from(host: &Host, path: &Path) -> Result<OpenInfo, String> {
    let data = std::fs::read(path).map_err(|e| format!("Could not read the file: {e}."))?;
    let (universe, notices, state) = if data.starts_with(MAGIC) {
        let state = crate::commands::FileState { path: Some(path.to_path_buf()), suggested: None };
        (glife_file::read(path)?, Vec::new(), state)
    } else {
        let text = String::from_utf8(data).map_err(|_| "This is not a glife or RLE file.".to_string())?;
        let parsed = rle::parse(&text)?;
        let (universe, notices) = universe_from_rle(&parsed)?;
        let suggested = parsed.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| stem(path));
        (universe, notices, crate::commands::FileState { path: None, suggested: Some(suggested) })
    };
    host.send(Command::Load(universe))?;
    *host.file.lock().unwrap() = state;
    Ok(OpenInfo { file_name: file_name(path), notices })
}

/// The file name Save As offers.
pub fn suggested_save_name(host: &Host) -> String {
    let file = host.file.lock().unwrap();
    match (&file.path, &file.suggested) {
        (Some(path), _) => file_name(path),
        (None, Some(name)) => format!("{name}.glife"),
        (None, None) => "Untitled.glife".into(),
    }
}

/// Reads an RLE file into the pending slot.
pub fn import_from(host: &Host, path: &Path) -> Result<PendingInfo, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read the file: {e}."))?;
    let parsed = rle::parse(&text)?;
    host.set_pending(parsed.pattern, parsed.name.unwrap_or_default(), parsed.comment.unwrap_or_default(), parsed.notices)
}

/// The RLE text to export, and the file name to offer.
pub fn export_text(host: &Host, source: &ExportSource) -> Result<(String, String), String> {
    let named = |pattern: &Pattern, name: &str, comment: &str| {
        let comment = Some(comment).filter(|c| !c.is_empty());
        (rle::write(pattern, Some(name), comment, None), format!("{name}.rle"))
    };
    match source {
        ExportSource::Selection(rect) => Ok((selection_rle(host, (*rect).into())?, "selection.rle".into())),
        ExportSource::Universe => {
            let snapshot = host.latest.lock().unwrap().clone();
            let u = &snapshot.universe;
            let kind = match u.edges() {
                glife_engine::Edges::Wrap => 'T',
                glife_engine::Edges::Dead => 'P',
            };
            let whole = u.extract(Rect { x: 0, y: 0, w: u.width(), h: u.height() });
            let bounded = Bounded { kind, width: u.width(), height: u.height() };
            let file = host.file.lock().unwrap();
            let name = file.path.as_deref().map(stem).or_else(|| file.suggested.clone()).unwrap_or_else(|| "universe".into());
            Ok((rle::write(&whole, None, None, Some(bounded)), format!("{name}.rle")))
        }
        ExportSource::Builtin(id) => {
            let (p, n, c) = source_pattern(host, &Source::Builtin(id.clone()))?;
            Ok(named(&p, &n, &c))
        }
        ExportSource::Personal(name) => {
            let (p, n, c) = source_pattern(host, &Source::Personal(name.clone()))?;
            Ok(named(&p, &n, &c))
        }
        ExportSource::Pending => {
            let (p, n, c) = source_pattern(host, &Source::Pending)?;
            if n.is_empty() {
                Ok((rle::write(&p, None, Some(&c).filter(|c| !c.is_empty()).map(String::as_str), None), "pattern.rle".into()))
            } else {
                Ok(named(&p, &n, &c))
            }
        }
    }
}

fn with_extension(path: PathBuf, extension: &str) -> PathBuf {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)) {
        path
    } else {
        let mut name = path.into_os_string();
        name.push(".");
        name.push(extension);
        PathBuf::from(name)
    }
}

fn path_of(picked: Option<FilePath>) -> Result<Option<PathBuf>, String> {
    picked.map(|p| p.into_path().map_err(|e| e.to_string())).transpose()
}

/// Save or Save As; `None` when the operator cancels the dialog.
fn save_flow(app: &AppHandle, host: &Host, ask: bool) -> Result<Option<String>, String> {
    let known = host.file.lock().unwrap().path.clone();
    let path = match known {
        Some(path) if !ask => path,
        _ => {
            let picked = app
                .dialog()
                .file()
                .add_filter("glife universe", &["glife"])
                .set_file_name(suggested_save_name(host))
                .blocking_save_file();
            match path_of(picked)? {
                Some(path) => with_extension(path, "glife"),
                None => return Ok(None),
            }
        }
    };
    save_to(host, &path).map(Some)
}

/// The unsaved-changes prompt; whether to go on.
pub fn confirm_discard_flow(app: &AppHandle, host: &Host) -> Result<bool, String> {
    if !host.latest.lock().unwrap().dirty {
        return Ok(true);
    }
    let name = {
        let file = host.file.lock().unwrap();
        match (&file.path, &file.suggested) {
            (Some(path), _) => file_name(path),
            (None, Some(name)) => name.clone(),
            (None, None) => "Untitled".into(),
        }
    };
    let result = app
        .dialog()
        .message(format!("Save changes to \"{name}\" before closing it? Your changes will be lost if you don't save them."))
        .title("glife")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNoCancelCustom("Save".into(), "Don't Save".into(), "Cancel".into()))
        .blocking_show_with_result();
    match result {
        MessageDialogResult::Yes => Ok(save_flow(app, host, false)?.is_some()),
        MessageDialogResult::No => Ok(true),
        MessageDialogResult::Custom(label) if label == "Save" => Ok(save_flow(app, host, false)?.is_some()),
        MessageDialogResult::Custom(label) if label == "Don't Save" => Ok(true),
        _ => Ok(false),
    }
}

#[tauri::command]
pub async fn confirm_discard(app: AppHandle, host: State<'_, Host>) -> Result<bool, String> {
    confirm_discard_flow(&app, &host)
}

#[tauri::command]
pub async fn save(app: AppHandle, host: State<'_, Host>) -> Result<Option<String>, String> {
    save_flow(&app, &host, false)
}

#[tauri::command]
pub async fn save_as(app: AppHandle, host: State<'_, Host>) -> Result<Option<String>, String> {
    save_flow(&app, &host, true)
}

/// Opens a glife or RLE file. The caller runs `confirm_discard` first.
#[tauri::command]
pub async fn open(app: AppHandle, host: State<'_, Host>) -> Result<Option<OpenInfo>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("glife and RLE files", &["glife", "rle", "txt"])
        .blocking_pick_file();
    match path_of(picked)? {
        Some(path) => open_from(&host, &path).map(Some),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn import_rle(app: AppHandle, host: State<'_, Host>) -> Result<Option<PendingInfo>, String> {
    let picked = app.dialog().file().add_filter("RLE pattern", &["rle", "txt"]).blocking_pick_file();
    match path_of(picked)? {
        Some(path) => import_from(&host, &path).map(Some),
        None => Ok(None),
    }
}

/// Exports to an RLE file the operator names; does nothing if cancelled.
#[tauri::command]
pub async fn export_rle(app: AppHandle, host: State<'_, Host>, source: ExportSource) -> Result<(), String> {
    let (text, default_name) = export_text(&host, &source)?;
    let picked = app.dialog().file().add_filter("RLE pattern", &["rle"]).set_file_name(default_name).blocking_save_file();
    if let Some(path) = path_of(picked)? {
        glife_file::write_atomic(&with_extension(path, "rle"), text.as_bytes())
            .map_err(|e| format!("Could not write the file: {e}."))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;

    use glife_engine::{Edges, Universe};

    use super::*;
    use crate::clipboard::paste_text;
    use crate::sim::Sim;

    struct Rig {
        sim: Sim,
        host: Host,
        rx: Receiver<Command>,
        dir: tempfile::TempDir,
    }

    impl Rig {
        fn new() -> Rig {
            let sim = Sim::new(Universe::new(16, 16, Edges::Wrap).unwrap(), Duration::ZERO);
            let (queue, rx) = mpsc::channel();
            let dir = tempfile::tempdir().unwrap();
            let builtin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("patterns");
            let host = Host::new(queue, sim.latest(), Some(&builtin), &dir.path().join("lib")).unwrap();
            Rig { sim, host, rx, dir }
        }

        /// Applies every queued command, as the simulation thread would.
        fn pump(&mut self) {
            while let Ok(command) = self.rx.try_recv() {
                self.sim.handle(command, Duration::ZERO);
            }
        }

        fn send(&mut self, command: Command) {
            self.sim.handle(command, Duration::ZERO);
        }

        fn snapshot(&self) -> std::sync::Arc<crate::sim::Snapshot> {
            self.host.latest.lock().unwrap().clone()
        }
    }

    #[test]
    fn save_then_open_gives_the_same_universe_and_a_clean_mark() {
        let mut rig = Rig::new();
        rig.send(Command::Edit { cells: vec![(5, 5), (6, 5), (7, 5)], alive: true, stroke: None });
        rig.send(Command::Step);
        let before = rig.snapshot();
        let path = rig.dir.path().join("a.glife");

        assert_eq!(save_to(&rig.host, &path).unwrap(), "a.glife");
        rig.pump();
        assert!(!rig.snapshot().dirty);

        rig.send(Command::Clear);
        assert!(rig.snapshot().dirty);
        let info = open_from(&rig.host, &path).unwrap();
        rig.pump();
        assert_eq!(info.file_name, "a.glife");
        let after = rig.snapshot();
        assert!(!after.dirty);
        let (a, b) = (&before.universe, &after.universe);
        assert_eq!((a.width(), a.height(), a.generation()), (b.width(), b.height(), b.generation()));
        for y in 0..a.height() {
            for x in 0..a.width() {
                assert_eq!((a.get(x, y), a.get(x, y).then(|| a.age(x, y))), (b.get(x, y), b.get(x, y).then(|| b.age(x, y))));
            }
        }
        assert_eq!(suggested_save_name(&rig.host), "a.glife");
    }

    #[test]
    fn save_as_on_an_rle_opened_universe_suggests_the_pattern_name() {
        let rig = Rig::new();
        let named = rig.dir.path().join("file-name.rle");
        std::fs::write(&named, "#N Fancy\nx = 3, y = 1, rule = B3/S23\n3o!\n").unwrap();
        let info = open_from(&rig.host, &named).unwrap();
        assert_eq!(info.file_name, "file-name.rle");
        assert_eq!(suggested_save_name(&rig.host), "Fancy.glife");

        let unnamed = rig.dir.path().join("plain.rle");
        std::fs::write(&unnamed, "x = 3, y = 1, rule = B3/S23\n3o!\n").unwrap();
        open_from(&rig.host, &unnamed).unwrap();
        assert_eq!(suggested_save_name(&rig.host), "plain.glife");
    }

    #[test]
    fn a_failed_open_leaves_the_universe_and_file_state() {
        let mut rig = Rig::new();
        let bad = rig.dir.path().join("bad.glife");
        std::fs::write(&bad, b"GLIFE\0\r\ntruncated").unwrap();
        assert!(open_from(&rig.host, &bad).is_err());
        rig.pump();
        assert_eq!(rig.snapshot().universe.width(), 16);
        assert!(rig.host.file.lock().unwrap().path.is_none());
    }

    #[test]
    fn a_paste_larger_than_the_grid_names_both_sizes() {
        let rig = Rig::new();
        let err = paste_text(&rig.host, "x = 20, y = 3, rule = B3/S23\n20o$20o$20o!\n").unwrap_err();
        assert!(err.contains("20") && err.contains('3') && err.contains("16"), "{err}");
        assert!(rig.host.pending.lock().unwrap().is_none());
        let info = paste_text(&rig.host, "forum text\nx = 3, y = 1, rule = B3/S23\n3o!\n").unwrap();
        assert_eq!((info.width, info.height), (3, 1));
        assert!(paste_text(&rig.host, "no pattern here").is_err());
    }

    #[test]
    fn copying_an_empty_selection_gives_an_rle_of_its_size() {
        let rig = Rig::new();
        let text = selection_rle(&rig.host, Rect { x: 2, y: 3, w: 5, h: 4 }).unwrap();
        let parsed = rle::parse(&text).unwrap();
        assert_eq!((parsed.pattern.width(), parsed.pattern.height(), parsed.pattern.live_count()), (5, 4, 0));
        assert!(selection_rle(&rig.host, Rect { x: 14, y: 0, w: 5, h: 1 }).is_err());
    }

    #[test]
    fn exports_carry_the_grid_name_and_comment() {
        let rig = Rig::new();
        let (text, name) = export_text(&rig.host, &ExportSource::Universe).unwrap();
        assert!(text.contains(":T16,16"), "{text}");
        assert_eq!(name, "universe.rle");

        let id = rig.host.builtin[0].id.clone();
        let (text, name) = export_text(&rig.host, &ExportSource::Builtin(id)).unwrap();
        let parsed = rle::parse(&text).unwrap();
        assert_eq!(name, format!("{}.rle", parsed.name.as_deref().unwrap()));
        assert!(parsed.comment.is_some());
    }

    #[test]
    fn the_builtin_folder_loads_without_problems() {
        let rig = Rig::new();
        assert!(rig.host.builtin.len() >= 25);
        assert!(rig.host.builtin_problems.is_empty(), "{:?}", rig.host.builtin_problems);
        let listed = crate::library_cmds::list(&rig.host);
        assert_eq!(listed["builtin"].as_array().unwrap().len(), rig.host.builtin.len());
    }

    #[test]
    fn a_taken_library_name_is_reported_not_raised() {
        let rig = Rig::new();
        let pattern = Pattern::new(2, 2);
        rig.host.personal.save("Mine", None, &pattern, false).unwrap();
        let taken = rig.host.personal.save("mine", None, &pattern, false);
        assert_eq!(crate::library_cmds::nothing(taken).unwrap(), serde_json::json!({ "nameTaken": true }));
    }
}
