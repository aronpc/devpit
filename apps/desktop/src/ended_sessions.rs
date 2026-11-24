//! Sessions of an orchestrator's account that ran and ended: listed, read and
//! resumed in a devpit terminal as the same conversation.
//!
//! The CLI lists only what runs now, so a session stopped by mistake left its
//! work behind a transcript somebody had to find by hand. devpit keeps what it
//! saw running (`seen_session`), and this reads it back.

use std::path::Path;

use devpit_core::store::SeenSessionRow;
use devpit_rpc::{EndedSession, EndedSessions, ErrorCode, RpcError, SessionOnCard, SessionsOnCard};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

/// How many ended sessions are listed.
const MOST: u32 = 20;

pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|at| at.as_secs() as i64)
        .unwrap_or(0)
}

/// Files changed and not committed in `folder`, when it is a repository.
pub(crate) fn dirty_in(folder: &str) -> Option<u32> {
    let folder = Path::new(folder);
    if !folder.is_dir() {
        return None;
    }
    devpit_git::status(folder)
        .ok()
        .map(|status| status.dirty_files())
}

/// Records what `profile_id` has running now, so what ends is kept.
pub(crate) fn remember(profile_id: &str, running: &[devpit_rpc::LiveSession]) {
    let seen: Vec<_> = running
        .iter()
        .filter_map(|one| {
            Some(devpit_core::store::SessionSeen {
                session_id: one.session_id.as_deref()?,
                name: &one.name,
                cwd: &one.cwd,
                project_id: one.project_id.as_deref(),
                card_id: one.card_id.as_deref(),
                status: &one.status,
            })
        })
        .collect();
    let kept = crate::projects::store()
        .map_err(|err| err.message)
        .and_then(|store| {
            store
                .saw_sessions(profile_id, &seen, now_secs())
                .map_err(|err| err.to_string())
        });
    if let Err(why) = kept {
        devpit_core::reports::background("seen sessions", &why);
    }
}

fn drawn(row: SeenSessionRow, projects: &[devpit_rpc::Project]) -> EndedSession {
    EndedSession {
        project_name: row
            .project_id
            .as_deref()
            .and_then(|id| projects.iter().find(|one| one.id == id))
            .map(|one| one.name.clone()),
        dirty: dirty_in(&row.cwd),
        started_at: row.first_seen_at as f64 * 1000.0,
        ended_at: row.ended_at.unwrap_or(row.last_seen_at) as f64 * 1000.0,
        session_id: row.session_id,
        name: row.name,
        cwd: row.cwd,
        project_id: row.project_id,
        card_id: row.card_id,
        ended_by: row.ended_by,
        last_status: row.last_status,
    }
}

/// The sessions of `profile_id` that ended, the latest first.
pub(crate) fn ended_now(profile_id: &str) -> Result<EndedSessions, RpcError> {
    let store = crate::projects::store()?;
    let projects = crate::live_sessions::recent_projects()?;
    Ok(EndedSessions {
        sessions: store
            .ended_sessions(profile_id, MOST)?
            .into_iter()
            .map(|row| drawn(row, &projects))
            .collect(),
    })
}

/// How many sessions a card lists.
const MOST_ON_CARD: u32 = 30;

/// A card's kept sessions, joined with the live ones of their accounts, the
/// newest first: running ones, then by when they ended.
pub(crate) fn on_card(
    mut rows: Vec<SeenSessionRow>,
    live: &[devpit_rpc::LiveSession],
    dirty: impl Fn(&str) -> Option<u32>,
) -> Vec<SessionOnCard> {
    rows.sort_by_key(|row| {
        std::cmp::Reverse((row.ended_at.unwrap_or(i64::MAX), row.first_seen_at))
    });
    rows.into_iter()
        .map(|row| {
            let running = row
                .ended_at
                .is_none()
                .then(|| {
                    live.iter()
                        .find(|one| one.session_id.as_deref() == Some(row.session_id.as_str()))
                })
                .flatten();
            SessionOnCard {
                dirty: dirty(&row.cwd),
                started_at: row.first_seen_at as f64 * 1000.0,
                ended_at: row.ended_at.map(|at| at as f64 * 1000.0),
                pane: running.and_then(|one| one.pane.clone()),
                remote: running.map(|one| one.remote.clone()).unwrap_or_default(),
                profile_id: row.profile_id,
                session_id: row.session_id,
                name: row.name,
                ended_by: row.ended_by,
                last_status: row.last_status,
            }
        })
        .collect()
}

/// [`card_session_history`], on the calling thread.
fn on_card_now(card_id: &str) -> Result<SessionsOnCard, RpcError> {
    let store = crate::projects::store()?;
    let rows = store.sessions_on_card(card_id, MOST_ON_CARD)?;
    // A row still open is as fresh as its account's last listing: listed
    // again, the ones that ended since are closed.
    let mut accounts: Vec<&str> = rows
        .iter()
        .filter(|row| row.ended_at.is_none())
        .map(|row| row.profile_id.as_str())
        .collect();
    accounts.sort_unstable();
    accounts.dedup();
    let live: Vec<_> = accounts
        .iter()
        .filter_map(|profile| crate::live_sessions::orchestrator_sessions_now(profile).ok())
        .flat_map(|listed| listed.sessions)
        .collect();
    let rows = if accounts.is_empty() {
        rows
    } else {
        store.sessions_on_card(card_id, MOST_ON_CARD)?
    };
    Ok(SessionsOnCard {
        sessions: on_card(rows, &live, dirty_in),
    })
}

/// `card.sessionHistory` — every session devpit saw on a card, running or ended.
#[tauri::command]
#[specta::specta]
pub async fn card_session_history(card_id: String) -> Result<SessionsOnCard, RpcError> {
    crate::off_main::blocking(move || on_card_now(&card_id)).await
}

/// `orchestrator.ended` — what this account ran that has ended.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_ended(profile_id: String) -> Result<EndedSessions, RpcError> {
    crate::off_main::blocking(move || ended_now(&profile_id)).await
}

/// A kept session of `profile_id` that has ended, by id or name.
pub(crate) fn ended_one(profile_id: &str, id_or_name: &str) -> Result<SeenSessionRow, RpcError> {
    let row = crate::projects::store()?
        .seen_session(profile_id, id_or_name)?
        .ok_or_else(|| {
            RpcError::new(
                ErrorCode::NotFound,
                format!("no session called {id_or_name} has run here"),
            )
        })?;
    if row.ended_at.is_none() {
        return Err(RpcError::new(
            ErrorCode::Conflict,
            format!("{} is still running — message it instead", row.name),
        ));
    }
    Ok(row)
}

/// Refuses a conversation a live process listed in `dirs` already runs.
pub(crate) fn not_running(dirs: &[std::path::PathBuf], session_id: &str) -> Result<(), RpcError> {
    match dirs
        .iter()
        .find_map(|dir| crate::live_sessions::running_conversation(dir, session_id))
    {
        Some(name) => Err(RpcError::new(
            ErrorCode::Conflict,
            format!("{name} already runs that conversation — message it instead"),
        )),
        None => Ok(()),
    }
}

/// The line that resumes `session_id` as `name`, for this account.
fn resumed_line(
    profile_id: &str,
    project_id: &str,
    name: &str,
    session_id: &str,
) -> Result<String, RpcError> {
    resumed(
        // A session resumed is a session born again, in the language of the
        // moment it is resumed, not of the one that ended.
        &crate::shell_launch::to_start_in(
            crate::session_locale::SessionLocale::capture(),
            profile_id,
            Some(project_id),
        )?,
        name,
        crate::restoring::resume_flag_of(profile_id),
        session_id,
    )
}

/// `start` named `name`, continuing `session_id`; refused rather than quoted
/// when either would not stay text at a shell.
pub(crate) fn resumed(
    start: &str,
    name: &str,
    flag: Option<&str>,
    session_id: &str,
) -> Result<String, RpcError> {
    crate::opening::typeable(name)?;
    let flag = flag.ok_or_else(|| {
        RpcError::new(
            ErrorCode::Invalid,
            "this account's agent cannot resume a conversation",
        )
    })?;
    // What `with_resume` types; anything else it would drop, and start afresh.
    let id = !session_id.is_empty()
        && session_id.len() <= 64
        && session_id
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-');
    if !id {
        return Err(RpcError::new(
            ErrorCode::Invalid,
            "that is not a session id",
        ));
    }
    let named = format!(
        "{start} --name {}",
        devpit_agentcli::declaring::quoted(name)
    );
    Ok(crate::restoring::with_resume(
        &named,
        Some(flag),
        Some(session_id),
    ))
}

/// Resumes an ended session's conversation in a new terminal tab of its
/// project, in the folder it ran in.
pub(crate) fn resume(
    app: &AppHandle,
    profile_id: &str,
    id_or_name: &str,
    named: Option<&str>,
) -> Result<Value, RpcError> {
    let row = ended_one(profile_id, id_or_name)?;
    // Its process may outlive what devpit last read: a second one would share its work.
    not_running(
        &crate::live_sessions::listed_dirs(profile_id).unwrap_or_default(),
        &row.session_id,
    )?;
    let project_id = row.project_id.clone().ok_or_else(|| {
        RpcError::new(
            ErrorCode::Invalid,
            format!("{} ran outside devpit's projects", row.name),
        )
    })?;
    let config = crate::live_sessions::config_of(profile_id)?;
    let transcript =
        devpit_agentcli::transcript_path(&config, Path::new(&row.cwd), &row.session_id);
    if !std::fs::metadata(&transcript).is_ok_and(|meta| meta.len() > 0) {
        return Err(RpcError::new(
            ErrorCode::Conflict,
            format!("{} wrote no conversation to resume", row.name),
        ));
    }
    let name = crate::handing::unused(
        named
            .map(str::trim)
            .filter(|one| !one.is_empty())
            .unwrap_or(&row.name)
            .to_owned(),
        &crate::handing::live_names(profile_id),
    );
    let line = resumed_line(profile_id, &project_id, &name, &row.session_id)?;
    let cwd = if Path::new(&row.cwd).is_dir() {
        std::path::PathBuf::from(&row.cwd)
    } else {
        crate::roots::root_of(&project_id, None)?
    };
    let tab_id = crate::opening::fresh_tab();
    let pane_id =
        crate::opening::typed_in(app, &project_id, &tab_id, &cwd, &line, Some(profile_id))?;
    let _ = app.emit(
        crate::opening::TAB_OPENED,
        json!({ "projectId": project_id, "tabId": tab_id, "paneId": pane_id }),
    );
    crate::starting::began(crate::starting::Starting {
        profile: profile_id.to_owned(),
        name: name.clone(),
        project_id: project_id.clone(),
        project_name: None,
        cwd: cwd.display().to_string(),
        pane_id,
        at: std::time::Instant::now(),
    });
    Ok(json!({
        "name": name,
        "sessionId": row.session_id,
        "cwd": cwd.display().to_string(),
        "next": "The same conversation runs again in a new terminal tab of its project. Message it by this name once it is up.",
    }))
}

/// `orchestrator.resume` — the person resumes an ended session.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_resume(
    app: AppHandle,
    profile_id: String,
    session_id: String,
) -> Result<String, RpcError> {
    crate::off_main::blocking(move || {
        resume(&app, &profile_id, &session_id, None)
            .map(|said| said["name"].as_str().unwrap_or_default().to_owned())
    })
    .await
}

/// `orchestrator.told` — what a session said last, running or ended.
#[tauri::command]
#[specta::specta]
pub async fn orchestrator_told(
    profile_id: String,
    session: String,
) -> Result<devpit_rpc::SessionTold, RpcError> {
    crate::off_main::blocking(move || {
        let said = crate::session_told::told_by(&profile_id, &session, 3)
            .map_err(|why| RpcError::new(ErrorCode::NotFound, why))?;
        let texts = |key: &str| -> Vec<String> {
            said[key]
                .as_array()
                .map(|all| {
                    all.iter()
                        .filter_map(|one| one.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        Ok(devpit_rpc::SessionTold {
            replies: texts("replies"),
            prompts: texts("prompts"),
        })
    })
    .await
}

#[cfg(test)]
#[path = "ended_sessions_tests.rs"]
mod tests;
