//! Stopping a session of an orchestrator's account: the agent ends, and so
//! does the terminal it ran in — its pane closed the way the window closes
//! one, its tab too when it was the last pane — so nothing is left running
//! out of sight.
//!
//! Never on the orchestrator's own initiative: the tool says so, and the
//! brief. Work in flight is lost, and that is the person's call.

use std::time::Duration;

use devpit_rpc::{ErrorCode, RpcError};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

/// Said when a tab was closed from here, so a window showing it lets it go.
pub(crate) const TAB_CLOSED: &str = "session:tab-closed";

/// `orchestrator.stop` — the window's own way to stop a session.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_stop(
    app: AppHandle,
    profile_id: String,
    name: String,
    force: bool,
) -> Result<String, RpcError> {
    crate::off_main::blocking(move || {
        stop(&app, &profile_id, &name, None, "person", force)
            .map(|said| said["stopped"].as_str().unwrap_or_default().to_owned())
    })
    .await
}

/// `terminal.close` — a devpit terminal closed from outside its tab: the
/// pane, or its tab when it is the last one, and whatever runs in it.
#[tauri::command]
#[specta::specta]
pub async fn terminal_close(
    app: AppHandle,
    project_id: String,
    pane_id: String,
) -> Result<(), RpcError> {
    crate::off_main::blocking(move || close_pane(&app, &project_id, &pane_id).map(|_| ())).await
}

/// Closes a pane the way the window does — its tab too when it was the last
/// one — and answers the tab it was in.
pub(crate) fn close_pane(
    app: &AppHandle,
    project_id: &str,
    pane_id: &str,
) -> Result<String, RpcError> {
    let store = crate::projects::store()?;
    let (tab_id, panes) = store
        .pane_layout_tabs(project_id)?
        .into_iter()
        .find_map(|tab| {
            let (tree, focused) = store.pane_layout(project_id, &tab).ok()??;
            let layout = crate::sessions::decode(project_id, &tree, &focused).ok()?;
            layout
                .tree
                .contains_leaf(pane_id)
                .then(|| (tab, layout.tree.leaves().len()))
        })
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "that terminal is not in any tab"))?;
    let state = app.state::<crate::sessions::SessionState>();
    if panes > 1 {
        crate::arranging::session_close_leaf_now(
            app.clone(),
            &state,
            project_id.to_owned(),
            tab_id.clone(),
            pane_id.to_owned(),
        )?;
    } else {
        crate::arranging::session_close_tab_now(
            app.clone(),
            state,
            project_id.to_owned(),
            tab_id.clone(),
        )?;
        let _ = app.emit(
            TAB_CLOSED,
            json!({ "projectId": project_id, "tabId": tab_id }),
        );
    }
    Ok(tab_id)
}

/// What stopping a session would lose, if anything: it is working, or its
/// folder holds changes not committed.
pub(crate) fn at_stake(busy: bool, dirty: Option<u32>) -> Option<String> {
    let dirty = dirty.filter(|files| *files > 0);
    Some(match (busy, dirty) {
        (false, None) => return None,
        (true, None) => "it is working right now".to_owned(),
        (false, Some(files)) => format!("its folder has {files} file(s) not committed"),
        (true, Some(files)) => {
            format!("it is working right now and its folder has {files} file(s) not committed")
        }
    })
}

/// Why stopping a session would lose work, if it would, asked as a question.
pub(crate) fn would_lose(name: &str, busy: bool, dirty: Option<u32>) -> Option<String> {
    at_stake(busy, dirty).map(|why| format!("{name}: {why} — stop it anyway?"))
}

/// The sessions called `name` running now, only `only` when given.
fn running(
    profile_id: &str,
    name: &str,
    only: Option<i32>,
) -> Result<Vec<crate::live_sessions::Running>, RpcError> {
    let found: Vec<_> = crate::live_sessions::running_named(profile_id, name)?
        .into_iter()
        .filter(|one| only.is_none_or(|pid| one.pid == pid))
        .collect();
    if found.is_empty() {
        return Err(RpcError::new(
            ErrorCode::NotFound,
            format!("no session called {name} is running with that process"),
        ));
    }
    Ok(found)
}

/// Whether any of `found` is working, and the most files one leaves.
fn stake(found: &[crate::live_sessions::Running]) -> (bool, Option<u32>) {
    let busy = found.iter().any(|one| one.status == "busy");
    let dirty = found
        .iter()
        .filter_map(|one| crate::ended_sessions::dirty_in(&one.cwd))
        .max();
    (busy, dirty)
}

/// What stopping `name` (only `only`) would lose now, if anything.
pub(crate) fn losing(
    profile_id: &str,
    name: &str,
    only: Option<i32>,
) -> Result<Option<String>, RpcError> {
    let (busy, dirty) = stake(&running(profile_id, name, only)?);
    Ok(at_stake(busy, dirty))
}

/// Stops the sessions called `name` of this account — every one, since a
/// name started twice is one name to the person — and closes their terminals.
/// With `only`, the one process of that name; without, every one — a name
/// handed twice before names were unique is one piece of work. One working,
/// or with work not committed, is stopped only with `force`.
pub(crate) fn stop(
    app: &AppHandle,
    profile_id: &str,
    name: &str,
    only: Option<i32>,
    by: &str,
    force: bool,
) -> Result<Value, RpcError> {
    let found = running(profile_id, name, only)?;
    if !force {
        let (busy, dirty) = stake(&found);
        if let Some(why) = would_lose(name, busy, dirty) {
            return Err(RpcError::new(ErrorCode::Conflict, why));
        }
    }
    end(app, profile_id, name, &found, by)
}

/// What a person's click on a stop came to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Clicked {
    Stopped,
    /// It would lose this now, which the click did not show: not stopped.
    Changed(String),
}

/// Whether a click that showed `seen` as what a stop loses covers what it
/// loses `now`: nothing, or that very loss. Otherwise `now`, to show first.
pub(crate) fn as_seen(seen: Option<&str>, now: Option<String>) -> Result<(), String> {
    match now {
        Some(now) if seen != Some(now.as_str()) => Err(now),
        _ => Ok(()),
    }
}

/// Stops `name` (only `only`) on the person's click, which showed `seen` as
/// what it loses: read again here, at the click, in the one read the stop
/// goes on.
pub(crate) fn stop_as_seen(
    app: &AppHandle,
    profile_id: &str,
    name: &str,
    only: Option<i32>,
    seen: Option<&str>,
) -> Result<Clicked, RpcError> {
    let found = running(profile_id, name, only)?;
    let (busy, dirty) = stake(&found);
    if let Err(now) = as_seen(seen, at_stake(busy, dirty)) {
        return Ok(Clicked::Changed(now));
    }
    end(app, profile_id, name, &found, "person").map(|_| Clicked::Stopped)
}

/// Ends `found`, the sessions called `name`, and closes their terminals.
fn end(
    app: &AppHandle,
    profile_id: &str,
    name: &str,
    found: &[crate::live_sessions::Running],
    by: &str,
) -> Result<Value, RpcError> {
    if let Ok(store) = crate::projects::store() {
        for id in found.iter().filter_map(|one| one.session_id.as_deref()) {
            let _ = store.session_stopped(id, by, crate::ended_sessions::now_secs());
        }
    }
    let mut terminal = Value::Null;
    let mut directly = 0;
    for one in found {
        match (&one.pane, &one.job) {
            (Some(pane), _) => {
                let tab_id = close_pane(app, &pane.project_id, &pane.pane_id)?;
                terminal = json!({ "projectId": pane.project_id, "tabId": tab_id });
                directly += usize::from(outlived(name, one, GRACE)?);
            }
            (None, Some(job)) => stop_job(profile_id, job)
                .map_err(|why| RpcError::new(ErrorCode::Conflict, format!("{name}: {why}")))?,
            // Outside devpit's terminals and not a job: only the process.
            (None, None) => ended_directly(name, one, GRACE)?,
        }
    }
    crate::stop_asked::forget(profile_id, name);
    let said = match (found.len(), terminal.is_null()) {
        (1, true) => format!("{name} was stopped"),
        (1, false) => format!("{name} was stopped, and its terminal closed"),
        (many, _) => format!("{many} sessions called {name} were stopped"),
    };
    let said = match (directly, found.len()) {
        (0, _) => said,
        (_, 1) => format!(
            "{said}; it was not running in that terminal, so its process was ended directly"
        ),
        (many, _) => format!(
            "{said}; {many} were not running in their terminals, so their processes were ended directly"
        ),
    };
    Ok(json!({ "stopped": said, "terminal": terminal }))
}

/// How long a process is given to end: once its terminal closed, and again
/// once it was asked to.
const GRACE: Duration = Duration::from_secs(3);

/// Whether `one`, whose terminal was just closed, outlived it — that was not
/// the terminal it ran in — and so was ended directly. Never stopped in word
/// only: one that will not end is an error.
pub(crate) fn outlived(
    name: &str,
    one: &crate::live_sessions::Running,
    grace: Duration,
) -> Result<bool, RpcError> {
    if gone_within(one, grace) {
        return Ok(false);
    }
    ended_directly(name, one, grace).map(|()| true)
}

/// Ends `one`'s process itself, and waits to see it gone.
fn ended_directly(
    name: &str,
    one: &crate::live_sessions::Running,
    grace: Duration,
) -> Result<(), RpcError> {
    if devpit_pty::process::terminate(one.pid) && gone_within(one, grace) {
        return Ok(());
    }
    Err(RpcError::new(
        ErrorCode::Conflict,
        format!(
            "{name} could not be stopped: its process {} still runs",
            one.pid
        ),
    ))
}

/// Whether `one`'s process is gone, or its pid another process's, within `wait`.
fn gone_within(one: &crate::live_sessions::Running, wait: Duration) -> bool {
    let until = std::time::Instant::now() + wait;
    loop {
        if !crate::live_sessions::still_running(one.pid, one.started.as_deref()) {
            return true;
        }
        if std::time::Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A background job, stopped by its own CLI under the account that runs it.
fn stop_job(profile_id: &str, job: &str) -> Result<(), String> {
    let store = crate::projects::store().map_err(|err| err.to_string())?;
    let runner = crate::agent_profiles::runner_for(&store, profile_id)?;
    devpit_agentcli::stop_background(Some(&runner), job).map_err(|err| err.to_string())
}

#[cfg(test)]
#[path = "stopping_tests.rs"]
mod tests;
