//! What a chat turn settles before anything runs.
//!
//! Apart from `chat.rs`, which had grown to its ceiling holding the command and
//! every rule a turn follows at once.

use devpit_agentcli::head::Head;
use devpit_rpc::{ErrorCode, RpcError};

/// Notes on the turn's end what it ran on: the model the CLI said it used,
/// else the one asked for, and the effort it was asked to think at.
pub(crate) fn ran_on(said: &mut devpit_agentcli::talk::Said, opening: &Head) {
    said.end.model = said
        .init
        .as_ref()
        .and_then(|init| init.model.clone())
        .or_else(|| opening.model.clone());
    said.end.effort = opening.effort.clone();
}

/// The head written before the turn runs.
///
/// What the conversation already settled carries over; a turn may still change
/// the model, the budget, the permission mode and the effort.
pub(crate) fn opening(
    head: Option<&Head>,
    profile_id: &str,
    model: Option<String>,
    budget_usd: Option<f64>,
    permission: Option<String>,
    effort: Option<String>,
    now: f64,
) -> Head {
    Head {
        profile: profile_id.to_owned(),
        model,
        created_at: head.map_or(now, |head| head.created_at),
        cost_usd: head.map(|head| head.cost_usd).unwrap_or_default(),
        budget_usd: budget_usd.or(head.and_then(|head| head.budget_usd)),
        session_id: head.and_then(|head| head.session_id.clone()),
        permission: permission
            .or_else(|| head.and_then(|head| head.permission.clone()))
            // Nothing to ask with yet: a turn left waiting on a prompt this
            // screen cannot draw would hang with no way to answer it.
            .or_else(|| Some("acceptEdits".to_owned())),
        effort: effort.or_else(|| head.and_then(|head| head.effort.clone())),
        title: head.and_then(|head| head.title.clone()),
        rewind: head.map(|head| head.rewind.clone()).unwrap_or_default(),
        cwd: head.and_then(|head| head.cwd.clone()),
        context: head.and_then(|head| head.context),
    }
}

/// Where a turn runs: the folder the conversation was fixed to, or the one the
/// window asked for when it was fixed to none.
///
/// Never a folder made for the turn. One that has gone is refused rather than
/// replaced: a session resumed anywhere else is a session the CLI does not find.
pub(crate) fn turn_cwd(fixed: Option<&str>, asked: &str) -> Result<String, RpcError> {
    match fixed {
        None => Ok(asked.to_owned()),
        Some(folder) if std::path::Path::new(folder).is_dir() => Ok(folder.to_owned()),
        Some(_) => Err(RpcError::new(
            ErrorCode::Conflict,
            "the folder this conversation ran in is gone",
        )),
    }
}

/// The profile a chat turn runs as, and the program to spawn for it: a name
/// only the shell knows is refused here, since a turn is spawned, not typed.
pub(crate) fn spawnable(profile_id: &str) -> Result<(devpit_rpc::Profile, String), RpcError> {
    let Some(profile) = crate::agent_profiles::all(&crate::projects::store()?)?
        .into_iter()
        .find(|found| found.id == profile_id)
    else {
        return Err(RpcError::new(
            ErrorCode::NotFound,
            format!("no profile called {profile_id}"),
        ));
    };
    if let Some(why) = crate::profile_waiting::blocked(&crate::projects::store()?, profile_id) {
        return Err(RpcError::new(ErrorCode::Conflict, why));
    }
    // Spawned, not typed, so a name only the shell knows is no use here — and
    // saying so beats "not on the PATH" about something the person watches
    // their terminal run every day.
    let Some(path) = profile.path.clone() else {
        return Err(RpcError::new(
            ErrorCode::NotFound,
            match profile.reach {
                devpit_rpc::Reach::ShellOnly => format!(
                    "{} is a shell function, so devpit cannot start it here — \
                     name the program it runs instead",
                    profile.command
                ),
                _ => format!("{} is not installed", profile.command),
            },
        ));
    };
    if let Some(why) = devpit_agentcli::acp::outdated(&profile.base, &path) {
        return Err(RpcError::new(ErrorCode::Conflict, why));
    }
    Ok((profile, path))
}

/// How a turn that failed ends: an error, so the screen stops waiting on it.
pub(crate) fn failed_end(turn_id: &str) -> devpit_rpc::TurnEnd {
    devpit_rpc::TurnEnd {
        turn_id: turn_id.to_owned(),
        cost_usd: None,
        duration_ms: None,
        stop_reason: Some("error".to_owned()),
        is_error: true,
        context: None,
        ..Default::default()
    }
}

/// A turn as it came back. One that failed keeps what it said before
/// failing, when it began, and its answer is said to be over, so nothing is
/// left "working" and a reload no longer claims the app closed mid-turn.
pub(crate) fn or_failed<T>(
    said: Result<T, RpcError>,
    turn_id: &str,
    file: &std::path::Path,
    kept: Option<&std::sync::Arc<std::sync::Mutex<Vec<devpit_rpc::Part>>>>,
    opened: &devpit_rpc::Message,
    on_frame: &tauri::ipc::Channel<devpit_rpc::Frame>,
) -> Result<T, RpcError> {
    if let Err(failed) = &said {
        if let Some(parts) = kept {
            let answered = devpit_rpc::Message {
                parts: parts.lock().map(|held| held.clone()).unwrap_or_default(),
                streaming: false,
                ended: Some(devpit_rpc::Ended {
                    error: Some(failed.message.clone()),
                    ..devpit_rpc::Ended::of(&failed_end(turn_id), crate::chat_listening::now())
                }),
                ..opened.clone()
            };
            let _ = devpit_agentcli::store::append(file, &answered);
        }
        let end = failed_end(turn_id);
        let _ = on_frame.send(devpit_rpc::Frame::Ended { end });
    }
    said
}

#[cfg(test)]
#[path = "chat_turn_tests.rs"]
mod tests;

/// The head after a turn, holding how full the context was when it ended. A
/// turn that reported none — stopped before its end frame — keeps the last
/// known reading rather than blanking the meter.
pub(crate) fn after(
    opening: Head,
    turn_id: &str,
    end: &devpit_rpc::TurnEnd,
    session_id: Option<String>,
    anchor: Option<String>,
) -> Head {
    let kept = opening.context;
    Head {
        context: end.context.or(kept),
        ..devpit_agentcli::head::after_turn(opening, turn_id, end.cost_usd, session_id, anchor)
    }
}

/// Head writes after a turn, one at a time: a woken turn and the person's both
/// end in the same head, and each read-then-write erased the other's.
static SETTLING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Writes the head after a turn onto the one on disk now — not the one the
/// turn began with, which misses whatever ended since: a woken turn's session
/// and cost. `fallback` stands in when there is none on disk; with neither,
/// nothing is written.
pub(crate) fn settle(
    file: &std::path::Path,
    fallback: Option<Head>,
    turn_id: &str,
    end: &devpit_rpc::TurnEnd,
    session_id: Option<String>,
    anchor: Option<String>,
) {
    let _held = SETTLING.lock();
    let Some(now) = devpit_agentcli::head::read_head(file).or(fallback) else {
        return;
    };
    let _ = devpit_agentcli::head::write_head(file, &after(now, turn_id, end, session_id, anchor));
}

/// The person's message and the answer about to be written under it, both
/// shown at once; the transcript takes them only once the turn has run.
pub(crate) fn messages(
    turn_id: &str,
    prompt: &str,
    at: f64,
) -> (devpit_rpc::Message, devpit_rpc::Message) {
    let message = |role, parts, streaming| devpit_rpc::Message {
        id: format!("msg_{}", ulid::Ulid::generate()),
        turn_id: Some(turn_id.to_owned()),
        role,
        parts,
        created_at: at,
        streaming,
        ended: None,
    };
    let said = devpit_rpc::Part::Text {
        text: prompt.to_owned(),
        parent: None,
    };
    (
        message(devpit_rpc::Role::User, vec![said], false),
        message(devpit_rpc::Role::Assistant, Vec::new(), true),
    )
}
