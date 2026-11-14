//! Talking to the orchestrators and their sessions from the Remote: what
//! they said, the person's turn sent from the phone, and the photos and
//! recordings a phone sends in pieces.
//!
//! A turn goes through the composer's own `chat.send`, marked as the
//! Remote's, and reaches the device the way it reaches a chat reopened
//! mid-turn: joined through `chat_relay`. A session's words are its own
//! transcript, and a message to it is typed into its terminal as the person.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use devpit_core::home::ProjectHome;
use devpit_core::Store;
use devpit_rpc::{
    Ask, Attachment, Conversation, Frame, Part, RemoteOrchestrator, RemoteOut, RemoteSession,
    RemoteTalks, RpcError, Transcript, TurnEnd, TurnFrom,
};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{Emitter as _, Manager as _};

use crate::chat_relay::Relay;
use crate::pasting::{extension_for, MOST_BYTES};

/// The latest messages of a conversation a phone is sent.
const PHONE_MESSAGES: usize = 80;
/// The most of a tool's output, a thought or a command's echo a phone is sent.
const PHONE_OUTPUT: usize = 2_000;
/// The most of a tool's input: past it the subject a row names may be cut.
const PHONE_INPUT: usize = 16_000;
/// The longest message the person sends from the phone.
const LONGEST_SAID: usize = 20_000;
/// The most a recording from the phone may be: two minutes of speech is far
/// under it in any format a phone records.
pub(crate) const MOST_VOICE: usize = 3 * 1024 * 1024;
/// Files one connection may be sending at once.
const MOST_UPLOADS: usize = 2;

/// The conversations whose turns a connection is being sent.
type Following = Arc<Mutex<HashSet<String>>>;

/// What one connection holds for talking.
#[derive(Default)]
pub(crate) struct Talk {
    pub uploads: Uploads,
    following: Following,
    /// The conversation the device last opened, and its project.
    viewing: Arc<Mutex<Option<(String, String)>>>,
}

impl Talk {
    /// See [`follow`].
    pub(crate) fn follow(
        &self,
        app: &tauri::AppHandle,
        out: &Sender<RemoteOut>,
        project: &str,
        conversation: &str,
    ) {
        follow(
            &app.state::<Relay>(),
            out,
            &self.following,
            project,
            conversation,
        );
    }

    /// The conversation the device reads now.
    pub(crate) fn view(&self, project: &str, conversation: &str) {
        if let Ok(mut viewing) = self.viewing.lock() {
            *viewing = Some((project.to_owned(), conversation.to_owned()));
        }
    }

    /// For [`Relay::watch`]: a turn entering the conversation the device
    /// reads is followed, wherever it began.
    pub(crate) fn watching(
        &self,
        out: Sender<RemoteOut>,
    ) -> impl Fn(&Relay, &str) + Send + Sync + 'static {
        let (viewing, following) = (self.viewing.clone(), self.following.clone());
        move |relay, conversation| {
            let project = viewing.lock().ok().and_then(|viewing| {
                viewing
                    .as_ref()
                    .filter(|(_, read)| read == conversation)
                    .map(|(project, _)| project.clone())
            });
            if let Some(project) = project {
                follow(relay, &out, &following, &project, conversation);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn follows(&self, conversation: &str) -> bool {
        self.following
            .lock()
            .is_ok_and(|held| held.contains(conversation))
    }
}

/// Files a phone is sending in pieces, by the id it gave each.
#[derive(Default)]
pub(crate) struct Uploads(HashMap<String, Vec<u8>>);

impl Uploads {
    /// Adds a piece. Refused before it is decoded when the file would pass
    /// the most any file may be, and the file is dropped with it.
    pub(crate) fn add(&mut self, upload: &str, b64: &str) -> Result<(), String> {
        if !plain(upload) {
            return Err("that is not an upload".to_owned());
        }
        if !self.0.contains_key(upload) && self.0.len() >= MOST_UPLOADS {
            return Err("two files at a time — wait for one to arrive".to_owned());
        }
        let held = self.0.get(upload).map_or(0, Vec::len);
        // Base64 is four characters for three bytes.
        if held + b64.len() / 4 * 3 > MOST_BYTES {
            self.0.remove(upload);
            return Err(format!(
                "that file is past the {} MB the Remote takes",
                MOST_BYTES / 1_048_576
            ));
        }
        let bytes = BASE64
            .decode(b64)
            .map_err(|_| "that piece is not base64".to_owned())?;
        self.0.entry(upload.to_owned()).or_default().extend(bytes);
        Ok(())
    }

    pub(crate) fn take(&mut self, upload: &str) -> Option<Vec<u8>> {
        self.0.remove(upload)
    }
}

/// An id a device may name a file or a conversation by: what the window and
/// the viewer make, and nothing that reaches out of a folder.
pub(crate) fn plain(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The orchestrators, each with its latest conversation, and the sessions of
/// their accounts.
pub(crate) fn talks() -> Result<RemoteTalks, String> {
    let projects = crate::projects::project_list_unread_now()
        .map_err(|err| err.message)?
        .projects;
    let orchestrators: Vec<RemoteOrchestrator> = projects
        .into_iter()
        .filter_map(|one| {
            let profile = one.orchestrator?;
            let latest = crate::threads::chat_list_now(one.id.clone())
                .ok()
                .and_then(|listed| listed.conversations.into_iter().next());
            Some(RemoteOrchestrator {
                conversation: latest.map_or_else(
                    || ulid::Ulid::generate().to_string().to_lowercase(),
                    |thread| thread.id,
                ),
                project: one.id,
                name: one.name,
                profile,
            })
        })
        .collect();
    let mut accounts: Vec<&str> = orchestrators
        .iter()
        .map(|one| one.profile.as_str())
        .collect();
    accounts.sort_unstable();
    accounts.dedup();
    let sessions = accounts
        .into_iter()
        .flat_map(|profile| {
            crate::live_sessions::orchestrator_sessions_now(profile)
                .map(|live| live.sessions)
                .unwrap_or_default()
                .into_iter()
                .map(move |session| RemoteSession {
                    profile: profile.to_owned(),
                    session,
                })
        })
        .collect();
    Ok(RemoteTalks {
        orchestrators,
        sessions,
        voice: crate::transcribe::voice_ready(),
    })
}

/// A conversation as a phone is sent it: the latest messages, each long
/// output cut — the screen shows a line of it.
pub(crate) fn for_phone(mut conversation: Conversation) -> Conversation {
    let skip = conversation.messages.len().saturating_sub(PHONE_MESSAGES);
    conversation.messages.drain(..skip);
    for part in conversation
        .messages
        .iter_mut()
        .flat_map(|message| message.parts.iter_mut())
    {
        cut_part(part);
    }
    conversation
}

fn cut_part(part: &mut Part) {
    match part {
        Part::ToolCall { input, .. } => cut(input, PHONE_INPUT),
        Part::ToolResult { output: long, .. }
        | Part::Thinking { text: long, .. }
        | Part::Command { content: long }
        | Part::Unknown { text: long } => cut(long, PHONE_OUTPUT),
        _ => {}
    }
}

fn cut(text: &mut String, most: usize) {
    if let Some((at, _)) = text.char_indices().nth(most) {
        text.truncate(at);
        text.push('…');
    }
}

fn frame_for_phone(mut frame: Frame) -> Frame {
    match &mut frame {
        Frame::Part { part, .. } => cut_part(part),
        Frame::Opened { message } => message.parts.iter_mut().for_each(cut_part),
        _ => {}
    }
    frame
}

/// The turn the person's words from the Remote or a channel make: as the
/// account the conversation belongs to — the orchestrator's, for one that has
/// had no turn yet — with the model and mode it last ran with, and marked
/// with where they came from: nowhere, for a turn devpit starts itself.
pub(crate) fn ask_of(
    conversation: &Conversation,
    orchestrator: Option<&str>,
    root: &str,
    text: &str,
    from: Option<TurnFrom>,
) -> Result<Ask, String> {
    let profile = Some(conversation.profile.as_str())
        .filter(|profile| !profile.is_empty())
        .or(orchestrator)
        .ok_or("this chat has no account yet — start it on the machine")?;
    let text = text.trim();
    if text.is_empty() || text.chars().count() > LONGEST_SAID {
        return Err(format!(
            "a message is between 1 and {LONGEST_SAID} characters"
        ));
    }
    Ok(Ask {
        project_id: conversation.project_id.clone(),
        conversation_id: conversation.id.clone(),
        profile_id: profile.to_owned(),
        model: conversation.model.clone(),
        prompt: text.to_owned(),
        cwd: conversation.cwd.clone().unwrap_or_else(|| root.to_owned()),
        budget_usd: None,
        permission: conversation.permission.clone(),
        effort: conversation.effort.clone(),
        from,
    })
}

/// The turn `text` makes in `conversation` of `project`, sent from `from`.
pub(crate) fn turn_of(
    project: &str,
    conversation: &str,
    text: &str,
    from: TurnFrom,
) -> Result<Ask, String> {
    turn_from(project, conversation, text, Some(from))
}

/// The turn devpit starts itself in `conversation` of `project`: a session's
/// message or an idle notice, which is no one's word.
pub(crate) fn devpits_turn(project: &str, conversation: &str, text: &str) -> Result<Ask, String> {
    turn_from(project, conversation, text, None)
}

fn turn_from(
    project: &str,
    conversation: &str,
    text: &str,
    from: Option<TurnFrom>,
) -> Result<Ask, String> {
    if !plain(conversation) {
        return Err("that is not a conversation".to_owned());
    }
    let history = crate::chat::chat_history_now(project.to_owned(), conversation.to_owned())
        .map_err(|err| err.message)?;
    let store = crate::projects::store().map_err(|err| err.message)?;
    let (_, root) = crate::projects::locate(&store, project).map_err(|err| err.message)?;
    let orchestrator = Store::root()
        .ok()
        .and_then(|home| home.canonicalize().ok())
        .and_then(|home| devpit_core::home::orchestrator_of(&store, &home, &root));
    ask_of(
        &history,
        orchestrator.as_deref(),
        &root.display().to_string(),
        text,
        from,
    )
}

/// The person's message from the phone, sent as their turn. Its frames reach
/// the device through the relay, joined once the turn is there; the window
/// is told the way it is of a woken turn, so a chat open on it joins too.
pub(crate) fn say(
    app: &tauri::AppHandle,
    out: &Sender<RemoteOut>,
    talk: &Talk,
    project: &str,
    conversation: &str,
    text: &str,
) -> Result<(), String> {
    let ask = turn_of(project, conversation, text, TurnFrom::Remote)?;
    // Only a trigger: the first frame is sent once the turn is in the relay,
    // under its lock, so what joins it is started on its own.
    let first = AtomicBool::new(true);
    let woke = (app.clone(), out.clone(), talk.following.clone());
    let (project, conversation) = (project.to_owned(), conversation.to_owned());
    let (at, said) = (project.clone(), conversation.clone());
    let trigger = Channel::new(move |_: InvokeResponseBody| {
        if first.swap(false, Ordering::SeqCst) {
            let (app, out, following) = woke.clone();
            let (project, conversation) = (at.clone(), said.clone());
            tauri::async_runtime::spawn(async move {
                let _ = app.emit(crate::chat_resident::WOKE, &conversation);
                follow(
                    &app.state::<Relay>(),
                    &out,
                    &following,
                    &project,
                    &conversation,
                );
            });
        }
        Ok(())
    });
    let (app, out) = (app.clone(), out.clone());
    tauri::async_runtime::spawn(async move {
        let sent = crate::chat::chat_send(
            app.clone(),
            app.state(),
            app.state(),
            app.state(),
            ask,
            trigger,
        )
        .await;
        match sent {
            // A turn short enough to end before it was joined reaches the
            // phone as it was written down.
            Ok(_) => {
                let read = crate::chat::chat_history_now(project.clone(), conversation);
                if let Ok(read) = read {
                    let _ = out.send(RemoteOut::Chat {
                        project,
                        conversation: Box::new(for_phone(read)),
                    });
                }
            }
            // Nothing else ends it on the phone: a refused turn sends no end.
            Err(err) => refused(&out, project, conversation, err.message),
        }
    });
    Ok(())
}

/// A turn that could not run, ended on the phone and said why.
fn refused(out: &Sender<RemoteOut>, project: String, conversation: String, why: String) {
    let ended = Frame::Ended {
        end: TurnEnd {
            turn_id: String::new(),
            cost_usd: None,
            duration_ms: None,
            stop_reason: Some(why.clone()),
            is_error: true,
            context: None,
            ..Default::default()
        },
    };
    let _ = out.send(RemoteOut::ChatFrame {
        project,
        conversation,
        frame: Box::new(ended),
    });
    let _ = out.send(RemoteOut::Failed { why });
}

/// Sends this connection the turns running in a conversation, from where
/// they are, unless it is already being sent them.
fn follow(
    relay: &Relay,
    out: &Sender<RemoteOut>,
    following: &Following,
    project: &str,
    conversation: &str,
) {
    let relay = relay.clone();
    {
        let Ok(mut held) = following.lock() else {
            return;
        };
        if held.contains(conversation) || !relay.running(conversation) {
            return;
        }
        held.insert(conversation.to_owned());
    }
    let channel = forwarding(out.clone(), project, conversation);
    let (following, key) = (following.clone(), conversation.to_owned());
    tauri::async_runtime::spawn(async move {
        loop {
            relay.rejoin(&key, channel.clone()).await;
            // Let go under the lock a new turn's trigger takes, so a turn
            // that began meanwhile is joined here rather than missed.
            let again = {
                let Ok(mut held) = following.lock() else {
                    return;
                };
                let again = relay.running(&key);
                if !again {
                    held.remove(&key);
                }
                again
            };
            if !again {
                return;
            }
        }
    });
}

/// A channel whose frames go to the device, cut for a phone.
fn forwarding(out: Sender<RemoteOut>, project: &str, conversation: &str) -> Channel<Frame> {
    let (project, conversation) = (project.to_owned(), conversation.to_owned());
    Channel::new(move |body: InvokeResponseBody| {
        if let Ok(frame) = body.deserialize::<Frame>() {
            let _ = out.send(RemoteOut::ChatFrame {
                project: project.clone(),
                conversation: conversation.clone(),
                frame: Box::new(frame_for_phone(frame)),
            });
        }
        Ok(())
    })
}

/// What a session of `profile` said, as messages, the latest last.
pub(crate) fn session(profile: &str, name: &str) -> RemoteOut {
    let path = match crate::session_told::transcript_of(profile, name) {
        Ok(path) => path,
        Err(err) => return RemoteOut::Failed { why: err.message },
    };
    let mut messages = path
        .filter(|path| path.is_file())
        .map(|path| crate::adopted_history::from_transcript(&path, 0.0))
        .unwrap_or_default();
    let skip = messages.len().saturating_sub(PHONE_MESSAGES);
    messages.drain(..skip);
    RemoteOut::Session {
        profile: profile.to_owned(),
        name: name.to_owned(),
        messages,
    }
}

/// A photo from the phone, kept in the project's pasted pictures the way a
/// picture pasted into the composer is; the message names it by its path.
pub(crate) fn photo(
    store: &Store,
    root: &Path,
    project: &str,
    media_type: &str,
    bytes: &[u8],
) -> Result<Attachment, String> {
    let ext = extension_for(media_type)
        .filter(|_| media_type.starts_with("image/"))
        .ok_or("a photo is a PNG, JPEG, GIF or WebP")?;
    if bytes.len() > MOST_BYTES {
        return Err(format!("a photo is at most {} MB", MOST_BYTES / 1_048_576));
    }
    let home = ProjectHome::of(store, root, project).map_err(|_| "that is not a project here")?;
    crate::pasting::kept_in(&home.pasted(), media_type, ext, bytes).map_err(|err| err.message)
}

/// The extension a recording from the phone is kept under, or why it is
/// refused.
pub(crate) fn voice_kind(media_type: &str, size: usize) -> Result<&'static str, String> {
    let ext = extension_for(media_type)
        .filter(|_| media_type.starts_with("audio/"))
        .ok_or("that is not a recording")?;
    if size > MOST_VOICE {
        return Err(format!(
            "a recording is at most {} MB, about two minutes",
            MOST_VOICE / 1_048_576
        ));
    }
    Ok(ext)
}

/// A recording made into words by `hear`, in a file of its own under `dir`
/// that goes once it is heard: only the words go back to the phone.
pub(crate) async fn heard<F, Fut>(
    dir: &Path,
    ext: &str,
    bytes: &[u8],
    hear: F,
) -> Result<Transcript, String>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: Future<Output = Result<Transcript, RpcError>>,
{
    let audio = dir.join(format!(
        "voice-remote-{}.{ext}",
        ulid::Ulid::generate().to_string().to_lowercase()
    ));
    std::fs::write(&audio, bytes).map_err(|err| err.to_string())?;
    let words = hear(audio.clone()).await;
    let _ = std::fs::remove_file(&audio);
    words.map_err(|err| err.message)
}

/// [`heard`], by the engine chosen in Settings, answered on the device's
/// queue when it is done: a transcription can take a while.
pub(crate) fn hear(
    out: &Sender<RemoteOut>,
    root: &Path,
    upload: String,
    ext: &str,
    bytes: Vec<u8>,
) {
    let (out, root, ext) = (out.clone(), root.to_path_buf(), ext.to_owned());
    tauri::async_runtime::spawn(async move {
        let words = heard(&root, &ext, &bytes, |audio| async move {
            crate::transcribe::heard_in(&audio, None).await
        })
        .await;
        let _ = out.send(match words {
            Ok(transcript) => RemoteOut::Heard { upload, transcript },
            Err(why) => RemoteOut::Failed { why },
        });
    });
}

#[cfg(test)]
#[path = "remote_talk_tests.rs"]
mod tests;
