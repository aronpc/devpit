//! Compacting an idle Claude conversation with the CLI's own `/compact`.
//!
//! Once a conversation sits idle past the prompt cache's life, its next turn
//! reads the whole context again uncached. A profile can have a long one
//! compacted when the person returns to it, or just before the cache expires.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use devpit_agentcli::driver::Driver;
use devpit_agentcli::head::{head_path, read_head, remaining, Head};
use devpit_agentcli::store::{append, conversation_path};
use devpit_agentcli::talk::Say;
use devpit_core::{preference, Store};
use devpit_rpc::{Frame, Message, Part, Role, RpcError};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager};

use crate::chat_resident::Staying;

/// When an idle conversation is compacted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum IdleWhen {
    /// On the person's next turn, once the prompt cache has expired.
    Return,
    /// Just before the prompt cache expires.
    Expiry,
    #[default]
    Never,
}

/// A profile's rule: when, and past how many tokens of context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IdleCompact {
    pub when: IdleWhen,
    pub tokens: u32,
}

impl Default for IdleCompact {
    fn default() -> Self {
        Self {
            when: IdleWhen::Never,
            tokens: 200_000,
        }
    }
}

/// How long the prompt cache lives after its last read, in seconds.
const CACHE_LIFE: f64 = 300.0;
/// How long before it expires a compaction starts: the CLI has to start and
/// read the cache within it.
const AHEAD: f64 = 60.0;
const COMPACT: &str = "/compact";

fn rules(store: &Store) -> HashMap<String, IdleCompact> {
    store
        .preference(preference::AGENTS_IDLE_COMPACT)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub(crate) fn rule_of(store: &Store, profile_id: &str) -> IdleCompact {
    rules(store).get(profile_id).copied().unwrap_or_default()
}

pub(crate) fn written(store: &Store, profile_id: &str, rule: IdleCompact) -> Result<(), RpcError> {
    let mut all = rules(store);
    all.insert(profile_id.to_owned(), rule);
    let json = serde_json::to_string(&all).map_err(|err| RpcError::internal(err.to_string()))?;
    Ok(store.set_preference(preference::AGENTS_IDLE_COMPACT, &json)?)
}

/// `agents.idle_compact_read`
#[tauri::command]
#[specta::specta]
pub async fn idle_compact_read(profile_id: String) -> Result<IdleCompact, RpcError> {
    crate::off_main::blocking(move || Ok(rule_of(&Store::open_default()?, &profile_id))).await
}

/// `agents.idle_compact_write`
#[tauri::command]
#[specta::specta]
pub async fn idle_compact_write(
    profile_id: String,
    rule: IdleCompact,
) -> Result<IdleCompact, RpcError> {
    crate::off_main::blocking(move || {
        let store = Store::open_default()?;
        written(&store, &profile_id, rule)?;
        Ok(rule_of(&store, &profile_id))
    })
    .await
}

/// What decides a compaction on the person's turn, read before it runs.
pub(crate) struct Idle {
    pub rule: IdleCompact,
    /// How full the context was when the last turn ended.
    pub used: Option<u32>,
    /// Seconds since anything was said in the conversation.
    pub idle: f64,
    pub head: PathBuf,
}

impl Idle {
    pub(crate) fn of(
        profile_id: &str,
        head: Option<&Head>,
        transcript: &Path,
        file: &Path,
    ) -> Self {
        Self {
            rule: crate::projects::store()
                .map(|store| rule_of(&store, profile_id))
                .unwrap_or_default(),
            used: head
                .and_then(|head| head.context)
                .map(|context| context.used),
            idle: idle_since(transcript, SystemTime::now()),
            head: file.to_owned(),
        }
    }
}

/// Seconds since the transcript was last written. One not written yet is not idle.
fn idle_since(transcript: &Path, now: SystemTime) -> f64 {
    std::fs::metadata(transcript)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|at| now.duration_since(at).ok())
        .map_or(0.0, |idle| idle.as_secs_f64())
}

fn over(rule: IdleCompact, used: Option<u32>) -> bool {
    used.is_some_and(|used| used > rule.tokens)
}

/// What a compaction leaves: the session the next turn resumes, and the note.
pub(crate) struct Compacted {
    pub session_id: Option<String>,
    pub note: Message,
}

/// On the person's turn: compacts first when they come back to a conversation
/// past the threshold whose cache has expired, and shows the note.
pub(crate) fn on_return(
    idle: &Idle,
    staying: Option<Staying>,
    driver: &dyn Driver,
    turn: &Say<'_>,
    frames: &Channel<Frame>,
) -> Option<Compacted> {
    let due = idle.rule.when == IdleWhen::Return
        && over(idle.rule, idle.used)
        && idle.idle >= CACHE_LIFE
        && turn.fork_at.is_none();
    if !due {
        return None;
    }
    let compacted = compact(staying, driver, turn, idle.used?, &idle.head)?;
    let _ = frames.send(Frame::Opened {
        message: compacted.note.clone(),
    });
    Some(compacted)
}

/// The person's turn, on the session a compaction before it ended in.
pub(crate) fn resuming<'a>(compacted: &'a Option<Compacted>, turn: &Say<'a>) -> Say<'a> {
    let session = compacted
        .as_ref()
        .and_then(|done| done.session_id.as_deref());
    Say {
        session_id: session.or(turn.session_id),
        ..*turn
    }
}

/// What the expiry timer does next, `idle` seconds after the last word.
#[derive(Debug, PartialEq)]
pub(crate) enum Next {
    Compact,
    Wait(f64),
    Stop,
}

pub(crate) fn next(rule: IdleCompact, used: Option<u32>, idle: f64) -> Next {
    // Past the cache's life the read is uncached either way: left for the person.
    if rule.when != IdleWhen::Expiry || !over(rule, used) || idle >= CACHE_LIFE {
        Next::Stop
    } else if idle >= CACHE_LIFE - AHEAD {
        Next::Compact
    } else {
        Next::Wait(CACHE_LIFE - AHEAD - idle)
    }
}

/// `/compact` on the conversation, through its resident process or a resumed
/// one, with the head settled onto where it ended. `None` when it failed: the
/// conversation goes on as it was.
pub(crate) fn compact(
    staying: Option<Staying>,
    driver: &dyn Driver,
    turn: &Say<'_>,
    tokens: u32,
    head: &Path,
) -> Option<Compacted> {
    let compacting = Say {
        prompt: COMPACT,
        fork_at: None,
        control: None,
        on_session: None,
        ..*turn
    };
    // Only through a process already running: one started here would keep
    // this turn's lack of a window to ask in.
    let staying = staying
        .filter(|staying| crate::chat_resident::running(&staying.app, &staying.conversation_id));
    let said = crate::chat_resident::or_say(staying, driver, &compacting, || {}, |_| {}, |_| true);
    let said = match said {
        Ok(said) if !said.end.is_error => said,
        Ok(said) => {
            let why = said.end.stop_reason.unwrap_or_default();
            devpit_core::reports::background("idle compaction", &why);
            return None;
        }
        Err(err) => {
            devpit_core::reports::background("idle compaction", &err);
            return None;
        }
    };
    // No anchor: a rewind counts the person's turns, and this is none of them.
    crate::chat_turn::settle(head, None, "", &said.end, said.session_id.clone(), None);
    Some(Compacted {
        session_id: said.session_id,
        note: Message {
            id: format!("msg_{}", ulid::Ulid::generate()),
            turn_id: None,
            role: Role::System,
            parts: vec![Part::Compacted { tokens }],
            created_at: crate::chat_listening::now(),
            streaming: false,
            ended: None,
        },
    })
}

/// Conversations a timer waits on, so a turn ending again starts no second one.
static ARMED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// After a person's turn: a profile set to compact just before the cache
/// expires gets a timer that waits for the conversation to sit idle that long.
pub(crate) fn arm(
    app: &AppHandle,
    project_id: &str,
    conversation_id: &str,
    profile_id: &str,
    cwd: &str,
) {
    let Ok(store) = crate::projects::store() else {
        return;
    };
    if rule_of(&store, profile_id).when != IdleWhen::Expiry {
        return;
    }
    {
        let Ok(mut armed) = ARMED.lock() else {
            return;
        };
        if armed.iter().any(|one| one == conversation_id) {
            return;
        }
        armed.push(conversation_id.to_owned());
    }
    let (app, project, conversation, cwd) = (
        app.clone(),
        project_id.to_owned(),
        conversation_id.to_owned(),
        cwd.to_owned(),
    );
    let _ = std::thread::Builder::new()
        .name("idle-compact".to_owned())
        .spawn(move || {
            watch(&app, &project, &conversation, &cwd);
            if let Ok(mut armed) = ARMED.lock() {
                armed.retain(|one| *one != conversation);
            }
        });
}

fn watch(app: &AppHandle, project_id: &str, conversation_id: &str, cwd: &str) {
    let Ok(sessions) = crate::projects::project_home(project_id).map(|home| home.sessions()) else {
        return;
    };
    let file = head_path(&sessions, conversation_id);
    let transcript = conversation_path(&sessions, conversation_id);
    loop {
        let Some(head) = read_head(&file) else {
            return;
        };
        let Ok(store) = crate::projects::store() else {
            return;
        };
        let used = head.context.map(|context| context.used);
        match next(
            rule_of(&store, &head.profile),
            used,
            idle_since(&transcript, SystemTime::now()),
        ) {
            Next::Stop => return,
            Next::Wait(secs) => std::thread::sleep(Duration::from_secs_f64(secs)),
            Next::Compact if head.rewind.fork_at.is_some() => return,
            Next::Compact => {
                let talking = app.state::<crate::chat::Talking>();
                let steering = app.state::<crate::steering::Steering>();
                // A turn running now ends in the transcript, and the wait starts again from there.
                let Ok(guard) = talking.begin(conversation_id, &steering) else {
                    std::thread::sleep(Duration::from_secs_f64(CACHE_LIFE - AHEAD));
                    continue;
                };
                let tokens = used.unwrap_or_default();
                if let Some(compacted) = resumed(
                    app,
                    project_id,
                    conversation_id,
                    &sessions,
                    cwd,
                    &head,
                    tokens,
                ) {
                    let _ = append(&transcript, &compacted.note);
                    // An open chat reads what came after it, as it does for a woken turn.
                    let _ = app.emit(crate::chat_resident::WOKE, conversation_id);
                }
                drop(guard);
                return;
            }
        }
    }
}

/// The compaction a timer runs: the turn as `chat.send` would start it.
fn resumed(
    app: &AppHandle,
    project_id: &str,
    conversation_id: &str,
    sessions: &Path,
    cwd: &str,
    head: &Head,
    tokens: u32,
) -> Option<Compacted> {
    let (profile, path) = crate::chat_turn::spawnable(&head.profile).ok()?;
    let driver = devpit_agentcli::driver::driver_for(&profile.driver, &profile.base)?;
    let locale = crate::session_locale::of_conversation(conversation_id, false);
    let env = crate::chat_turn::env_of(locale, &profile, conversation_id);
    let mcp = crate::agent_reach::chat_mcp(locale, &profile);
    let reach = crate::orchestrator::reaches(project_id);
    let address = crate::orchestrator::address_of(project_id);
    let hooks = crate::steps::hook_settings_for(Some(&head.profile));
    let cwd = crate::chat_turn::turn_cwd(head.cwd.as_deref(), cwd).ok()?;
    let mode = head.permission.as_deref();
    // Only through a process this conversation already has: starting one
    // would close the one another conversation of the project listens in.
    let staying = crate::chat_resident::running(app, conversation_id)
        .then(|| {
            crate::chat_resident::staying(
                app,
                project_id,
                conversation_id,
                sessions,
                &profile,
                mode,
            )
        })
        .flatten();
    let turn = Say {
        command: &path,
        env: &env,
        prompt: COMPACT,
        cwd: Path::new(&cwd),
        model: head.model.as_deref(),
        budget_usd: remaining(Some(head)),
        session_id: head.session_id.as_deref(),
        fork_at: None,
        permission: mode,
        disallowed: &[],
        settings: hooks.as_deref(),
        setting_sources: None,
        mcp_config: mcp.as_deref(),
        effort: head.effort.as_deref(),
        control: None,
        on_session: None,
        add_dirs: &reach,
        name: address.as_deref(),
    };
    compact(
        staying,
        driver.as_ref(),
        &turn,
        tokens,
        &head_path(sessions, conversation_id),
    )
}

#[cfg(test)]
#[path = "idle_compact_tests.rs"]
mod tests;
