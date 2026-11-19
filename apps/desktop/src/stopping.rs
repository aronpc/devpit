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
    for one in found {
        match (&one.pane, &one.job) {
            (Some(pane), _) => {
                let tab_id = close_pane(app, &pane.project_id, &pane.pane_id)?;
                terminal = json!({ "projectId": pane.project_id, "tabId": tab_id });
            }
            (None, Some(job)) => stop_job(profile_id, job)
                .map_err(|why| RpcError::new(ErrorCode::Conflict, format!("{name}: {why}")))?,
            // Outside devpit's terminals and not a job: only the process.
            (None, None) => {
                if crate::live_sessions::still_running(one.pid, one.started.as_deref()) {
                    devpit_pty::process::terminate(one.pid);
                }
            }
        }
    }
    let directly = made_sure(name, found, GRACE)?;
    crate::stop_asked::forget(profile_id, name);
    let said = said(name, found.len(), !terminal.is_null(), directly);
    Ok(json!({ "stopped": said, "terminal": terminal }))
}

/// What a stop answers: what it stopped, and how many processes it had to
/// end itself.
pub(crate) fn said(name: &str, count: usize, closed: bool, directly: usize) -> String {
    let said = match (count, closed) {
        (1, false) => format!("{name} was stopped"),
        (1, true) => format!("{name} was stopped, and its terminal closed"),
        (many, _) => format!("{many} sessions called {name} were stopped"),
    };
    match directly {
        0 => said,
        1 => format!("{said}; one process still ran after that, and was ended directly"),
        many => format!("{said}; {many} processes still ran after that, and were ended directly"),
    }
}

/// How long a process is given to end: once its terminal closed or its job
/// was stopped, and again once it was ended directly.
const GRACE: Duration = Duration::from_secs(3);

/// Waits once, for all of `found`, to see each process gone. One that
/// outlived its terminal — the listing named one it did not run in — is
/// ended directly, when devpit can tell it is still that session's process.
/// Answers how many were. A stop is never said done while one runs: one
/// devpit cannot tell, or that will not end, is an error naming it.
pub(crate) fn made_sure(
    name: &str,
    found: &[crate::live_sessions::Running],
    grace: Duration,
) -> Result<usize, RpcError> {
    let outlived = still_after(found.iter().collect(), grace);
    let (sure, unsure): (Vec<_>, Vec<_>) = outlived.into_iter().partition(|one| identified(one));
    for one in &sure {
        end_itself(one, grace);
    }
    let stuck = still_after(sure.clone(), grace);
    if stuck.is_empty() && unsure.is_empty() {
        return Ok(sure.len());
    }
    let mut why = Vec::new();
    if !stuck.is_empty() {
        why.push(format!("{} would not end", processes(&stuck)));
    }
    if !unsure.is_empty() {
        why.push(format!(
            "{} outlived its terminal, and devpit cannot tell it is still that session's, so it was left running",
            processes(&unsure)
        ));
    }
    Err(RpcError::new(
        ErrorCode::Conflict,
        format!("{name} could not be stopped: {}", why.join("; ")),
    ))
}

/// `process 12` or `processes 12, 34`.
fn processes(these: &[&crate::live_sessions::Running]) -> String {
    let pids: Vec<String> = these.iter().map(|one| one.pid.to_string()).collect();
    match pids.len() {
        1 => format!("process {}", pids[0]),
        _ => format!("processes {}", pids.join(", ")),
    }
}

/// Whether devpit can tell `one`'s pid is still the process the listing
/// named: only Linux keeps a process's start, and only a listing that says
/// its own is checked against it. A pid devpit cannot tell is never signalled.
fn identified(one: &crate::live_sessions::Running) -> bool {
    cfg!(target_os = "linux") && one.started.is_some()
}

/// Ends `one`'s process itself. An agent leads its own process group and
/// runs its tools in it, so the group goes, as a closed terminal's does.
fn end_itself(one: &crate::live_sessions::Running, grace: Duration) {
    if !crate::live_sessions::still_running(one.pid, one.started.as_deref()) {
        return;
    }
    #[cfg(unix)]
    if let Ok(pid) = u32::try_from(one.pid) {
        // SAFETY: getpgid only reads the process table.
        if unsafe { libc::getpgid(one.pid) } == one.pid {
            devpit_pty::stop_group(pid, grace);
            return;
        }
    }
    let _ = grace;
    devpit_pty::process::terminate(one.pid);
}

/// Those of `these` still running after `wait`, watched together.
fn still_after(
    these: Vec<&crate::live_sessions::Running>,
    wait: Duration,
) -> Vec<&crate::live_sessions::Running> {
    let until = std::time::Instant::now() + wait;
    let mut left = these;
    loop {
        left.retain(|one| crate::live_sessions::still_running(one.pid, one.started.as_deref()));
        if left.is_empty() || std::time::Instant::now() >= until {
            return left;
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
