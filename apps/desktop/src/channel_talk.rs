//! The person talking to an orchestrator from a channel's paired chat: a
//! message or a voice note in, the orchestrator's answer out.
//!
//! A transport hands in what the paired chat said — Telegram through the
//! person's own bot, or through devpit.app's (`telegram_devpit`) — and only
//! the paired chat: a message from any other never reaches here. It becomes
//! the person's turn in the orchestrator's chat through the composer's own
//! door, marked with its channel. While that turn runs, what it would do on the person's consent —
//! send a draft, stop a session, start a lane's step — becomes a button under
//! the answer, and only the press does it: a yes typed or heard on a phone is
//! the agent's reading of words, a button says exactly what it does.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use devpit_rpc::{ChannelOrchestrator, Conversation, Part, Privacy, Role, Transcript, TurnFrom};
use serde_json::{json, Value};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{Emitter as _, Manager as _};

use crate::channels::Action;
use crate::remote_talk::{voice_kind, MOST_VOICE};

/// The longest message sent on a channel: Telegram takes 4096 characters.
pub(crate) const LONGEST_MESSAGE: usize = 4000;
/// How much of an answer's first line `Privacy::Titles` lets out.
const FIRST_LINE: usize = 200;

pub(crate) const NO_VOICE: &str = "Voice notes need devpit's voice model: set one up in Settings → Voice, on the computer. Text works meanwhile.";

/// What the paired chat said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Heard {
    Text(String),
    /// A voice note still on the channel, by the transport's handle for it.
    Voice {
        file: String,
        media_type: String,
        /// What the channel says it weighs, before any of it is fetched.
        size: Option<usize>,
    },
}

/// A channel's side: where answers go, and how a voice note is fetched.
pub(crate) trait Line {
    /// The channel's id, as a turn's `TurnFrom::Channel` names it.
    fn id(&self) -> &'static str;
    fn privacy(&self) -> Privacy;
    /// The orchestrator chosen for this channel in Settings, by project.
    fn orchestrator(&self) -> Option<String>;
    fn reply(&self, text: &str, actions: &[Action]) -> impl Future<Output = ()> + Send;
    /// A voice note's bytes, refused once past `most`.
    fn fetch(
        &self,
        file: &str,
        most: usize,
    ) -> impl Future<Output = Result<Vec<u8>, String>> + Send;
}

/// This machine's side: its orchestrators, its voice engine and the turn.
pub(crate) trait Desk {
    /// The orchestrators here, by project.
    fn orchestrators(&self) -> Vec<String>;
    fn voice_ready(&self) -> bool;
    fn hear(
        &self,
        ext: &str,
        bytes: Vec<u8>,
    ) -> impl Future<Output = Result<Transcript, String>> + Send;
    /// The person's turn in the chat of orchestrator `project`; answers the
    /// words it ended on.
    fn turn(
        &self,
        project: &str,
        words: &str,
        from: TurnFrom,
    ) -> impl Future<Output = Result<String, String>> + Send;
}

/// One channel's message at a time, in the order sent: two turns at once in
/// one chat would refuse the second.
fn talking() -> &'static tokio::sync::Mutex<()> {
    static TALKING: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    TALKING.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// What the paired chat of `line` said, taken to its orchestrator and
/// answered there.
pub(crate) async fn heard(line: &(impl Line + Sync), desk: &(impl Desk + Sync), said: Heard) {
    let _one = talking().lock().await;
    let words = match said {
        Heard::Text(text) => text,
        Heard::Voice {
            file,
            media_type,
            size,
        } => match listened(line, desk, &file, &media_type, size).await {
            Ok(words) => words,
            Err(why) => return line.reply(&why, &[]).await,
        },
    };
    let words = words.trim();
    if words.is_empty() {
        return;
    }
    let project = match which(&desk.orchestrators(), line.orchestrator().as_deref()) {
        Ok(project) => project,
        Err(why) => return line.reply(&why, &[]).await,
    };
    let privacy = line.privacy();
    let turn = Turn::begin(&project, line.id());
    let answer = desk
        .turn(
            &project,
            words,
            TurnFrom::Channel {
                id: line.id().to_owned(),
            },
        )
        .await;
    let offered = together(turn.end());
    let mut messages = match answer {
        Ok(answer) => shaped(&answer, privacy),
        // Why can carry the agent's own output, which minimal keeps in devpit.
        Err(_) if privacy == Privacy::Minimal => {
            vec!["The orchestrator did not take that: open devpit to see why.".to_owned()]
        }
        Err(why) => vec![format!("The orchestrator did not take that: {why}")],
    };
    if !offered.is_empty() {
        let asks: Vec<String> = offered
            .iter()
            .filter(|action| !matches!(action, Action::All(_)))
            .map(|action| asked(action, privacy))
            .collect();
        messages.push(format!("Waiting for your press:\n{}", asks.join("\n")));
    }
    let last = messages.len().saturating_sub(1);
    for (at, message) in messages.iter().enumerate() {
        let buttons = if at == last { offered.as_slice() } else { &[] };
        line.reply(message, buttons).await;
    }
}

/// A voice note made into words here, or why it was not.
async fn listened(
    line: &impl Line,
    desk: &impl Desk,
    file: &str,
    media_type: &str,
    size: Option<usize>,
) -> Result<String, String> {
    if !desk.voice_ready() {
        return Err(NO_VOICE.to_owned());
    }
    // Refused on what the channel says it weighs, before a byte is fetched.
    if size.is_some_and(|size| size > MOST_VOICE) {
        return Err(past_most(MOST_VOICE));
    }
    let ext = voice_kind(media_type, 0)?;
    let bytes = line.fetch(file, MOST_VOICE).await?;
    if bytes.len() > MOST_VOICE {
        return Err(past_most(MOST_VOICE));
    }
    let heard = desk.hear(ext, bytes).await?;
    heard.text.ok_or_else(|| {
        format!(
            "Nothing was made of that voice note: {}.",
            heard.note.as_deref().unwrap_or("nothing was heard in it")
        )
    })
}

pub(crate) fn past_most(most: usize) -> String {
    format!(
        "That voice note is past the {} MB devpit takes: send a shorter one.",
        most / 1_048_576
    )
}

/// Adds `piece` to a voice note being fetched, or refuses it — and lets go
/// of what came before — once past `most`.
pub(crate) fn gather(held: &mut Vec<u8>, piece: &[u8], most: usize) -> Result<(), String> {
    if held.len() + piece.len() > most {
        *held = Vec::new();
        return Err(past_most(most));
    }
    held.extend_from_slice(piece);
    Ok(())
}

/// The orchestrator a channel talks to: the one chosen in Settings, or the
/// only one there is.
pub(crate) fn which(orchestrators: &[String], chosen: Option<&str>) -> Result<String, String> {
    match (chosen, orchestrators) {
        (Some(chosen), _) if orchestrators.iter().any(|one| one == chosen) => Ok(chosen.to_owned()),
        (Some(_), _) => Err(
            "The orchestrator this chat talks to is gone: choose another in Settings → Channels."
                .to_owned(),
        ),
        (None, [only]) => Ok(only.clone()),
        (None, []) => Err(
            "There is no orchestrator on this computer yet: make one in devpit first.".to_owned(),
        ),
        (None, _) => {
            Err("Choose which orchestrator this chat talks to in Settings → Channels.".to_owned())
        }
    }
}

/// What a turn ended on: its answer's text after the last tool it used,
/// without what a subagent said — the chatter stays in devpit.
pub(crate) fn final_words(conversation: &Conversation, turn_id: &str) -> String {
    let Some(answer) = conversation
        .messages
        .iter()
        .rev()
        .find(|one| one.role == Role::Assistant && one.turn_id.as_deref() == Some(turn_id))
    else {
        return String::new();
    };
    let mut words: Vec<&str> = Vec::new();
    for part in &answer.parts {
        match part {
            Part::Text { text, parent: None } => words.push(text),
            Part::ToolCall { parent: None, .. } | Part::ToolResult { parent: None, .. } => {
                words.clear()
            }
            _ => {}
        }
    }
    words.join("\n\n").trim().to_owned()
}

/// The messages an answer makes on a channel kept at `privacy`.
pub(crate) fn shaped(answer: &str, privacy: Privacy) -> Vec<String> {
    let answer = answer.trim();
    if answer.is_empty() {
        return vec!["The orchestrator finished without a word.".to_owned()];
    }
    match privacy {
        Privacy::Minimal => vec!["The orchestrator answered: read it in devpit.".to_owned()],
        Privacy::Titles => {
            let first = answer.lines().find(|line| !line.trim().is_empty());
            let first = first.unwrap_or_default().trim();
            let mut said = cut_at_word(first, FIRST_LINE);
            if said.len() < answer.len() && !said.ends_with('…') {
                said.push('…');
            }
            vec![said]
        }
        Privacy::Full => split(answer, LONGEST_MESSAGE),
    }
}

/// `text` within `most` characters, cut at a word and said to be cut.
fn cut_at_word(text: &str, most: usize) -> String {
    let Some((end, _)) = text.char_indices().nth(most) else {
        return text.to_owned();
    };
    let cut = &text[..end];
    format!("{}…", cut.rsplit_once(' ').map_or(cut, |(head, _)| head))
}

/// `text` in pieces of at most `most` characters, broken at a paragraph, a
/// line or a word where one is near the end.
pub(crate) fn split(text: &str, most: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let Some((end, _)) = rest.char_indices().nth(most) else {
            pieces.push(rest.to_owned());
            break;
        };
        let head = &rest[..end];
        let at = ["\n\n", "\n", " "]
            .iter()
            .find_map(|gap| head.rfind(gap).filter(|at| *at > end / 2))
            .unwrap_or(end);
        pieces.push(rest[..at].trim_end().to_owned());
        rest = rest[at..].trim_start();
    }
    pieces
}

/// The buttons a turn asked for, and one more for each kind asked several
/// times: N stops, or N drafts, go in one press.
pub(crate) fn together(offered: Vec<Action>) -> Vec<Action> {
    let of = |kind: fn(&Action) -> bool| -> Vec<Action> {
        offered.iter().filter(|one| kind(one)).cloned().collect()
    };
    let stops = of(|one| matches!(one, Action::StopSession { .. }));
    let drafts = of(|one| matches!(one, Action::SendDraft { .. }));
    let mut all = offered;
    for several in [stops, drafts] {
        if several.len() > 1 {
            all.push(Action::All(several));
        }
    }
    all
}

/// What a button under an answer will do, as much as `privacy` lets out.
fn asked(action: &Action, privacy: Privacy) -> String {
    let named = privacy != Privacy::Minimal;
    let said = match action {
        Action::SendDraft { profile, session } if privacy == Privacy::Full => {
            match crate::reply_drafts::drafted(profile, session) {
                Some(draft) => format!("send {session}: «{draft}»"),
                None => format!("send the draft to {session}"),
            }
        }
        Action::SendDraft { session, .. } if named => format!("send the draft to {session}"),
        Action::SendDraft { .. } => "send a draft".to_owned(),
        Action::StopSession {
            session, losing, ..
        } => {
            let who = if named { session.as_str() } else { "a session" };
            match losing {
                Some(losing) => format!("stop {who}, which loses work: {losing}"),
                None => format!("stop {who}"),
            }
        }
        Action::MoveCard { card, lane, .. } if named => {
            format!("move «{card}» to {lane}, which starts its step")
        }
        Action::MoveCard { .. } => "move a card into a lane that starts its step".to_owned(),
        other => crate::channel_actions::label(other),
    };
    format!("• {said}")
}

/// A channel's turn running in an orchestrator's chat, and the buttons asked
/// for while it runs.
struct Asking {
    project: String,
    channel: &'static str,
    offered: Vec<Action>,
}

fn asking() -> &'static Mutex<Vec<Asking>> {
    static ASKING: OnceLock<Mutex<Vec<Asking>>> = OnceLock::new();
    ASKING.get_or_init(Mutex::default)
}

/// Held while a channel's turn runs; lets go of it however that ends.
struct Turn {
    project: String,
}

impl Turn {
    fn begin(project: &str, channel: &'static str) -> Turn {
        if let Ok(mut all) = asking().lock() {
            all.push(Asking {
                project: project.to_owned(),
                channel,
                offered: Vec::new(),
            });
        }
        Turn {
            project: project.to_owned(),
        }
    }

    /// The buttons the turn asked for.
    fn end(self) -> Vec<Action> {
        asking()
            .lock()
            .ok()
            .and_then(|mut all| {
                let at = all.iter().position(|one| one.project == self.project)?;
                Some(all.remove(at).offered)
            })
            .unwrap_or_default()
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        if let Ok(mut all) = asking().lock() {
            all.retain(|one| one.project != self.project);
        }
    }
}

/// While a channel's turn runs in orchestrator `project`, `action` waits
/// under its answer for the person's press. Whether it was put there.
pub(crate) fn offer(project: &str, action: Action) -> bool {
    let Ok(mut all) = asking().lock() else {
        return false;
    };
    let Some(turn) = all.iter_mut().find(|one| one.project == project) else {
        return false;
    };
    if !turn.offered.contains(&action) {
        turn.offered.push(action);
    }
    true
}

/// What the agent is told of an action put under its answer for a press.
pub(crate) fn pending(project: &str, what: &str) -> Value {
    let channel = asking()
        .lock()
        .ok()
        .and_then(|all| {
            let turn = all.iter().find(|one| one.project == project)?;
            Some(devpit_agentcli::person::From::Channel(turn.channel).label())
        })
        .unwrap_or("a channel");
    json!({
        "done": false,
        "asked": what,
        "note": format!("Not done yet: the person wrote from {channel}, where this waits for them to press the button devpit puts under your answer. Say what it will do; a typed yes does not do it."),
    })
}

/// The orchestrators on this machine.
pub(crate) fn orchestrators() -> Vec<ChannelOrchestrator> {
    crate::projects::project_list_unread_now()
        .map(|listed| listed.projects)
        .unwrap_or_default()
        .into_iter()
        .filter(|one| one.orchestrator.is_some())
        .map(|one| ChannelOrchestrator {
            project: one.id,
            name: one.name,
        })
        .collect()
}

/// The orchestrator's latest conversation on the account it speaks as now,
/// as the Remote opens it, or a new one: one begun on another stays there.
pub(crate) fn latest_conversation(project: &str) -> String {
    let speaks_as = crate::projects::project_list_unread_now()
        .ok()
        .and_then(|listed| listed.projects.into_iter().find(|one| one.id == project))
        .and_then(|one| one.orchestrator);
    crate::threads::chat_list_now(project.to_owned())
        .ok()
        .and_then(|listed| latest_on(listed.conversations, speaks_as.as_deref()))
        .unwrap_or_else(|| ulid::Ulid::generate().to_string().to_lowercase())
}

/// The first of `threads`, newest first, begun on `profile` or on none yet.
pub(crate) fn latest_on(threads: Vec<devpit_rpc::Thread>, profile: Option<&str>) -> Option<String> {
    threads
        .into_iter()
        .find(|one| profile.is_none_or(|profile| one.profile.is_empty() || one.profile == profile))
        .map(|one| one.id)
}

/// `ask` run as a turn the window was not asked for: it hears of it as of a
/// woken one, so a chat open on that conversation joins.
pub(crate) async fn turn_in(
    app: &tauri::AppHandle,
    conversation: &str,
    ask: devpit_rpc::Ask,
) -> Result<devpit_rpc::TurnEnd, devpit_rpc::RpcError> {
    let first = AtomicBool::new(true);
    let (woke, said) = (app.clone(), conversation.to_owned());
    let trigger = Channel::new(move |_: InvokeResponseBody| {
        if first.swap(false, Ordering::SeqCst) {
            let (app, conversation) = (woke.clone(), said.clone());
            tauri::async_runtime::spawn(async move {
                let _ = app.emit(crate::chat_resident::WOKE, &conversation);
            });
        }
        Ok(())
    });
    crate::chat::chat_send(
        app.clone(),
        app.state(),
        app.state(),
        app.state(),
        ask,
        trigger,
    )
    .await
}

/// This machine, as a channel's messages reach it.
pub(crate) struct Here(pub tauri::AppHandle);

impl Desk for Here {
    fn orchestrators(&self) -> Vec<String> {
        orchestrators().into_iter().map(|one| one.project).collect()
    }

    fn voice_ready(&self) -> bool {
        crate::transcribe::voice_ready()
    }

    async fn hear(&self, ext: &str, bytes: Vec<u8>) -> Result<Transcript, String> {
        let dir = devpit_core::Store::root().map_err(|err| err.to_string())?;
        crate::remote_talk::heard(&dir, ext, &bytes, |audio| async move {
            crate::transcribe::heard_in(&audio, None).await
        })
        .await
    }

    async fn turn(&self, project: &str, words: &str, from: TurnFrom) -> Result<String, String> {
        let app = &self.0;
        let conversation = latest_conversation(project);
        let ask = crate::remote_talk::turn_of(project, &conversation, words, from)?;
        let end = turn_in(app, &conversation, ask)
            .await
            .map_err(|err| err.message)?;
        let read = crate::chat::chat_history_now(project.to_owned(), conversation)
            .map_err(|err| err.message)?;
        let words = final_words(&read, &end.turn_id);
        match end.stop_reason {
            Some(why) if words.is_empty() && end.is_error => Err(why),
            _ => Ok(words),
        }
    }
}

#[cfg(test)]
#[path = "channel_talk_tests.rs"]
mod tests;
