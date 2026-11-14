//! A draft sent at the orchestrator's call, on the person's own word.
//!
//! The orchestrator asks (`devpit_send_draft`); devpit reads the person's last
//! message in the orchestrator's log itself and sends only when that message
//! lets this draft go (`devpit_agentcli::person`). So a ✅ counts from
//! wherever the person spoke to the orchestrator — the composer or the Claude
//! app — and never from another session, a tool or the agent's own account.
//! One message lets one draft go: a second ask on the same words is refused.
//! Words that do not let it go leave it waiting for the person's click, and
//! from a channel's chat a word lets nothing go: there it is their press.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use devpit_agentcli::person::{authorises, bare, last_said, From, Said};
use serde_json::{json, Value};

/// A message that let a draft go: (profile, message id, session). A bare
/// "envie" names no session, so it is spent whole.
type Spent = (String, String, String);

fn spent() -> &'static Mutex<Vec<Spent>> {
    static SPENT: OnceLock<Mutex<Vec<Spent>>> = OnceLock::new();
    SPENT.get_or_init(Mutex::default)
}

/// Records `key` as spent; false when it already was.
pub(crate) fn spend(held: &mut Vec<Spent>, key: Spent) -> bool {
    if held.contains(&key) {
        return false;
    }
    held.push(key);
    true
}

/// What sending to `name` on `said` spends.
pub(crate) fn key_of(profile: &str, said: &Said, name: &str) -> Spent {
    let session = if bare(&said.text) { "" } else { name };
    (profile.to_owned(), said.id.clone(), session.to_owned())
}

/// Whether what the person said lets the draft for `name` go, with
/// `waiting` drafts in all. Never a word from a channel: there the yes is a
/// press on devpit's own button.
pub(crate) fn lets_go(said: &Said, name: &str, waiting: usize) -> bool {
    !matches!(said.from, From::Channel(_)) && authorises(&said.text, name, waiting)
}

/// What the orchestrator is told when what the person said does not let the
/// draft for `name` go: nothing is sent, and it waits for their click.
pub(crate) fn waits(said: &Said, name: &str) -> Value {
    let why = match said.from {
        From::Channel(_) => format!(
            "the person last wrote from {}, where a draft goes only by its button",
            said.from.label()
        ),
        _ => format!(
            "the person's last message («{}») does not ask for it by itself",
            short(&said.text)
        ),
    };
    json!({
        "done": false,
        "asked": format!("send the draft to {name}"),
        "note": format!("Not sent yet: {why}. It waits beside the session and above this chat's composer, where one click of the person's sends it. Tell them so; do not ask them to type it again."),
    })
}

/// Sends the draft for `name` when the person's last word in the log of the
/// orchestrator of `project`, standing in `root`, asks for it, or at once
/// when its guardrail does not ask now (`unasked`).
pub(crate) fn send(
    project: &str,
    profile: &str,
    root: &Path,
    name: &str,
    unasked: bool,
) -> Result<Value, String> {
    let drafted = crate::reply_drafts::drafted(profile, name).ok_or_else(|| {
        format!("no draft waits for {name}: write it with devpit_draft_reply first")
    })?;
    if unasked {
        crate::live_sessions::reply_now(profile, name, &drafted, Some("the orchestrator"))
            .map_err(|err| err.message)?;
        let sent = json!({ "sent": name, "from": "orchestrator", "words": drafted });
        let did = format!("Sent the draft to {name}");
        return Ok(crate::guardrails::reported(
            root,
            devpit_rpc::Guardrail::SendDraft,
            &did,
            sent,
        ));
    }
    let press = crate::channels::Action::SendDraft {
        profile: profile.to_owned(),
        session: name.to_owned(),
    };
    if crate::channel_talk::offer(project, press) {
        return Ok(crate::channel_talk::pending(
            project,
            &format!("send the draft to {name}"),
        ));
    }
    let said = last_word(profile, root)?;
    let waiting = crate::reply_drafts::all()
        .iter()
        .filter(|(whose, _, _)| whose == profile)
        .count();
    if !lets_go(&said, name, waiting) {
        return Ok(waits(&said, name));
    }
    let mut held = spent()
        .lock()
        .map_err(|_| "the record of sends could not be read")?;
    if held.contains(&key_of(profile, &said, name)) {
        return Err(
            "not sent: that message of the person's already sent this; one message sends once"
                .to_owned(),
        );
    }
    let from = format!("{} («{}»)", said.from.label(), short(&said.text));
    crate::live_sessions::reply_now(profile, name, &drafted, Some(&from))
        .map_err(|err| err.message)?;
    let _ = spend(&mut held, key_of(profile, &said, name));
    Ok(json!({ "sent": name, "from": said.from.label(), "words": drafted }))
}

/// The person's last message in the log of `profile`'s orchestrator,
/// standing in `root`.
pub(crate) fn last_word(profile: &str, root: &Path) -> Result<Said, String> {
    last_said(&log_tail(profile, root)?)
        .ok_or_else(|| "devpit found nothing the person said in this conversation".to_owned())
}

/// The end of the log of `profile`'s orchestrator, standing in `root`.
pub(crate) fn log_tail(profile: &str, root: &Path) -> Result<String, String> {
    let config = crate::live_sessions::config_of(profile).map_err(|err| err.message)?;
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let log = devpit_agentcli::peers::newest(&devpit_agentcli::peers::logs_of(&config, &root))
        .ok_or(
            "devpit could not find this conversation's log, so it cannot read what the person said",
        )?;
    Ok(devpit_agentcli::peers::tail(&log))
}

/// A message cut to what a record needs.
pub(crate) fn short(text: &str) -> String {
    let mut cut: String = text.chars().take(80).collect();
    if text.chars().count() > 80 {
        cut.push('…');
    }
    cut
}

#[cfg(test)]
#[path = "draft_sending_tests.rs"]
mod tests;
