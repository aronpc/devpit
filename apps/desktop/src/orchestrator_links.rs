//! The projects an orchestrator is linked to — chosen by the person, one by
//! one. Only those are on its Boards panel, readable by its chat, and within
//! reach of its tools; every other project stays where it is.
//!
//! Kept in devpit's store beside its account, by the orchestrator's folder —
//! not in a file inside it, where the orchestrator's own chat could edit the
//! links that bound it.

use std::path::Path;

use devpit_core::Store;
use devpit_rpc::{ErrorCode, LiveSession, Project, RpcError};
use serde_json::{json, Value};

/// One setting changed, the rest kept.
pub(crate) fn set_in(
    store: &Store,
    folder: &Path,
    key: &str,
    value: Value,
) -> Result<(), RpcError> {
    Ok(store.set_orchestrator_setting(folder, key, value)?)
}

/// The ids of the projects this orchestrator's folder is linked to.
pub(crate) fn linked(folder: &Path) -> Vec<String> {
    crate::projects::store()
        .map(|store| linked_in(&store, folder))
        .unwrap_or_default()
}

/// [`linked`], in a given store.
pub(crate) fn linked_in(store: &Store, folder: &Path) -> Vec<String> {
    store
        .orchestrator_settings(folder)
        .unwrap_or_else(|_| json!({}))["projects"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The sessions an orchestrator may see and touch: those running in itself or
/// in a project linked to it. A session elsewhere in the account is someone
/// else's work, and reading its screen or stopping it is not this one's to do.
pub(crate) fn within(
    here_id: &str,
    linked: &[String],
    sessions: Vec<LiveSession>,
) -> Vec<LiveSession> {
    sessions
        .into_iter()
        .filter(|one| {
            one.project_id
                .as_deref()
                .is_some_and(|id| id == here_id || linked.iter().any(|link| link == id))
        })
        .collect()
}

/// [`within`], for the orchestrator asking.
pub(crate) fn reachable(here: &Project, sessions: Vec<LiveSession>) -> Vec<LiveSession> {
    within(&here.id, &linked(Path::new(&here.root_path)), sessions)
}

/// Whether `here`, an orchestrator, may reach `project`: itself, or one the
/// person linked. A project reaches only itself, and is not asked here.
pub(crate) fn reaches(here: &Project, project: &Project) -> Result<(), String> {
    let store = crate::projects::store().map_err(|err| err.message)?;
    reaches_in(&store, here, project)
}

/// [`reaches`], in a given store.
pub(crate) fn reaches_in(store: &Store, here: &Project, project: &Project) -> Result<(), String> {
    if here.orchestrator.is_none() || project.id == here.id {
        return Ok(());
    }
    linked_in(store, Path::new(&here.root_path))
        .contains(&project.id)
        .then_some(())
        .ok_or_else(|| {
            format!(
                "{} is not linked to this orchestrator — propose linking it with devpit_propose_project, and the person links it with one click",
                project.name
            )
        })
}

/// `orchestrator.links` — the projects this orchestrator is linked to.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_links(project_id: String) -> Result<Vec<String>, RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::projects::store()?;
        let (_, root) = crate::projects::locate(&store, &project_id)?;
        Ok(linked(&root))
    })
    .await
}

/// `orchestrator.link` — the projects it is linked to, as the person chose
/// them. Only projects, never another orchestrator. Linking reads nothing:
/// a project's agent learns it once the person turns it on for that project.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_link(project_id: String, linked: Vec<String>) -> Result<(), RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::projects::store()?;
        let (_, root) = crate::projects::locate(&store, &project_id)?;
        let projects = crate::projects::project_list_now()?.projects;
        if let Some(stray) = linked.iter().find(|id| {
            !projects
                .iter()
                .any(|one| &one.id == *id && one.orchestrator.is_none())
        }) {
            return Err(RpcError::new(
                ErrorCode::Invalid,
                format!("{stray} is not a project that can be linked"),
            ));
        }
        set_in(&store, &root, "projects", json!(linked))?;
        // Links sync: the other machines hear of it in seconds.
        crate::sync::nudge();
        Ok(())
    })
    .await
}

#[cfg(test)]
#[path = "orchestrator_links_tests.rs"]
mod tests;
