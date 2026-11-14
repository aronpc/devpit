//! Assistant mode: an orchestrator that keeps the person's Schedules in view
//! and proposes new ones. Off by default and per orchestrator, kept in its
//! settings, out of its own chat's reach.
//!
//! On, devpit writes `.devpit/assistant.md` — its own, rewritten like
//! `.devpit/orchestrator.md` — imports it into the orchestrator's `CLAUDE.md`
//! beside that brief, and lists the assistant's MCP tools. Off, the import
//! and the file go, and the tool list is the one it always was.

use std::path::Path;

use devpit_core::Store;
use devpit_rpc::{AssistantMode, ErrorCode, Project, RpcError};
use tauri::AppHandle;

/// devpit's assistant brief, beside its orchestrator brief.
const BRIEF: (&str, &str) = (".devpit/assistant.md", include_str!("assistant_devpit.md"));
/// The line that imports it.
pub(crate) const IMPORT: &str = "@.devpit/assistant.md";
/// The line it goes under.
const BESIDE: &str = "@.devpit/orchestrator.md";
/// Its key among the orchestrator's settings.
const KEY: &str = "assistant";

/// Whether the orchestrator in `folder` has Assistant mode on.
pub(crate) fn on(store: &Store, folder: &Path) -> bool {
    store
        .orchestrator_settings(folder)
        .is_ok_and(|all| all[KEY].as_bool() == Some(true))
}

/// `CLAUDE.md` with the import under devpit's brief, or without it; every
/// other line as the person wrote it.
pub(crate) fn imported(text: &str, on: bool) -> String {
    let mut lines: Vec<&str> = text
        .split_inclusive('\n')
        .filter(|line| line.trim() != IMPORT)
        .collect();
    if !on {
        return lines.concat();
    }
    let line = format!("{IMPORT}\n");
    match lines.iter().position(|one| one.trim() == BESIDE) {
        Some(at) => {
            if !lines[at].ends_with('\n') {
                return format!("{}\n{line}", lines.concat());
            }
            lines.insert(at + 1, &line);
            lines.concat()
        }
        None if text.is_empty() || text.ends_with('\n') => format!("{}{line}", lines.concat()),
        None => format!("{}\n{line}", lines.concat()),
    }
}

/// Refuses to write `path` unless it is a file of `folder` itself: the
/// orchestrator's chat can put a link there, and devpit writes only its own.
pub(crate) fn inside(folder: &Path, path: &Path) -> std::io::Result<()> {
    let link = std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink());
    let parent = path.parent().unwrap_or(folder).canonicalize()?;
    if link || !parent.starts_with(folder.canonicalize()?) {
        return Err(std::io::Error::other(format!(
            "{} is not a file of the orchestrator's folder: left as it is",
            path.display()
        )));
    }
    Ok(())
}

/// The brief and its import as the mode says: written and imported when on,
/// taken out when off.
pub(crate) fn brief_in(folder: &Path, on: bool) -> std::io::Result<()> {
    let brief = folder.join(BRIEF.0);
    if on {
        let text = format!(
            "<!-- written by devpit {} -->\n{}",
            env!("CARGO_PKG_VERSION"),
            BRIEF.1
        );
        std::fs::create_dir_all(brief.parent().unwrap_or(folder))?;
        inside(folder, &brief)?;
        if std::fs::read_to_string(&brief).ok() != Some(text.clone()) {
            std::fs::write(&brief, text)?;
        }
    } else if brief.symlink_metadata().is_ok() {
        // A link is removed, not what it points at.
        std::fs::remove_file(&brief)?;
    }
    let claude = folder.join("CLAUDE.md");
    if claude.symlink_metadata().is_ok() {
        inside(folder, &claude)?;
    }
    let before = std::fs::read_to_string(&claude).unwrap_or_default();
    let after = imported(&before, on);
    if after != before {
        std::fs::write(&claude, after)?;
    }
    crate::orchestrator_brief::write_agents(folder)
}

/// The methods only an orchestrator in Assistant mode is answered, for the
/// agent API's list.
pub(crate) const METHODS: [&str; 5] = [
    "schedules",
    "schedule_propose",
    "heartbeat_status",
    "briefing",
    "telegram_send",
];

/// One of [`METHODS`], asked by the orchestrator `here` about `project`.
pub(crate) fn respond(
    app: Option<&AppHandle>,
    method: &str,
    here: &Project,
    project: &Project,
    projects: &[Project],
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    require(here)?;
    match method {
        "heartbeat_status" => crate::assistant_watch::status(here),
        "briefing" => {
            let hours = params
                .get("hours")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(24);
            let since = crate::ended_sessions::now_secs() - hours.clamp(1, 24 * 14) as i64 * 3600;
            Ok(serde_json::json!(crate::briefing::briefing(
                here, projects, since
            )))
        }
        "telegram_send" => crate::assistant_watch::telegram(params),
        _ => crate::schedules::respond(app, method, here, project, projects, params),
    }
}

/// The orchestrator `here`, when its Assistant mode is on; why not otherwise.
pub(crate) fn require(here: &Project) -> Result<(), String> {
    const OFF: &str =
        "Assistant mode is off for this orchestrator: the person turns it on in its chat";
    if here.orchestrator.is_none() {
        return Err(OFF.to_owned());
    }
    let store = crate::projects::store().map_err(|err| err.message)?;
    on(&store, Path::new(&here.root_path))
        .then_some(())
        .ok_or_else(|| OFF.to_owned())
}

/// The folder of the orchestrator `project_id`, or why it is not one.
pub(crate) fn folder_of(store: &Store, project_id: &str) -> Result<std::path::PathBuf, RpcError> {
    let (_, root) = crate::projects::locate(store, project_id)?;
    let home = Store::root()
        .ok()
        .and_then(|home| home.canonicalize().ok())
        .unwrap_or_default();
    devpit_core::home::orchestrator_of(store, &home, &root)
        .map(|_| root)
        .ok_or_else(|| RpcError::new(ErrorCode::Invalid, "that project is not an orchestrator"))
}

/// `assistant.mode` — whether this orchestrator's Assistant mode is on.
#[tauri::command]
#[specta::specta]
pub async fn assistant_mode(project_id: String) -> Result<AssistantMode, RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::projects::store()?;
        let folder = folder_of(&store, &project_id)?;
        Ok(AssistantMode {
            on: on(&store, &folder),
        })
    })
    .await
}

/// `assistant.mode_set` — Assistant mode on or off, as the person clicked it.
/// The MCP servers are told to look again, so the running chat gains or loses
/// the assistant's tools; its brief is read when the chat next starts.
#[tauri::command]
#[specta::specta]
pub async fn assistant_mode_set(
    app: AppHandle,
    project_id: String,
    on: bool,
) -> Result<AssistantMode, RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::projects::store()?;
        let folder = folder_of(&store, &project_id)?;
        store.set_orchestrator_setting(&folder, KEY, serde_json::json!(on))?;
        brief_in(&folder, on).map_err(|err| RpcError::internal(err.to_string()))?;
        crate::devflow_asks::rang(Some(&app), &project_id);
        Ok(AssistantMode { on })
    })
    .await
}

#[cfg(test)]
#[path = "assistant_tests.rs"]
mod tests;
