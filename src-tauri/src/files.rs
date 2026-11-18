//! Saving, opening, importing and exporting, and the unsaved-changes prompt.
//! The logic sits in functions that take paths and the `Host`; the commands
//! around them only run dialogs.

use std::path::{Path, PathBuf};

use glife_engine::{Pattern, Rect, Rule};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath, MessageDialogButtons, MessageDialogKind, MessageDialogResult};

use crate::clipboard::{pending_from_file, pending_from_rle, selection_pattern, source_pattern, Source, Sourced};
use crate::commands::{Host, PendingInfo, RectArg};
use crate::glife_file;
use crate::open_rle::universe_from_rle;
use crate::pattern_file::PatternFile;
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
    pending_from_rle(host, &text)
}

/// Reads a pattern file into the pending slot.
pub fn import_pattern_from(host: &Host, path: &Path) -> Result<PendingInfo, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read the file: {e}."))?;
    pending_from_file(host, PatternFile::parse(&text)?)
}

/// What an export writes, before it is turned into RLE or a pattern file.
struct Exported {
    pattern: Pattern,
    rule: Rule,
    name: Option<String>,
    comment: Option<String>,
    bounded: Option<Bounded>,
    /// The file name without its extension.
    stem: String,
}

fn export_parts(host: &Host, source: &ExportSource) -> Result<Exported, String> {
    let named = |s: Sourced, stem: &str| Exported {
        pattern: s.pattern,
        rule: s.rule,
        comment: Some(s.comment).filter(|c| !c.is_empty()),
        bounded: None,
        stem: stem.into(),
        name: Some(s.name),
    };
    match source {
        ExportSource::Selection(rect) => {
            let (pattern, rule) = selection_pattern(host, (*rect).into())?;
            Ok(Exported { pattern, rule, name: None, comment: None, bounded: None, stem: "selection".into() })
        }
        ExportSource::Universe => {
            let snapshot = host.latest.lock().unwrap().clone();
            let u = &snapshot.universe;
            let kind = match u.edges() {
                glife_engine::Edges::Wrap => 'T',
                glife_engine::Edges::Dead => 'P',
            };
            let pattern = u.extract(Rect { x: 0, y: 0, w: u.width(), h: u.height() });
            let bounded = Some(Bounded { kind, width: u.width(), height: u.height() });
            let file = host.file.lock().unwrap();
            let stem = file.path.as_deref().map(stem).or_else(|| file.suggested.clone()).unwrap_or_else(|| "universe".into());
            Ok(Exported { pattern, rule: u.rule(), name: None, comment: None, bounded, stem })
        }
        ExportSource::Builtin(id) => {
            let s = source_pattern(host, &Source::Builtin(id.clone()))?;
            let stem = s.name.clone();
            Ok(named(s, &stem))
        }
        ExportSource::Personal(name) => {
            let s = source_pattern(host, &Source::Personal(name.clone()))?;
            let stem = s.name.clone();
            Ok(named(s, &stem))
        }
        ExportSource::Pending => {
            let s = source_pattern(host, &Source::Pending)?;
            if s.name.is_empty() {
                Ok(Exported {
                    comment: Some(s.comment).filter(|c| !c.is_empty()),
                    pattern: s.pattern,
                    rule: s.rule,
                    name: None,
                    bounded: None,
                    stem: "pattern".into(),
                })
            } else {
                let stem = s.name.clone();
                Ok(named(s, &stem))
            }
        }
    }
}

/// The RLE text to export, and the file name to offer. Square grid only.
pub fn export_text(host: &Host, source: &ExportSource) -> Result<(String, String), String> {
    let e = export_parts(host, source)?;
    rle::square_only(e.pattern.tiling())?;
    Ok((rle::write(&e.pattern, e.rule, e.name.as_deref(), e.comment.as_deref(), e.bounded), format!("{}.rle", e.stem)))
}

/// The pattern file text to export, and the file name to offer.
pub fn export_pattern_text(host: &Host, source: &ExportSource) -> Result<(String, String), String> {
    let e = export_parts(host, source)?;
    let name = e.name.unwrap_or_else(|| e.stem.clone());
    Ok((PatternFile::pattern_text(&e.pattern, e.rule, &name, e.comment.as_deref())?, format!("{}.toml", e.stem)))
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

/// Reads a glife pattern file into the pending slot; null if cancelled.
#[tauri::command]
pub async fn import_pattern(app: AppHandle, host: State<'_, Host>) -> Result<Option<PendingInfo>, String> {
    let picked = app.dialog().file().add_filter("glife pattern", &["toml"]).blocking_pick_file();
    match path_of(picked)? {
        Some(path) => import_pattern_from(&host, &path).map(Some),
        None => Ok(None),
    }
}

/// Exports to a pattern file the operator names; does nothing if cancelled.
#[tauri::command]
pub async fn export_pattern(app: AppHandle, host: State<'_, Host>, source: ExportSource) -> Result<(), String> {
    let (text, default_name) = export_pattern_text(&host, &source)?;
    let picked = app.dialog().file().add_filter("glife pattern", &["toml"]).set_file_name(default_name).blocking_save_file();
    if let Some(path) = path_of(picked)? {
        glife_file::write_atomic(&with_extension(path, "toml"), text.as_bytes())
            .map_err(|e| format!("Could not write the file: {e}."))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;

    use glife_engine::{Edges, Tiling, Universe};

    use super::*;
    use crate::clipboard::{paste_text, selection_text};
    use crate::sim::Sim;

    struct Rig {
        sim: Sim,
        host: Host,
        rx: Receiver<Command>,
        dir: tempfile::TempDir,
    }

    impl Rig {
        fn new() -> Rig {
            Rig::on(Tiling::Square, None)
        }

        /// A 16 x 16 universe of `tiling`, under `rule` or the grid's default.
        fn on(tiling: Tiling, rule: Option<&str>) -> Rig {
            let mut universe = Universe::new(16, 16, Edges::Wrap, tiling).unwrap();
            if let Some(rule) = rule {
                universe.set_rule(glife_engine::Rule::parse(rule, tiling).unwrap()).unwrap();
            }
            let sim = Sim::new(universe, Duration::ZERO);
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
        let text = selection_text(&rig.host, Rect { x: 2, y: 3, w: 5, h: 4 }).unwrap();
        let parsed = rle::parse(&text).unwrap();
        assert_eq!((parsed.pattern.width(), parsed.pattern.height(), parsed.pattern.live_count()), (5, 4, 0));
        assert!(selection_text(&rig.host, Rect { x: 14, y: 0, w: 5, h: 1 }).is_err());
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
        let pattern = Pattern::new(2, 2, Tiling::Square, false);
        rig.host.personal.save("Mine", None, &pattern, Tiling::Square.default_rule(), false).unwrap();
        let taken = rig.host.personal.save("mine", None, &pattern, Tiling::Square.default_rule(), false);
        assert_eq!(crate::library_cmds::nothing(taken).unwrap(), serde_json::json!({ "nameTaken": true }));
    }

    fn sample(tiling: Tiling, offset: bool) -> Pattern {
        let mut p = Pattern::new(5, 4, tiling, offset);
        for (x, y) in [(0, 0), (2, 1), (4, 3), (1, 2)] {
            p.set(x, y, true);
        }
        p
    }

    #[test]
    fn copy_writes_rle_on_the_square_grid_and_pattern_text_elsewhere() {
        let rect = Rect { x: 1, y: 1, w: 4, h: 3 };
        let square = Rig::new();
        let text = selection_text(&square.host, rect).unwrap();
        assert!(text.starts_with("x = 4, y = 3, rule = B3/S23"), "{text}");

        let hex = Rig::on(Tiling::Hexagonal, Some("B3/S23"));
        let text = selection_text(&hex.host, rect).unwrap();
        assert!(text.contains("tiling = \"hexagonal\"") && text.contains("offset = 1") && text.contains("rule = B3/S23"), "{text}");
        let info = paste_text(&hex.host, &text).unwrap();
        assert_eq!((info.width, info.height, info.tiling, info.rule.as_str()), (4, 3, "hexagonal", "B3/S23"));
        assert!(info.notices.is_empty());
    }

    #[test]
    fn a_pattern_under_another_rule_of_the_grid_pastes_with_a_notice_naming_it() {
        let rig = Rig::on(Tiling::Hexagonal, None);
        let text = PatternFile::pattern_text(&sample(Tiling::Hexagonal, false), glife_engine::Rule::parse("B3/S23", Tiling::Hexagonal).unwrap(), "Mine", None).unwrap();
        let info = paste_text(&rig.host, &text).unwrap();
        assert_eq!(info.name, "Mine");
        assert_eq!(info.notices.len(), 1);
        assert!(info.notices[0].contains("B3/S23") && info.notices[0].contains("B2/S35"), "{:?}", info.notices);
        assert_eq!(rig.host.pending_meta.lock().unwrap().2.to_string(), "B3/S23");

        let own = PatternFile::pattern_text(&sample(Tiling::Hexagonal, false), Tiling::Hexagonal.default_rule(), "Own", None).unwrap();
        assert!(paste_text(&rig.host, &own).unwrap().notices.is_empty());

        // RLE on a square universe under another rule gets the same notice.
        let square = Rig::new();
        let info = paste_text(&square.host, "x = 3, y = 1, rule = B36/S23\n3o!\n").unwrap();
        assert_eq!(info.notices.len(), 1);
        assert!(info.notices[0].contains("B36/S23"));
        assert!(paste_text(&square.host, "x = 3, y = 1, rule = B3/S23\n3o!\n").unwrap().notices.is_empty());
    }

    #[test]
    fn a_pattern_of_another_grid_and_rle_off_the_square_grid_are_refused() {
        let triangular = Rig::on(Tiling::Triangular12, None);
        let text = PatternFile::pattern_text(&sample(Tiling::Hexagonal, false), Tiling::Hexagonal.default_rule(), "Hex", None).unwrap();
        let err = paste_text(&triangular.host, &text).unwrap_err();
        assert!(err.contains("hexagonal") && err.contains("triangular-12"), "{err}");
        assert!(triangular.host.pending.lock().unwrap().is_none());

        let hex = Rig::on(Tiling::Hexagonal, None);
        let err = paste_text(&hex.host, "x = 3, y = 1, rule = B3/S23\n3o!\n").unwrap_err();
        assert!(err.contains("RLE") && err.contains("square"), "{err}");
        assert!(hex.host.pending.lock().unwrap().is_none());
        let path = hex.dir.path().join("a.rle");
        std::fs::write(&path, "x = 3, y = 1, rule = B3/S23\n3o!\n").unwrap();
        assert!(import_from(&hex.host, &path).is_err());
        assert!(export_text(&hex.host, &ExportSource::Selection(RectArg { x: 0, y: 0, w: 3, h: 3 })).unwrap_err().contains("RLE"));

        // Begin place of a library entry of another grid is refused.
        let entry = hex.host.builtin.iter().find(|e| e.pattern.tiling() == Tiling::Square).unwrap();
        let s = source_pattern(&hex.host, &Source::Builtin(entry.id.clone())).unwrap();
        let err = hex.host.set_pending(s.pattern, s.rule, s.name, s.comment, Vec::new()).unwrap_err();
        assert!(err.contains("square") && err.contains("hexagonal"), "{err}");
    }

    #[test]
    fn export_then_import_of_a_triangular_pattern_file_keeps_everything() {
        let mut rig = Rig::on(Tiling::Triangular3, None);
        let pattern = sample(Tiling::Triangular3, true);
        let rule = glife_engine::Rule::parse("B2/S23", Tiling::Triangular3).unwrap();
        rig.send(Command::SetRule(rule));
        rig.host.personal.save("Odd one", Some("first\nsecond"), &pattern, rule, false).unwrap();

        let (text, file_name) = export_pattern_text(&rig.host, &ExportSource::Personal("odd one".into())).unwrap();
        assert_eq!(file_name, "Odd one.toml");
        let path = rig.dir.path().join(&file_name);
        glife_file::write_atomic(&path, text.as_bytes()).unwrap();

        let info = import_pattern_from(&rig.host, &path).unwrap();
        assert_eq!((info.name.as_str(), info.comment.as_str(), info.tiling, info.rule.as_str()), ("Odd one", "first\nsecond", "triangular-3", "B2/S23"));
        assert!(info.notices.is_empty());
        assert_eq!(*rig.host.pending().unwrap(), pattern);

        // A selection exports as a pattern file too, and its text is pattern text.
        let (text, name) = export_pattern_text(&rig.host, &ExportSource::Selection(RectArg { x: 1, y: 2, w: 3, h: 3 })).unwrap();
        assert_eq!(name, "selection.toml");
        let file = PatternFile::parse(&text).unwrap();
        assert_eq!((file.tiling.as_str(), file.offset), ("triangular-3", 1));
    }

    #[test]
    fn personal_patterns_keep_grid_offset_and_rule_and_names_are_unique_across_grids() {
        let rig = Rig::new();
        let hex = sample(Tiling::Hexagonal, true);
        let rule = glife_engine::Rule::parse("B3/S23", Tiling::Hexagonal).unwrap();
        rig.host.personal.save("Shared", None, &hex, rule, false).unwrap();
        let square = sample(Tiling::Square, false);
        assert_eq!(rig.host.personal.save("shared", None, &square, Tiling::Square.default_rule(), false), Err(crate::library::personal::Error::NameTaken));
        let listed = crate::library_cmds::list(&rig.host);
        let entry = &listed["personal"][0];
        assert_eq!((entry["tiling"].as_str(), entry["rule"].as_str()), (Some("hexagonal"), Some("B3/S23")));
        let s = source_pattern(&rig.host, &Source::Personal("Shared".into())).unwrap();
        assert_eq!((s.pattern, s.rule), (hex, rule));
        let builtin = &listed["builtin"];
        assert!(builtin.as_array().unwrap().iter().all(|e| e["tiling"].is_string() && e["rule"].is_string()));
    }
}
