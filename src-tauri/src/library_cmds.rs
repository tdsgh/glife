//! Commands for the built-in and personal pattern libraries.

use glife_engine::Pattern;
use serde_json::{json, Value};
use tauri::ipc::Response;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

use crate::clipboard::{selection_pattern, source_pattern, Source};
use crate::commands::Host;
use crate::files::SelectionSource;
use crate::library::personal::Error;

const PREVIEW_SIDE: u32 = 64;

pub fn nothing(result: Result<(), Error>) -> Result<Value, String> {
    match result {
        Ok(()) => Ok(Value::Null),
        Err(Error::NameTaken) => Ok(json!({ "nameTaken": true })),
        Err(e) => Err(e.to_string()),
    }
}

pub fn list(host: &Host) -> Value {
    let builtin: Vec<Value> = host
        .builtin
        .iter()
        .map(|e| {
            json!({
                "id": e.id, "name": e.name, "category": e.category.as_str(), "description": e.description,
                "width": e.pattern.width(), "height": e.pattern.height(), "live": e.live,
                "period": e.period, "speed": e.speed,
            })
        })
        .collect();
    let (personal, personal_problems) = host.personal.list();
    let personal: Vec<Value> = personal
        .iter()
        .map(|e| {
            json!({
                "name": e.name, "comment": e.comment.clone().unwrap_or_default(),
                "width": e.pattern.width(), "height": e.pattern.height(), "live": e.live,
                "created": e.created.clone().unwrap_or_default(), "changed": e.changed.clone().unwrap_or_default(),
            })
        })
        .collect();
    let problems: Vec<Value> = host
        .builtin_problems
        .iter()
        .chain(&personal_problems)
        .map(|p| json!({ "file": p.file, "reason": p.reason }))
        .collect();
    json!({ "builtin": builtin, "personal": personal, "problems": problems })
}

/// The pattern a save to the library takes.
fn save_source(host: &Host, source: SelectionSource) -> Result<Pattern, String> {
    match source {
        SelectionSource::Selection(rect) => selection_pattern(host, rect.into()),
        SelectionSource::Pending => Ok(source_pattern(host, &Source::Pending)?.0),
    }
}

#[tauri::command]
pub fn library_list(host: State<Host>) -> Value {
    list(&host)
}

/// A pattern buffer of at most 64 x 64 cells.
#[tauri::command]
pub fn pattern_preview(host: State<Host>, source: Source) -> Result<Response, String> {
    let (pattern, _, _) = source_pattern(&host, &source)?;
    let mut words = Vec::new();
    pattern.thumbnail(PREVIEW_SIDE).to_words(&mut words);
    Ok(Response::new(words.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>()))
}

#[tauri::command]
pub fn library_save(host: State<Host>, source: SelectionSource, name: String, comment: Option<String>, replace: bool) -> Result<Value, String> {
    let pattern = save_source(&host, source)?;
    nothing(host.personal.save(&name, comment.as_deref(), &pattern, replace))
}

#[tauri::command]
pub fn library_copy_builtin(host: State<Host>, id: String, name: String, replace: bool) -> Result<Value, String> {
    let (pattern, _, description) = source_pattern(&host, &Source::Builtin(id))?;
    nothing(host.personal.save(&name, Some(&description), &pattern, replace))
}

#[tauri::command]
pub fn library_rename(host: State<Host>, name: String, new_name: String, replace: bool) -> Result<Value, String> {
    nothing(host.personal.rename(&name, &new_name, replace))
}

#[tauri::command]
pub fn library_set_comment(host: State<Host>, name: String, comment: String) -> Result<(), String> {
    host.personal.set_comment(&name, &comment).map_err(|e| e.to_string())
}

/// Asks first; whether the pattern was deleted.
#[tauri::command]
pub async fn library_delete(app: AppHandle, host: State<'_, Host>, name: String) -> Result<bool, String> {
    let confirmed = app
        .dialog()
        .message(format!("Delete \"{name}\" from your library? This cannot be undone."))
        .title("glife")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Delete".into(), "Cancel".into()))
        .blocking_show();
    if !confirmed {
        return Ok(false);
    }
    host.personal.delete(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_show_folder(app: AppHandle, host: State<Host>) -> Result<(), String> {
    let dir = host.personal.dir().to_string_lossy().into_owned();
    app.opener().open_path(dir, None::<&str>).map_err(|e| format!("Could not open the folder: {e}"))
}
