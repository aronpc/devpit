//! devpit's MCP for a client on another machine, through devpit.app's relay:
//! the board, the sessions and what waits on the person — read, never
//! changed.
//!
//! The client is a device paired through the relay like a phone, from an
//! offer made for an MCP client, and known by its key. It is served
//! `devpit_agentapi::remote_mcp`, whose allowlist decides what may be asked,
//! and the methods here answer reads alone. Off unless the person allows it
//! in Settings → Remote; turning it off, or forgetting the client, ends its
//! connection at once and its session's keys with it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use devpit_core::{preference, Store};
use devpit_rpc::{Project, RemoteOut};
use serde_json::{json, Value};

use crate::remote_devices::{Device, Devices};
use crate::remote_relay::RelayLink;

/// A client that says nothing for this long is let go; it connects again
/// when it next has something to ask.
const IDLE: Duration = Duration::from_secs(90);
/// The most of a question's input, a draft or a session's step sent out.
const MOST_TEXT: usize = 2_000;

/// The methods a client outside the machine is answered, every one a read.
pub(crate) const READS: [&str; 5] = ["projects", "board", "card", "sessions", "waiting"];

pub(crate) fn allowed(store: &Store) -> bool {
    store
        .preference(preference::REMOTE_MCP)
        .ok()
        .flatten()
        .as_deref()
        == Some("true")
}

/// Ends the connection of every MCP client: remote MCP was turned off.
pub(crate) fn cut(app: &tauri::AppHandle, root: &Path) {
    for id in clients(root) {
        crate::remote_hub::drop_device(app, &id);
    }
}

/// The MCP clients paired here, by device id; the viewers are left alone.
fn clients(root: &Path) -> Vec<String> {
    Devices::read(root)
        .devices
        .into_iter()
        .filter(|one| one.mcp)
        .map(|one| one.id)
        .collect()
}

/// An MCP client's connection, served until it leaves, goes quiet, or is cut.
pub(crate) fn serve(app: tauri::AppHandle, root: PathBuf, mut link: RelayLink, device: Device) {
    let on = crate::projects::store().is_ok_and(|store| allowed(&store));
    if !on {
        let refused = RemoteOut::Refused {
            why: "remote MCP is off on that machine — allow it in Settings → Remote".to_owned(),
        };
        if let Ok(text) = serde_json::to_string(&refused) {
            let _ = link.send(text.as_bytes());
        }
        return link.close();
    }
    // In the hub like any device, so forgetting it or turning remote MCP off
    // drops it the way a viewer is dropped.
    let (id, _out, heard) = crate::remote_hub::join(&app, &device.id);
    crate::remote::note(&root, &device, "connected over MCP");
    let mut still = || {
        // What viewers are told is not this client's: kept from piling up.
        while heard.try_recv().is_ok() {}
        crate::remote_hub::holds(id)
    };
    let mut answer = |line: &str| {
        devpit_agentapi::remote_mcp::handle_in(
            devpit_core::locale::Locale::current().tag(),
            line,
            &|method, params| ask(method, &params),
        )
    };
    devpit_remote::mcp::serve(&mut link, &mut still, &mut answer, IDLE);
    crate::remote_hub::leave(&app, id);
    crate::remote::note(&root, &device, "disconnected");
    link.close();
}

/// One read for a client outside the machine. Anything but [`READS`] is
/// refused, whatever the allowlist upstream let through.
pub(crate) fn ask(method: &str, params: &Value) -> Result<Value, String> {
    if !READS.contains(&method) {
        return Err(format!("`{method}` is not read from outside the machine"));
    }
    let text = |name: &str| params.get(name).and_then(Value::as_str);
    match method {
        "projects" => projects(),
        "board" => {
            let project = project(text("project"))?;
            let board =
                crate::board::board_get_now(project.id.clone()).map_err(|err| err.message)?;
            Ok(json!({
                "project": { "id": project.id, "name": project.name },
                "columns": crate::agent_api::board_view(&board),
            }))
        }
        "card" => {
            let project = project(text("project"))?;
            let board = crate::board::board_get_now(project.id).map_err(|err| err.message)?;
            crate::agent_api::card_view(&board, text("cardId").unwrap_or_default())
        }
        "sessions" => {
            let listed: Vec<Value> = sessions().iter().map(session_view).collect();
            Ok(json!({ "sessions": listed }))
        }
        "waiting" => waiting(),
        _ => Err(format!("`{method}` is not read from outside the machine")),
    }
}

/// A session as a client outside the machine reads it: what it is doing and
/// what it waits on, without the handles a terminal is reached by.
fn session_view((account, one): &(String, devpit_rpc::LiveSession)) -> Value {
    json!({
        "account": account,
        "name": one.name,
        "status": one.status,
        "kind": one.kind,
        "project": one.project_name,
        "projectId": one.project_id,
        "cardId": one.card_id,
        "since": one.since,
        "step": one.step.as_deref().map(|step| cut_to(step, MOST_TEXT)),
        "waitingOn": one.waiting.as_ref().map(|waiting| &waiting.question),
        "draftWaiting": one.draft.is_some(),
    })
}

fn listed() -> Result<Vec<Project>, String> {
    Ok(crate::projects::project_list_unread_now()
        .map_err(|err| err.message)?
        .projects
        .into_iter()
        .filter(|one| one.orchestrator.is_none())
        .collect())
}

/// The project a client names, by id or by name.
fn project(named: Option<&str>) -> Result<Project, String> {
    let named = named
        .map(str::trim)
        .filter(|named| !named.is_empty())
        .ok_or("which project? pass its id or name — devpit_projects lists them")?;
    listed()?
        .into_iter()
        .find(|one| one.id == named || one.name.eq_ignore_ascii_case(named))
        .ok_or_else(|| format!("no project called `{named}` — devpit_projects lists them"))
}

fn projects() -> Result<Value, String> {
    let projects: Vec<Value> = listed()?
        .into_iter()
        .map(|one| {
            let lanes = crate::board::board_get_now(one.id.clone())
                .map(|board| {
                    board
                        .columns
                        .iter()
                        .map(|column| {
                            let cards = board
                                .cards
                                .iter()
                                .filter(|card| card.column_id == column.id)
                                .count();
                            json!({ "name": column.name, "cards": cards })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            json!({ "id": one.id, "name": one.name, "group": one.group, "lanes": lanes })
        })
        .collect();
    Ok(json!({ "projects": projects }))
}

/// Every session running now, of every account this machine talks as.
fn sessions() -> Vec<(String, devpit_rpc::LiveSession)> {
    let Ok(profiles) = crate::agent_profiles::agent_profiles_now() else {
        return Vec::new();
    };
    let mut every: Vec<(String, devpit_rpc::LiveSession)> = Vec::new();
    for profile in profiles.iter().filter(|one| one.enabled) {
        let live = crate::live_sessions::orchestrator_sessions_now(&profile.id)
            .map(|live| live.sessions)
            .unwrap_or_default();
        for one in live {
            // Two profiles on one account list its sessions twice.
            if !every.iter().any(|(_, seen)| seen.pid == one.pid) {
                every.push((profile.label.clone(), one));
            }
        }
    }
    every
}

fn waiting() -> Result<Value, String> {
    let questions: Vec<Value> = crate::remote_hub::questions()
        .into_iter()
        .map(|one| {
            json!({
                "from": one.from,
                "project": one.project,
                "tool": one.tool,
                "input": cut_to(&one.input, MOST_TEXT),
            })
        })
        .collect();
    let stopped: Vec<Value> = sessions()
        .into_iter()
        .filter_map(|(account, one)| {
            let waiting = one.waiting?;
            Some(json!({
                "account": account,
                "session": one.name,
                "project": one.project_name,
                "cardId": one.card_id,
                "question": waiting.question,
                "options": waiting.options.into_iter().map(|option| option.label).collect::<Vec<_>>(),
            }))
        })
        .collect();
    let drafts: Vec<Value> = crate::reply_drafts::all()
        .into_iter()
        .map(|(account, session, text)| json!({ "account": account, "session": session, "text": cut_to(&text, MOST_TEXT) }))
        .collect();
    let store = crate::projects::store().map_err(|err| err.message)?;
    let names = listed()?;
    let reminders: Vec<Value> = store
        .reminders_open(None)
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter(|row| row.reminded_at.is_some())
        .map(|row| {
            let project = names.iter().find(|one| one.id == row.project_id).map(|one| &one.name);
            json!({ "project": project, "cardId": row.card_id, "title": row.title, "at": row.due_at })
        })
        .collect();
    Ok(json!({
        "questions": questions,
        "sessions": stopped,
        "drafts": drafts,
        "reminders": reminders,
        "note": "Allowing a tool, answering a session, sending a draft and dealing with a reminder are done on the machine or from the Remote.",
    }))
}

/// `text`, at most `most` bytes of it, cut on a character.
fn cut_to(text: &str, most: usize) -> String {
    if text.len() <= most {
        return text.to_owned();
    }
    let mut end = most;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

#[cfg(test)]
#[path = "remote_mcp_tests.rs"]
mod tests;
