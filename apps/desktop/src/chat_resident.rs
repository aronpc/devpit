//! A chat's conversation, kept running between turns.
//!
//! Its process stays (`devpit_agentcli::resident`) and the person's turns are
//! lines on its stdin: a turn starts without starting the CLI, a model or mode
//! switch is a control request, and Stop interrupts the turn, the same way in
//! every chat. A chat's process goes after a while idle, and its next turn
//! resumes the session in a new one; one that dies is replaced the same way.
//!
//! An orchestrator's never idles out. It is also spoken to by the sessions it
//! follows — "done", "blocked", the notice it asked for when one went idle —
//! and those reach only a process that is still there. A turn some other
//! session wakes is written into the conversation here, streamed through the
//! relay, and announced so an open chat joins it.
//!
//! Only outside the supervised mode: asking before a tool runs is held per
//! turn by the window that sent it, and a woken turn has no such window.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use devpit_agentcli::driver::Driver;
use devpit_agentcli::resident::{Heard, Resident};
use devpit_agentcli::talk::{say, Said, Say};
use devpit_agentcli::AgentError;
use devpit_rpc::{ErrorCode, Frame, Message, Part, Role, RpcError};
use serde_json::{json, Value};
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager};

use crate::chat_listening::{Ear, Woken};

/// The event a woken turn is announced on, with its conversation id.
pub(crate) const WOKE: &str = "chat:woke";

/// How long a chat's process waits for its next turn before it is let go.
const IDLE: Duration = Duration::from_secs(10 * 60);

/// How long a model or mode switch waits for the CLI to take it.
const SWITCH_WAITS: Duration = Duration::from_secs(5);
const SWITCH: &str = "switch_";

/// Numbers the turns: an idle-out sees whether another came since.
static TURNS: AtomicU64 = AtomicU64::new(1);

/// What stays for one conversation.
struct Live {
    resident: Resident,
    /// The flags it was started with: a turn asking for others restarts it.
    started_as: String,
    /// The model and mode it runs in now: a turn asking for others switches it.
    model: Option<String>,
    permission: Option<String>,
    /// Where the CLI's answers to a switch go, while a turn waits for them.
    switching: Arc<Mutex<Option<mpsc::Sender<Value>>>>,
    project_id: String,
    /// An orchestrator's: it listens for its sessions, so it never idles out.
    listens: bool,
    /// The last turn it took.
    turn: u64,
    /// The CLI's session, once its stream names it.
    session: Arc<Mutex<Option<String>>>,
    ear: Arc<Ear>,
}

#[derive(Default)]
pub(crate) struct Residents(Mutex<HashMap<String, Live>>);

impl Residents {
    /// Takes this conversation's process out when `when` holds for it.
    fn taken(&self, conversation_id: &str, when: impl Fn(&Live) -> bool) -> Option<Live> {
        let mut all = self.0.lock().ok()?;
        all.get(conversation_id).filter(|live| when(live))?;
        all.remove(conversation_id)
    }

    /// Waits `idle`, then lets a chat's process go when no turn came since
    /// turn `stamp` and it is not reachable.
    fn idle_out(
        &self,
        conversation_id: &str,
        stamp: u64,
        idle: Duration,
        reachable: impl Fn(&str) -> bool,
    ) {
        std::thread::sleep(idle);
        // Remote Control lives in the process.
        if reachable(conversation_id) {
            return;
        }
        if let Some(live) = self.taken(conversation_id, |live| !live.listens && live.turn == stamp)
        {
            live.resident.close();
        }
    }
}

/// What a turn needs to go through the conversation's resident process.
#[derive(Clone)]
pub(crate) struct Staying {
    pub app: AppHandle,
    pub project_id: String,
    pub conversation_id: String,
    pub transcript: PathBuf,
    pub head: PathBuf,
    pub driver: String,
    /// The agent its profile is based on, which says how the CLI starts.
    pub base: String,
    /// An orchestrator's conversation.
    pub listens: bool,
}

/// What a turn needs to stay, when it is to: every chat's, outside the
/// supervised mode.
pub(crate) fn staying(
    app: &AppHandle,
    project_id: &str,
    conversation_id: &str,
    sessions: &std::path::Path,
    profile: &devpit_rpc::Profile,
    mode: Option<&str>,
) -> Option<Staying> {
    if !stays(mode) {
        // The turn's own process takes the Stop: one the chat kept would be
        // interrupted instead. An orchestrator's, or a reachable one, stays.
        let reachable = crate::chat_remote::wanted(app, conversation_id);
        let kept = app.try_state::<Residents>().filter(|_| !reachable);
        if let Some(live) = kept.and_then(|all| all.taken(conversation_id, |live| !live.listens)) {
            live.resident.close();
        }
        return None;
    }
    Some(Staying {
        app: app.clone(),
        project_id: project_id.to_owned(),
        conversation_id: conversation_id.to_owned(),
        transcript: devpit_agentcli::store::conversation_path(sessions, conversation_id),
        head: devpit_agentcli::head::head_path(sessions, conversation_id),
        driver: profile.driver.clone(),
        base: profile.base.clone(),
        listens: listens(project_id),
    })
}

/// Whether a turn in `mode` goes through the conversation's process: every
/// chat's but the supervised mode's.
pub(crate) fn stays(mode: Option<&str>) -> bool {
    mode != Some(crate::asking::ASKING_MODE)
}

/// Whether the project is an orchestrator's.
fn listens(project_id: &str) -> bool {
    let Ok(store) = crate::projects::store() else {
        return false;
    };
    let Ok((_, root)) = crate::projects::locate(&store, project_id) else {
        return false;
    };
    devpit_core::Store::root()
        .ok()
        .and_then(|home| home.canonicalize().ok())
        .and_then(|home| devpit_core::home::orchestrator_of(&store, &home, &root))
        .is_some()
}

/// What a turn [`or_say`] could not finish answers with. Every one of these
/// is the agent's process — missing, gone or failing — and not a devpit bug,
/// so none of them reaches the error report.
pub(crate) fn turn_refused(err: AgentError) -> RpcError {
    let code = match err {
        AgentError::NotInstalled => ErrorCode::NotFound,
        // Sending again starts a process of its own.
        AgentError::Failed { .. } | AgentError::Unreadable(_) => ErrorCode::Busy,
    };
    RpcError::new(code, err.to_string())
}

/// A turn, through the conversation's resident process when there is to be
/// one, and as a process of its own otherwise.
///
/// `on_began` runs once the turn has the process, before its prompt is sent;
/// a turn stopped while it waited for a woken one never gets there, and ends
/// `cancelled`.
pub(crate) fn or_say(
    staying: Option<Staying>,
    driver: &dyn Driver,
    turn: &Say<'_>,
    on_began: impl FnOnce(),
    mut on_part: impl FnMut(Part),
    mut on_start: impl FnMut(u32) -> bool,
) -> Result<Said, AgentError> {
    let Some(staying) = staying else {
        on_began();
        return say(driver, turn, on_part, |pid| {
            on_start(pid);
        });
    };
    let residents = staying.app.state::<Residents>();
    let (tell, heard) = mpsc::channel();
    let ear = {
        let wanted = started_as(turn);
        // A model or mode is switched on the process it has, once the CLI
        // takes it; anything else takes a new one. Waited for unlocked.
        let switching = residents
            .0
            .lock()
            .map_err(|_| AgentError::NotInstalled)?
            .get_mut(&staying.conversation_id)
            .map(|live| {
                (live.started_as == wanted)
                    .then(|| live.switch(turn))
                    .flatten()
            });
        let kept = match switching {
            Some(Some((answers, count))) => switched(&answers, count),
            Some(None) => false,
            None => true,
        };
        let mut all = residents.0.lock().map_err(|_| AgentError::NotInstalled)?;
        if !kept {
            if let Some(old) = all.remove(&staying.conversation_id) {
                old.resident.close();
            }
        }
        let fresh = !all.contains_key(&staying.conversation_id);
        if fresh && staying.listens {
            let listening = all
                .iter()
                .map(|(key, live)| (key.as_str(), live.project_id.as_str()));
            for key in evicted(listening, &staying.project_id, |key| {
                crate::chat_remote::wanted(&staying.app, key)
            }) {
                if let Some(old) = all.remove(&key) {
                    old.resident.close();
                }
            }
        }
        if fresh {
            let live = start(&staying, turn, wanted)?;
            all.insert(staying.conversation_id.clone(), live);
        }
        let ear = all
            .get(&staying.conversation_id)
            .map(|live| live.ear.clone())
            .ok_or(AgentError::NotInstalled)?;
        drop(all);
        // A new process is a new connection: one asked to be reachable is
        // reached again.
        if fresh {
            crate::chat_remote::reconnect(&staying.app, &staying.conversation_id);
        }
        ear
    };
    // A woken turn still running finishes first: the person's words go in
    // after it, not into the middle of it.
    if !ear.claim(&tell) {
        return Ok(stopped());
    }
    on_began();
    let stamp = TURNS.fetch_add(1, Ordering::Relaxed);
    let (ear, session) = {
        let mut all = residents.0.lock().map_err(|_| AgentError::NotInstalled)?;
        // A stop asked while this turn started: said, then interrupted at once,
        // so the turn ends the way any turn of this process does.
        let mut stopped_first = false;
        let spoken = all
            .get_mut(&staying.conversation_id)
            .filter(|live| Arc::ptr_eq(&live.ear, &ear))
            .is_some_and(|live| {
                live.turn = stamp;
                let goes_on = on_start(live.resident.pid());
                stopped_first = !goes_on;
                let said = live.resident.say(turn.prompt);
                if said && !goes_on {
                    live.resident.interrupt();
                }
                said
            });
        if !spoken {
            // It exited since it last spoke, or was closed while this turn
            // waited: a new one, told to listen.
            if let Some(old) = all.remove(&staying.conversation_id) {
                old.resident.close();
            }
            let mut live = start(&staying, turn, started_as(turn))?;
            live.ear.claim(&tell);
            live.turn = stamp;
            // The first try took the stop asked, if there was one.
            let goes_on = on_start(live.resident.pid()) && !stopped_first;
            let said = live.resident.say(turn.prompt);
            if said && !goes_on {
                live.resident.interrupt();
            }
            let kept = (live.ear.clone(), live.session.clone());
            all.insert(staying.conversation_id.clone(), live);
            if !said {
                return Err(AgentError::Unreadable(
                    "the conversation's process would not start".to_owned(),
                ));
            }
            kept
        } else {
            let session = all
                .get(&staying.conversation_id)
                .map(|live| live.session.clone())
                .unwrap_or_default();
            (ear, session)
        }
    };
    // The window is told the session as a turn of its own would: its questions
    // are drawn there.
    let mut told = false;
    let tell_session = |part: Part| {
        if !told {
            if let Some(id) = session.lock().ok().and_then(|held| held.clone()) {
                turn.on_session.inspect(|tell| tell(&id));
                told = true;
            }
        }
        on_part(part)
    };
    // Still this conversation's process: one let go of on purpose is not replaced.
    let kept = || {
        residents.0.lock().is_ok_and(|all| {
            all.get(&staying.conversation_id)
                .is_some_and(|live| Arc::ptr_eq(&live.ear, &ear))
        })
    };
    let said = answered(&heard, driver, turn, tell_session, &mut on_start, kept)?;
    if !staying.listens {
        idle_out(&staying, stamp);
    }
    Ok(said)
}

/// The person's turn, as the process answers it. One that died before saying
/// anything is said again by a process of its own, so the message is not lost;
/// one that died mid-answer ends there, with what it said.
fn answered(
    heard: &mpsc::Receiver<Heard>,
    driver: &dyn Driver,
    turn: &Say<'_>,
    mut on_part: impl FnMut(Part),
    on_start: &mut impl FnMut(u32) -> bool,
    kept: impl Fn() -> bool,
) -> Result<Said, AgentError> {
    let mut spoke = false;
    loop {
        match heard.recv() {
            Ok(Heard::Part(part)) => {
                spoke = true;
                on_part(part);
            }
            Ok(Heard::Ended(said)) => return Ok(*said),
            // Control answers are taken before they get here.
            Ok(Heard::Control(_)) => {}
            Ok(Heard::Gone) | Err(_) if !spoke && kept() => {
                return say(driver, turn, on_part, |pid| {
                    on_start(pid);
                })
            }
            Ok(Heard::Gone) | Err(_) => {
                return Err(AgentError::Unreadable(
                    "the conversation's process ended".to_owned(),
                ))
            }
        }
    }
}

/// After a turn: the chat's process is let go once it has waited [`IDLE`]
/// with no turn since, and the next turn resumes the session in a new one.
fn idle_out(staying: &Staying, stamp: u64) {
    let (app, conversation) = (staying.app.clone(), staying.conversation_id.clone());
    let _ = std::thread::Builder::new()
        .name("chat-idle".to_owned())
        .spawn(move || {
            app.state::<Residents>()
                .idle_out(&conversation, stamp, IDLE, |key| {
                    crate::chat_remote::wanted(&app, key)
                });
        });
}

/// The residents a new one in `project` closes. One conversation of an
/// orchestrator listens at a time — two would both answer the same message —
/// but a chat asked to be reachable keeps its process: it is its claude.ai
/// link, and only turning Remote Control off lets it go.
pub(crate) fn evicted<'a>(
    listening: impl Iterator<Item = (&'a str, &'a str)>,
    project: &str,
    reachable: impl Fn(&str) -> bool,
) -> Vec<String> {
    listening
        .filter(|(key, of)| *of == project && !reachable(key))
        .map(|(key, _)| key.to_owned())
        .collect()
}

/// Whether this conversation has a process running now.
pub(crate) fn running(app: &AppHandle, conversation_id: &str) -> bool {
    app.try_state::<Residents>()
        .and_then(|residents| {
            residents
                .0
                .lock()
                .ok()
                .map(|all| all.contains_key(conversation_id))
        })
        .unwrap_or(false)
}

/// Sends a control request to this conversation's process. False when it has
/// none running.
pub(crate) fn control(
    app: &AppHandle,
    conversation_id: &str,
    id: &str,
    request: serde_json::Value,
) -> bool {
    app.try_state::<Residents>()
        .and_then(|residents| {
            residents.0.lock().ok().and_then(|all| {
                all.get(conversation_id)
                    .map(|live| live.resident.control(id, request))
            })
        })
        .unwrap_or(false)
}

/// Hands this conversation's process a message for the turn it is on. `None`
/// when it has no process, so the caller reaches the turn the other way.
pub(crate) fn steer(
    app: &AppHandle,
    conversation_id: &str,
    prompt: &str,
    uuid: &str,
    id: &str,
) -> Option<bool> {
    let residents = app.try_state::<Residents>()?;
    let all = residents.0.lock().ok()?;
    // One that died is not listening: the turn its chat runs instead takes it.
    all.get(conversation_id)
        .and_then(|live| live.resident.steer(prompt, uuid, id).then_some(true))
}

/// Asks this conversation's process to stop one background task. False when
/// it has none listening.
pub(crate) fn stop_task(app: &AppHandle, conversation_id: &str, task_id: &str) -> bool {
    app.try_state::<Residents>().is_some_and(|residents| {
        residents.0.lock().is_ok_and(|all| {
            all.get(conversation_id)
                .is_some_and(|live| live.resident.stop_task(task_id))
        })
    })
}

/// The steers this conversation's process never echoed back, by id.
pub(crate) fn unheard(app: &AppHandle, conversation_id: &str) -> Vec<String> {
    app.try_state::<Residents>()
        .and_then(|residents| {
            let all = residents.0.lock().ok()?;
            all.get(conversation_id).map(|live| live.resident.unheard())
        })
        .unwrap_or_default()
}

/// Ends this conversation's process at once: a stop it did not heed. Its
/// pid, or `None` when it has none.
pub(crate) fn force(app: &AppHandle, conversation_id: &str) -> Option<u32> {
    let gone = app.try_state::<Residents>().and_then(|residents| {
        residents
            .0
            .lock()
            .ok()
            .and_then(|mut all| all.remove(conversation_id))
    });
    let live = gone?;
    live.resident.close();
    crate::chat_running::forced(live.resident.pid());
    Some(live.resident.pid())
}

/// Lets every chat's process go as devpit quits: one that does not exit on
/// its own is ended now, since nothing is left to end it later.
pub(crate) fn quit_all(app: &AppHandle) {
    let all = app.try_state::<Residents>().and_then(|residents| {
        residents
            .0
            .lock()
            .ok()
            .map(|mut all| std::mem::take(&mut *all))
    });
    for live in all.into_iter().flat_map(HashMap::into_values) {
        live.resident.quit();
    }
}

/// Stops a turn waiting for a woken one to finish; false when none waits.
pub(crate) fn stop_waiting(app: &AppHandle, conversation_id: &str) -> bool {
    app.try_state::<Residents>()
        .and_then(|residents| {
            let all = residents.0.lock().ok()?;
            all.get(conversation_id).map(|live| live.ear.stop_waiting())
        })
        .unwrap_or(false)
}

/// Ends this conversation's process, if it has one: a chat no longer
/// reachable goes back to a process per turn.
pub(crate) fn close(app: &AppHandle, conversation_id: &str) {
    let gone = app.try_state::<Residents>().and_then(|residents| {
        residents
            .0
            .lock()
            .ok()
            .and_then(|mut all| all.remove(conversation_id))
    });
    if let Some(live) = gone {
        live.resident.close();
    }
}

/// Stops the person's turn and keeps the process: the one waiting for a woken
/// turn stops waiting, and the woken turn goes on. False when this
/// conversation has none, so the caller stops it the other way.
pub(crate) fn interrupt(app: &AppHandle, conversation_id: &str) -> bool {
    app.try_state::<Residents>()
        .and_then(|residents| {
            residents.0.lock().ok().and_then(|all| {
                all.get(conversation_id)
                    .map(|live| live.ear.stop_waiting() || live.resident.interrupt())
            })
        })
        .unwrap_or(false)
}

/// The flags only a new process takes. Model and mode are switched on the
/// running one.
fn started_as(turn: &Say<'_>) -> String {
    format!(
        "{}|{:?}|{}|{}|{:?}",
        turn.command,
        turn.effort,
        turn.cwd.display(),
        turn.add_dirs.join(":"),
        turn.name
    )
}

/// The control requests that bring a process in `model` and `mode` to the
/// turn's, one for each that changed. `None` when only a new process can:
/// back to no model or mode, which leaves it to the person's own settings.
fn switches(model: Option<&str>, mode: Option<&str>, turn: &Say<'_>) -> Option<Vec<Value>> {
    if (model.is_some() && turn.model.is_none()) || (mode.is_some() && turn.permission.is_none()) {
        return None;
    }
    let model = turn
        .model
        .filter(|to| model != Some(*to))
        .map(|to| json!({ "subtype": "set_model", "model": to }));
    let mode = turn
        .permission
        .filter(|to| mode != Some(*to))
        .map(|to| json!({ "subtype": "set_permission_mode", "mode": to }));
    Some(model.into_iter().chain(mode).collect())
}

impl Live {
    /// Asks it to switch to the turn's model and mode. The CLI's answers come
    /// back on the receiver, as many as the count. `None` when it takes a new
    /// process.
    fn switch(&mut self, turn: &Say<'_>) -> Option<(mpsc::Receiver<Value>, usize)> {
        let requests = switches(self.model.as_deref(), self.permission.as_deref(), turn)?;
        let (tell, answers) = mpsc::channel();
        *self.switching.lock().ok()? = Some(tell);
        let count = requests.len();
        for request in requests {
            let id = format!("{SWITCH}{}", ulid::Ulid::generate());
            if !self.resident.control(&id, request) {
                return None;
            }
        }
        self.model = turn.model.map(str::to_owned);
        self.permission = turn.permission.map(str::to_owned);
        Some((answers, count))
    }
}

/// Whether the CLI took every switch, in time. It refuses some: Full access
/// for a process not started in it.
fn switched(answers: &mpsc::Receiver<Value>, count: usize) -> bool {
    (0..count).all(|_| {
        answers
            .recv_timeout(SWITCH_WAITS)
            .is_ok_and(|said| said["subtype"] == "success")
    })
}

/// Hands the CLI's answer to a switch to the turn waiting for it. Any other
/// answer comes back.
fn to_switch(waiting: &Mutex<Option<mpsc::Sender<Value>>>, said: Value) -> Option<Value> {
    let ours = said["request_id"]
        .as_str()
        .is_some_and(|id| id.starts_with(SWITCH));
    let tell = waiting.lock().ok().and_then(|held| held.clone());
    match tell {
        Some(tell) if ours => {
            let _ = tell.send(said);
            None
        }
        _ => Some(said),
    }
}

/// A person's turn stopped before it had the process: nothing was said.
fn stopped() -> Said {
    Said {
        end: devpit_rpc::TurnEnd {
            turn_id: String::new(),
            cost_usd: None,
            duration_ms: None,
            stop_reason: Some("cancelled".to_owned()),
            is_error: false,
            context: None,
            ..Default::default()
        },
        session_id: None,
        init: None,
        anchor: None,
    }
}

fn start(staying: &Staying, turn: &Say<'_>, started_as: String) -> Result<Live, AgentError> {
    let driver = devpit_agentcli::driver::driver_for(&staying.driver, &staying.base)
        .ok_or(AgentError::NotInstalled)?;
    let ear = Arc::new(Ear::default());
    let heard_by = ear.clone();
    let app = staying.app.clone();
    let conversation = staying.conversation_id.clone();
    let transcript = staying.transcript.clone();
    let head = staying.head.clone();
    let answers = staying.app.clone();
    // Where the CLI logs this conversation: who woke a turn is written there.
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let config_dir = turn
        .env
        .iter()
        .find(|(key, _)| key == "CLAUDE_CONFIG_DIR")
        .map(|(_, value)| value.clone());
    let logs = devpit_agentcli::peers::logs_of(
        &devpit_agentcli::cli_config::config_dir_from(&home, config_dir.as_deref()),
        &turn
            .cwd
            .canonicalize()
            .unwrap_or_else(|_| turn.cwd.to_path_buf()),
    );
    let session = Arc::new(Mutex::new(None));
    let named = session.clone();
    let on_session = Arc::new(move |id: &str| {
        if let Ok(mut held) = named.lock() {
            *held = Some(id.to_owned());
        }
    });
    let switching = Arc::new(Mutex::new(None));
    let answering = switching.clone();
    let resident = Resident::start(driver, turn, on_session, move |heard| {
        if let Heard::Control(said) = heard {
            if let Some(said) = to_switch(&answering, said) {
                crate::chat_remote::answered(&answers, said);
            }
            return;
        }
        heard_by.hear(
            heard,
            || woke(&app, &conversation, &logs),
            &transcript,
            &head,
        );
    })?;
    Ok(Live {
        resident,
        started_as,
        model: turn.model.map(str::to_owned),
        permission: turn.permission.map(str::to_owned),
        switching,
        project_id: staying.project_id.clone(),
        listens: staying.listens,
        turn: 0,
        session,
        ear,
    })
}

/// A turn nobody here asked for has begun: opened in the conversation and
/// announced, so an open chat joins it through the relay.
fn woke(app: &AppHandle, conversation: &str, logs: &std::path::Path) -> Woken {
    let woken_by = Part::Text {
        text: crate::delegations::woken_label(logs),
        parent: None,
    };
    let relay = app.state::<crate::chat_relay::Relay>();
    let (frames, relaying) = relay.open(
        conversation,
        crate::chat_relay::Sink::whole(Channel::new(|_| Ok(()))),
    );
    let frames = crate::chat_pace::paced(frames);
    let turn_id = format!("turn_{}", ulid::Ulid::generate());
    let answer_id = format!("msg_{}", ulid::Ulid::generate());
    let began = crate::chat_listening::now();
    let opened = Message {
        id: answer_id.clone(),
        turn_id: Some(turn_id.clone()),
        role: Role::Assistant,
        parts: vec![woken_by.clone()],
        created_at: began,
        streaming: true,
        ended: None,
    };
    frames.send(Frame::Opened { message: opened }).ok();
    let _ = app.emit(WOKE, conversation);
    Woken {
        turn_id,
        answer_id,
        parts: vec![woken_by],
        began,
        frames,
        _relaying: relaying,
    }
}

#[cfg(test)]
#[path = "chat_resident_tests.rs"]
mod tests;
