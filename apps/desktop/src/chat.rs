//! The chat commands: send, cancel, and what was said before.
//!
//! The stream goes over a Channel, like the pty, because a turn is watched
//! rather than awaited.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use devpit_agentcli::driver::driver;
use devpit_agentcli::head::{head_path, read_head, remaining, settled, write_head};
use devpit_agentcli::store::{append, conversation_path};
use devpit_agentcli::talk::{Said, Say};
use devpit_rpc::{Ask, Conversation, ErrorCode, Frame, Message, RpcError, TurnEnd};
use tauri::ipc::Channel;

use crate::card_activity::Doing;
use tauri::{Manager, State};

/// The turns in flight, by conversation, so one can be stopped.
#[derive(Default, Clone)]
pub struct Talking {
    /// The pid serving each conversation, `None` while its turn is still
    /// starting. A key here means a turn is claimed — see `Talking::begin`.
    pub(crate) running: Arc<Mutex<HashMap<String, Option<u32>>>>,
    /// Stops asked of a turn whose process had not started yet, carried out
    /// as it starts. Written and read under `running`'s lock.
    pub(crate) stopping: Arc<Mutex<HashMap<String, crate::chat_running::Signal>>>,
}

pub(crate) fn home() -> PathBuf {
    devpit_core::Store::root().unwrap_or_default()
}

pub(crate) fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as f64)
        .unwrap_or_default()
}

fn id(prefix: &str) -> String {
    format!("{prefix}_{}", ulid::Ulid::generate())
}

/// [`crate::chat_window::chat_history`], whole, on the calling thread.
pub(crate) fn chat_history_now(
    project_id: String,
    conversation_id: String,
) -> Result<Conversation, RpcError> {
    crate::chat_window::chat_page_now(project_id, conversation_id, None)
}

/// `chat.send` — one turn, streamed as it happens.
#[tauri::command]
#[specta::specta]
pub async fn chat_send(
    app: tauri::AppHandle,
    state: State<'_, Talking>,
    steering: State<'_, crate::steering::Steering>,
    relay: State<'_, crate::chat_relay::Relay>,
    ask: Ask,
    on_frame: Channel<Frame>,
) -> Result<TurnEnd, RpcError> {
    let Ask {
        project_id,
        conversation_id,
        profile_id,
        model,
        prompt,
        cwd,
        budget_usd,
        permission,
        effort,
        from,
    } = ask;
    let home = home();
    let sessions = crate::projects::project_home(&project_id)?.sessions();
    let head_file = head_path(&sessions, &conversation_id);
    let head = read_head(&head_file);
    if let Err(fixed) = settled(head.as_ref(), &profile_id) {
        return Err(RpcError::new(
            ErrorCode::Conflict,
            format!("this conversation belongs to {fixed}"),
        ));
    }
    // Refused here rather than half way through: a cap that only stops a turn
    // mid-answer is not a ceiling.
    if remaining(head.as_ref()).is_some_and(|left| left <= 0.0) {
        return Err(RpcError::new(
            ErrorCode::Conflict,
            "this conversation has spent its budget".to_owned(),
        ));
    }
    let cwd = crate::chat_turn::turn_cwd(head.as_ref().and_then(|head| head.cwd.as_deref()), &cwd)?;

    let (profile, path) = crate::chat_turn::spawnable(&profile_id)?;
    let model = model.or_else(|| {
        crate::profile_models::default_model(&crate::projects::store().ok()?, &profile_id)
    });
    let Some(driver) = driver(&profile.driver) else {
        return Err(RpcError::new(
            ErrorCode::NotFound,
            format!("no driver called {}", profile.driver),
        ));
    };
    // Which account this turn spends. A terminal gets it as assignments in the
    // line it types and a step gets it from its runner; a chat was the one
    // path that spawned the binary with none of it.
    let env = devpit_agentcli::running::runner(&profile).env;
    let mcp = crate::agent_reach::chat_mcp(&profile.driver);
    let reach = crate::orchestrator::reaches(&project_id);
    let address = crate::orchestrator::address_of(&project_id);

    let file = conversation_path(&sessions, &conversation_id);
    let turn_id = id("turn");

    // Claimed before anything is written or sent, and released by the guard
    // however this returns. Nothing new once an update is about to go in: a
    // turn started here would die with the process at the commit.
    if let Some(updating) = app.try_state::<crate::update::Updating>() {
        if let Some(why) = crate::update::starting_refused(&updating.state()) {
            return Err(RpcError::new(ErrorCode::Busy, why.to_owned()));
        }
    }
    let guard = state.begin(&conversation_id, &steering)?;
    let idle = crate::idle_compact::Idle::of(&profile_id, head.as_ref(), &file, &head_file);

    // Written before the turn runs: an interrupted first turn still leaves the
    // conversation tied to the account that opened it.
    let opening = crate::chat_turn::opening(
        head.as_ref(),
        &profile_id,
        model.clone(),
        budget_usd,
        permission,
        effort,
        now(),
    );
    let _ = write_head(&head_file, &opening);
    let resuming = opening.session_id.clone();
    let forking = opening.rewind.fork_at.clone();
    let mode = opening.permission.clone();
    let thinking = opening.effort.clone();
    let left = remaining(Some(&opening));
    let staying = crate::chat_resident::staying(
        &app,
        &project_id,
        &conversation_id,
        &sessions,
        &profile.driver,
        mode.as_deref(),
    );
    // Every frame goes through the relay, so a chat reopened mid-turn can join.
    // A turn on a resident process may wait for a woken one: it is heard after it.
    // The relay keeps frames whole for the Remote, and cuts each once for the windows.
    let window = crate::chat_relay::Sink::window(on_frame);
    let (on_frame, relaying) = match staying {
        Some(_) => relay.waiting(&conversation_id, window),
        None => relay.open(&conversation_id, window),
    };
    let on_frame = crate::chat_pace::paced(on_frame);

    let (asked, opened) = crate::chat_turn::messages(&turn_id, &prompt, now() * 1000.0);
    let answer_id = opened.id.clone();
    let _ = on_frame.send(Frame::Opened {
        message: asked.clone(),
    });
    let _ = on_frame.send(Frame::Opened {
        message: opened.clone(),
    });

    let started = state.on_start(&conversation_id, staying.is_some());
    let talking = state.inner().clone();
    let key = conversation_id.clone();
    let parts = Arc::new(Mutex::new(Vec::new()));
    let collected = parts.clone();
    let sink = on_frame.clone();
    let answer = answer_id.clone();
    let hold = crate::asking::holding(mode.as_deref(), &app, on_frame.clone());
    let begin = relaying.beginning();
    let began = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let beginning = began.clone();
    let transcript = file.clone();

    let control = steering.hold(&conversation_id);
    control.asks_with(crate::asking_plan::plans(&app, driver.asks()));
    let steered = control.clone();
    crate::card_chat::turn_heard(&app, &conversation_id, Doing::Working);
    let noticed = (project_id.clone(), profile_id.clone(), from);
    let advised = profile_id.clone();
    let checkout_at = PathBuf::from(&cwd);
    let said = tauri::async_runtime::spawn_blocking(move || {
        let prompt = crate::session_notice::with_notice(&noticed, &prompt, now() * 1000.0);
        let hooks = crate::steps::hook_settings_for(Some(&advised));
        let checkout = std::path::Path::new(&cwd);
        let mut before = crate::turn_changes::Before::default();
        let turn = Say {
            command: &path,
            env: &env,
            prompt: &prompt,
            cwd: checkout,
            model: model.as_deref(),
            // What is left of the cap, not the cap: a resumed conversation
            // may not spend its whole budget again.
            budget_usd: left,
            session_id: resuming.as_deref(),
            fork_at: forking.as_deref(),
            permission: mode.as_deref(),
            disallowed: &[],
            settings: hooks.as_deref(),
            mcp_config: mcp.as_deref(),
            effort: thinking.as_deref(),
            control: Some(&control),
            on_session: Some(&hold),
            add_dirs: &reach,
            name: address.as_deref(),
        };
        let compacted =
            crate::idle_compact::on_return(&idle, staying.clone(), &*driver, &turn, &sink);
        let said = crate::chat_resident::or_say(
            staying,
            driver.as_ref(),
            &crate::idle_compact::resuming(&compacted, &turn),
            // Only once the turn has the process: a woken turn it waited for
            // is written first, and its changes are not this turn's.
            || {
                before = crate::turn_changes::Before::start(checkout.to_owned());
                let _ = append(&transcript, &asked);
                if let Some(done) = &compacted {
                    let _ = append(&transcript, &done.note);
                }
                begin();
                beginning.store(true, std::sync::atomic::Ordering::SeqCst);
            },
            |part| {
                collected
                    .lock()
                    .map(|mut held| crate::chat_relay::kept(&mut held, part.clone()))
                    .ok();
                let _ = sink.send(Frame::Part {
                    message_id: answer.clone(),
                    part,
                });
            },
            started,
        );
        talking.exited(&key);
        (said, before)
    })
    .await;
    let (said, mut before) = match said {
        Ok((said, before)) => (Ok(said), before),
        Err(err) => (Err(err), crate::turn_changes::Before::default()),
    };
    // Before the answer is looked at: a turn that failed is not still working.
    crate::card_chat::turn_heard(&app, &conversation_id, Doing::Done);
    let said = said.map_err(|err| RpcError::internal(err.to_string()));
    let said = said.and_then(|said| said.map_err(crate::chat_resident::turn_refused));
    if said.is_err() {
        // Written with what it changed, so that is read before its end, as it was.
        let before = std::mem::take(&mut before);
        crate::turn_changes::reported(
            checkout_at.clone(),
            before,
            parts.clone(),
            on_frame.clone(),
            answer_id.clone(),
        )
        .await;
    }
    let kept = began
        .load(std::sync::atomic::Ordering::SeqCst)
        .then_some(&parts);
    let mut said = crate::chat_turn::or_failed(said, &turn_id, &file, kept, &opened, &on_frame)?;
    // A steer the agent never echoed back did not reach it: the window offers to send it again.
    said.end.unheard = steered.unheard();
    said.end
        .unheard
        .extend(crate::chat_resident::unheard(&app, &conversation_id));

    // Stopped while it waited for a woken turn: nothing was said, or written.
    // The guard does both when it goes, including on the `?` above.
    if !began.load(std::sync::atomic::Ordering::SeqCst) {
        drop(guard);
        let end = TurnEnd {
            turn_id,
            ..said.end
        };
        let _ = on_frame.send(Frame::Ended { end: end.clone() });
        return Ok(end);
    }

    crate::chat_turn::ran_on(&mut said, &opening);
    let ended = devpit_rpc::Ended::of(&said.end, crate::chat_listening::now());
    let Said {
        end,
        session_id,
        init,
        anchor,
    } = said;
    crate::slash::remember(&home, &profile_id, init.as_ref());
    crate::chat_turn::settle(
        &head_file,
        Some(opening),
        &turn_id,
        &end,
        session_id,
        anchor,
    );
    let at = checkout_at.to_string_lossy().into_owned();

    // The end goes out before git reads what the turn changed. The answer is
    // written once it has, under the guard: a turn let in sooner would write
    // its prompt above this answer.
    let end = TurnEnd { turn_id, ..end };
    crate::turn_changes::end_then_changes(
        checkout_at,
        before,
        end.clone(),
        parts.clone(),
        on_frame.clone(),
        answer_id,
    )
    .await;
    let answered = Message {
        parts: parts.lock().map(|held| held.clone()).unwrap_or_default(),
        streaming: false,
        ended: Some(ended),
        ..opened
    };
    let _ = append(&file, &answered);
    // Once the answer is written: the timer counts idle from the transcript.
    crate::idle_compact::arm(&app, &project_id, &conversation_id, &profile_id, &at);
    drop(guard);
    Ok(end)
}

/// `chat.frames` — the shapes `chat.send` uses, on both sides.
///
/// It exists so the generated contract carries `Ask` and `Frame`: `chat.send`
/// streams over a Channel, which specta cannot describe, so it is left out of
/// the contract and its types would go with it.
#[tauri::command]
#[specta::specta]
pub fn chat_frames(_ask: Option<Ask>) -> Result<Vec<Frame>, RpcError> {
    Ok(Vec::new())
}
