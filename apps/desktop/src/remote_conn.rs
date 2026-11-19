//! One viewer's connection: the device it is, then requests answered one by
//! one, each checked against what the device may do.
//!
//! What a device may do is fixed for the connection. Changing it on the
//! machine drops the device's connections, and the viewer connects again
//! with what it has now — so a terminal watched read-only is attached again
//! read-only or not, by tmux, rather than trusted to the page.
//!
//! The connection is a `Wire`: a WebSocket on the tailnet, where the device
//! says hello with its token, or a Noise session through the relay, where
//! the handshake already proved which device it is.

use std::net::TcpStream;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use devpit_rpc::{RemoteIn, RemoteOut, RemoteProject, RemoteTerminal};
use tauri::Manager as _;
use tungstenite::{Message, WebSocket};

use crate::remote_devices::{Device, Devices};
use crate::remote_panes::Watching;
use crate::remote_talk::Talk;

/// How long a viewer has to say who it is.
pub(crate) const HELLO_WITHIN: Duration = Duration::from_secs(10);
/// The most a message from a viewer may be.
pub(crate) const MOST_MESSAGE: usize = 256 * 1024;
/// A connection that says nothing for this long is closed.
const IDLE: Duration = Duration::from_secs(90);
/// The longest one read waits: what a pane prints waits at most this long
/// to go out, so an echo is not held behind a quiet viewer.
pub(crate) const LISTEN: Duration = Duration::from_millis(8);

/// What one read of a connection brought.
pub(crate) enum Heard {
    Text(String),
    /// A raw frame, `[u32 handle][u8 kind][bytes]`.
    Bytes(Vec<u8>),
    /// A ping, or part of a message: the viewer is there.
    Alive,
    /// Nothing within the read's timeout.
    Nothing,
}

/// A viewer's connection, however it came.
pub(crate) trait Wire {
    /// One read; an error when the connection is gone.
    fn hear(&mut self) -> Result<Heard, ()>;
    fn say(&mut self, text: &str);
    /// A raw frame, to a viewer that gave a handle.
    fn say_bytes(&mut self, bytes: &[u8]);
    /// Ends the connection, and with it whatever keys it held.
    fn end(&mut self);
}

/// The tailnet's: one JSON message per WebSocket frame, on the machine's own
/// TLS or the tailnet's encryption.
pub(crate) struct Tailnet(pub WebSocket<TcpStream>);

impl Wire for Tailnet {
    fn hear(&mut self) -> Result<Heard, ()> {
        match self.0.read() {
            Ok(Message::Text(text)) => Ok(Heard::Text(text.to_string())),
            Ok(Message::Binary(bytes)) => Ok(Heard::Bytes(bytes.to_vec())),
            Ok(Message::Ping(_) | Message::Pong(_)) => Ok(Heard::Alive),
            Ok(Message::Close(_)) => Err(()),
            Ok(_) => Ok(Heard::Nothing),
            Err(tungstenite::Error::Io(err))
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                Ok(Heard::Nothing)
            }
            Err(_) => Err(()),
        }
    }

    fn say(&mut self, text: &str) {
        let _ = self.0.send(Message::text(text));
    }

    fn say_bytes(&mut self, bytes: &[u8]) {
        let _ = self.0.send(Message::binary(bytes.to_vec()));
    }

    fn end(&mut self) {
        let _ = self.0.close(None);
        let _ = self.0.flush();
    }
}

/// A tailnet viewer: its hello names the device, then it is served.
pub(crate) fn run(app: tauri::AppHandle, ws: WebSocket<TcpStream>, root: std::path::PathBuf) {
    let _ = ws.get_ref().set_read_timeout(Some(LISTEN));
    // An echo is a few bytes: sent now, not when the kernel has more.
    let _ = ws.get_ref().set_nodelay(true);
    let mut wire = Tailnet(ws);
    match hello(&mut wire, &root) {
        Some(device) => serve(app, wire, root, device),
        None => wire.end(),
    }
}

/// `device`'s requests, answered until it leaves, goes silent or is dropped
/// here.
pub(crate) fn serve(
    app: tauri::AppHandle,
    mut wire: impl Wire,
    root: std::path::PathBuf,
    device: Device,
) {
    let (id, out, heard) = crate::remote_hub::join(&app, &device.id);
    crate::remote::note(&root, &device, "connected");
    let mut watching = Watching::new(id);
    let (raw, raws) = std::sync::mpsc::sync_channel(crate::remote_panes::MOST_RAW_QUEUED);
    watching.raw = Some(raw);
    let mut talk = crate::remote_talk::Talk::default();
    // A turn begun anywhere else in the conversation this device reads reaches it.
    let watch = app
        .state::<crate::chat_relay::Relay>()
        .watch(talk.watching(out.clone()));
    send(&mut wire, &welcome(&device));
    converse(
        &mut wire,
        id,
        &mut watching,
        &heard,
        &raws,
        |watching, asked| {
            typed_raw_or(&device, watching, asked, |watching, asked| {
                answer(&app, &root, &device, watching, &mut talk, &out, asked)
            })
        },
    );
    drop(watch);
    watching.close_all();
    crate::remote_hub::leave(&app, id);
    crate::remote::note(&root, &device, "disconnected");
    wire.end();
}

/// What the host says first: who the device is, what it may do, and the
/// language the host speaks now. A change of language at the host reaches the
/// viewer with the next welcome, which is the next connection.
fn welcome(device: &Device) -> RemoteOut {
    RemoteOut::Welcome {
        device: device.name.clone(),
        host: machine_name(),
        typing: device.typing,
        answering: device.answering,
        binary: true,
        stops: true,
        locale: devpit_core::locale::Locale::current().tag().to_owned(),
    }
}

/// What one message from the viewer was.
pub(crate) enum Asked<'a> {
    Json(RemoteIn),
    Raw(&'a [u8]),
}

/// The viewer's messages answered and what the host has for it sent, until
/// the hub lets the connection go — a device forgotten is cut off here, within
/// one [`LISTEN`] — or the viewer leaves or goes silent.
pub(crate) fn converse(
    wire: &mut impl Wire,
    id: u64,
    watching: &mut Watching,
    heard: &std::sync::mpsc::Receiver<RemoteOut>,
    raws: &std::sync::mpsc::Receiver<crate::remote_panes::Raw>,
    mut answer: impl FnMut(&mut Watching, Asked<'_>) -> Option<RemoteOut>,
) {
    let mut last = Instant::now();
    loop {
        if !crate::remote_hub::holds(id) {
            break;
        }
        match wire.hear() {
            Ok(Heard::Text(text)) => {
                last = Instant::now();
                if text.len() > MOST_MESSAGE {
                    break;
                }
                let said = match serde_json::from_str::<RemoteIn>(&text) {
                    Ok(asked) => answer(watching, Asked::Json(asked)),
                    Err(_) => Some(RemoteOut::Failed {
                        why: "that is not a message devpit reads".to_owned(),
                    }),
                };
                if let Some(said) = said {
                    send(wire, &said);
                }
            }
            Ok(Heard::Bytes(frame)) => {
                last = Instant::now();
                if frame.len() > MOST_MESSAGE {
                    break;
                }
                if let Some(said) = answer(watching, Asked::Raw(&frame)) {
                    send(wire, &said);
                }
            }
            Ok(Heard::Alive) => last = Instant::now(),
            Ok(Heard::Nothing) => {}
            Err(()) => break,
        }
        if last.elapsed() > IDLE {
            break;
        }
        // At most one LISTEN, and only while the hub holds the connection: a
        // pane printing faster than the wire writes cannot keep it here. The
        // hub's messages go first, or a flood would hold a chat back for good.
        let draining = Instant::now();
        let more = || draining.elapsed() < LISTEN && crate::remote_hub::holds(id);
        while more() {
            let Ok(said) = heard.try_recv() else { break };
            send(wire, &said);
        }
        while more() {
            let Ok(one) = raws.try_recv() else { break };
            wire.say_bytes(&crate::pane_socket::encode(
                one.handle,
                crate::pane_socket::kind::OUTPUT,
                &one.bytes,
            ));
        }
    }
}

/// A raw frame answered as keys, anything else by `json`.
fn typed_raw_or(
    device: &Device,
    watching: &mut Watching,
    asked: Asked<'_>,
    json: impl FnOnce(&mut Watching, RemoteIn) -> Option<RemoteOut>,
) -> Option<RemoteOut> {
    match asked {
        Asked::Json(asked) => json(watching, asked),
        Asked::Raw(frame) => typed_raw(device, watching, frame),
    }
}

/// A raw frame from the viewer: keys for the pane its handle names, the only
/// kind it may send. What it may not do is said back, as a JSON request's is.
pub(crate) fn typed_raw(
    device: &Device,
    watching: &mut Watching,
    frame: &[u8],
) -> Option<RemoteOut> {
    let (handle, kind, bytes) = crate::pane_socket::decode(frame)?;
    if kind != crate::pane_socket::kind::INPUT {
        return None;
    }
    if !device.typing {
        return failed("this device may only watch — allow it to type on the machine");
    }
    let pane = watching.pane_of(handle)?.to_owned();
    watching.input(&pane, bytes).err().and_then(failed)
}

/// What this machine is called, as a viewer shows it.
pub(crate) fn machine_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(hostname)
        .unwrap_or_else(|| "this machine".to_owned())
}

fn hostname() -> Option<String> {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
}

pub(crate) fn send(wire: &mut impl Wire, said: &RemoteOut) {
    if let Ok(text) = serde_json::to_string(said) {
        wire.say(&text);
    }
}

pub(crate) fn refuse(wire: &mut impl Wire, why: &str) {
    send(
        wire,
        &RemoteOut::Refused {
            why: why.to_owned(),
        },
    );
}

/// The first message a viewer sends, within `HELLO_WITHIN`.
pub(crate) fn first(wire: &mut impl Wire) -> Option<RemoteIn> {
    let began = Instant::now();
    while began.elapsed() < HELLO_WITHIN {
        match wire.hear() {
            Ok(Heard::Text(text)) if text.len() <= MOST_MESSAGE => {
                return serde_json::from_str::<RemoteIn>(&text).ok();
            }
            Ok(Heard::Text(_) | Heard::Bytes(_)) | Err(()) => return None,
            Ok(Heard::Alive | Heard::Nothing) => {}
        }
    }
    None
}

/// The device a tailnet connection's hello names, or nothing — said why.
fn hello(wire: &mut impl Wire, root: &std::path::Path) -> Option<Device> {
    let Some(RemoteIn::Hello { token }) = first(wire) else {
        refuse(wire, "say hello first");
        return None;
    };
    let mut devices = Devices::read(root);
    let Some(device) = devices.by_token(&token).cloned() else {
        refuse(
            wire,
            "this device is not paired with this machine — pair it again",
        );
        return None;
    };
    devices.seen(&device.id, crate::remote::now());
    let _ = devices.write(root);
    Some(device)
}

fn drafts_now() -> RemoteOut {
    RemoteOut::Drafts {
        drafts: crate::reply_drafts::all()
            .into_iter()
            .map(|(profile, name, text)| devpit_rpc::RemoteDraft {
                profile,
                name,
                text,
            })
            .collect(),
    }
}

/// The stops waiting on the person's click, and what the last click came to.
pub(crate) fn stops_now(said: Option<String>) -> RemoteOut {
    RemoteOut::Stops {
        stops: crate::stop_asked::all(),
        said,
    }
}

/// The stops a viewer sent, by the orchestrator account each waits for.
fn by_profile(stops: &[devpit_rpc::RemoteStop]) -> Vec<(String, Vec<devpit_rpc::StopAsked>)> {
    let mut grouped: Vec<(String, Vec<devpit_rpc::StopAsked>)> = Vec::new();
    for one in stops {
        match grouped
            .iter_mut()
            .find(|(profile, _)| *profile == one.profile)
        {
            Some((_, list)) => list.push(one.stop.clone()),
            None => grouped.push((one.profile.clone(), vec![one.stop.clone()])),
        }
    }
    grouped
}

fn failed(why: impl Into<String>) -> Option<RemoteOut> {
    Some(RemoteOut::Failed { why: why.into() })
}

/// What a request gets, checked against what `device` may do.
fn answer(
    app: &tauri::AppHandle,
    root: &std::path::Path,
    device: &Device,
    watching: &mut Watching,
    talk: &mut Talk,
    out: &std::sync::mpsc::Sender<RemoteOut>,
    asked: RemoteIn,
) -> Option<RemoteOut> {
    match asked {
        // Who the device is was settled before its requests were read.
        RemoteIn::Hello { .. } | RemoteIn::Pair { .. } => None,
        RemoteIn::Ping => Some(RemoteOut::Pong),
        RemoteIn::Projects => Some(match projects() {
            Ok(projects) => RemoteOut::Projects { projects },
            Err(why) => RemoteOut::Failed { why },
        }),
        RemoteIn::PaneOpen {
            project,
            pane,
            cols,
            rows,
            handle,
        } => match watching.open(
            &project,
            &pane,
            device.typing,
            (cols, rows),
            out.clone(),
            handle,
        ) {
            Ok(()) => {
                crate::remote::note(root, device, &format!("watching {pane}"));
                None
            }
            Err(why) => failed(why),
        },
        RemoteIn::PaneInput { pane, b64 } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            let Ok(bytes) = BASE64.decode(b64) else {
                return failed("those keys are not base64");
            };
            watching.input(&pane, &bytes).err().and_then(failed)
        }
        RemoteIn::PanePaste { pane, text } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            let text: String = text
                .chars()
                .filter(|c| *c == '\n' || !c.is_control())
                .take(4000)
                .collect();
            if text.trim().is_empty() {
                return None;
            }
            let Some(target) = watched_target(&pane) else {
                return failed("that terminal is not open");
            };
            let sent = crate::pane_io::paste(&target, &text, true).map_err(|err| err.message);
            crate::remote::note(root, device, &format!("sent a line to {pane}"));
            sent.err().and_then(failed)
        }
        RemoteIn::PaneClose { pane } => {
            watching.close(&pane);
            None
        }
        RemoteIn::Board { project } => Some(match crate::board::board_get_now(project.clone()) {
            Ok(board) => RemoteOut::Board {
                project,
                board: Box::new(board),
            },
            Err(err) => RemoteOut::Failed { why: err.message },
        }),
        RemoteIn::CardMove {
            project,
            card,
            column,
        } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            let board = match crate::board::board_get_now(project) {
                Ok(board) => board,
                Err(err) => return failed(err.message),
            };
            match crate::agent_api::moved(Some(app), &board, &card, &column) {
                Ok(_) => {
                    crate::remote::note(root, device, &format!("moved {card}"));
                    None
                }
                Err(why) => failed(why),
            }
        }
        RemoteIn::Waiting => Some(RemoteOut::Waiting {
            questions: crate::remote_hub::questions(),
        }),
        RemoteIn::Answer { id, allow } => {
            if !device.answering {
                return failed("this device may not answer — allow it on the machine");
            }
            crate::remote::note(
                root,
                device,
                if allow {
                    "allowed a tool"
                } else {
                    "denied a tool"
                },
            );
            settle(app, &id, allow).err().and_then(failed)
        }
        RemoteIn::Chats { project } => Some(match crate::threads::chat_list_now(project.clone()) {
            Ok(conversations) => RemoteOut::Chats {
                project,
                conversations: Box::new(conversations),
            },
            Err(err) => RemoteOut::Failed { why: err.message },
        }),
        RemoteIn::Chat {
            project,
            conversation,
        } => {
            if !crate::remote_talk::plain(&conversation) {
                return failed("that is not a conversation");
            }
            let read = crate::chat::chat_history_now(project.clone(), conversation.clone());
            // A turn running in it reaches this device from where it is, and so
            // does one begun later while it is read.
            talk.view(&project, &conversation);
            talk.follow(app, out, &project, &conversation);
            Some(match read {
                Ok(read) => RemoteOut::Chat {
                    project,
                    conversation: Box::new(crate::remote_talk::for_phone(read)),
                },
                Err(err) => RemoteOut::Failed { why: err.message },
            })
        }
        RemoteIn::Talks => Some(match crate::remote_talk::talks() {
            Ok(talks) => RemoteOut::Talks { talks },
            Err(why) => RemoteOut::Failed { why },
        }),
        RemoteIn::ChatSay {
            project,
            conversation,
            text,
        } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            crate::remote::note(
                root,
                device,
                &format!("sent a message to a chat in {project}"),
            );
            crate::remote_talk::say(app, out, talk, &project, &conversation, &text)
                .err()
                .and_then(failed)
        }
        RemoteIn::Session { profile, name } => Some(crate::remote_talk::session(&profile, &name)),
        RemoteIn::SessionSay {
            profile,
            name,
            text,
        } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            if let Err(err) =
                crate::live_sessions::reply_now(&profile, &name, &text, Some("the Remote"))
            {
                return failed(err.message);
            }
            crate::remote::note(root, device, &format!("sent a message to {name}"));
            None
        }
        RemoteIn::UploadPart { upload, b64 } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            talk.uploads.add(&upload, &b64).err().and_then(failed)
        }
        RemoteIn::Photo {
            upload,
            project,
            mime,
        } => {
            let bytes = talk.uploads.take(&upload);
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            let Some(bytes) = bytes else {
                return failed("that photo did not arrive — send it again");
            };
            let kept = crate::projects::store()
                .map_err(|err| err.message)
                .and_then(|store| crate::remote_talk::photo(&store, root, &project, &mime, &bytes));
            Some(match kept {
                Ok(attachment) => {
                    crate::remote::note(root, device, &format!("sent a photo to {project}"));
                    RemoteOut::Attached { upload, attachment }
                }
                Err(why) => RemoteOut::Failed { why },
            })
        }
        RemoteIn::Voice { upload, mime } => {
            let bytes = talk.uploads.take(&upload);
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            let Some(bytes) = bytes else {
                return failed("that recording did not arrive — record it again");
            };
            match crate::remote_talk::voice_kind(&mime, bytes.len()) {
                Ok(ext) => {
                    crate::remote::note(root, device, "sent a recording to be heard");
                    crate::remote_talk::hear(out, root, upload, ext, bytes);
                    None
                }
                Err(why) => failed(why),
            }
        }
        RemoteIn::Drafts => Some(drafts_now()),
        RemoteIn::DraftSend { profile, name } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            let Some(text) = crate::reply_drafts::drafted(&profile, &name) else {
                return failed("that draft is no longer waiting");
            };
            if let Err(err) =
                crate::live_sessions::reply_now(&profile, &name, &text, Some("the Remote"))
            {
                return failed(err.message);
            }
            crate::remote::note(root, device, &format!("sent a draft to {name}"));
            Some(drafts_now())
        }
        RemoteIn::Stops => Some(stops_now(None)),
        RemoteIn::StopsConfirm { stops } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            let mut said = Vec::new();
            for (profile, shown) in by_profile(&stops) {
                let done = crate::stop_asked::confirm_now(app, &profile, &shown);
                if !done.stopped.is_empty() {
                    let names = done.stopped.join(", ");
                    crate::remote::note(root, device, &format!("stopped {names}"));
                }
                said.extend(
                    done.changed
                        .iter()
                        .map(|one| format!("Not stopped, it would lose more now: {one}")),
                );
                said.extend(done.failed.iter().map(|one| format!("Not stopped: {one}")));
            }
            Some(stops_now((!said.is_empty()).then(|| said.join(" · "))))
        }
        RemoteIn::StopsDrop { stops } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            for (profile, shown) in by_profile(&stops) {
                let names: Vec<String> = shown.into_iter().map(|one| one.name).collect();
                crate::stop_asked::drop_now(app, &profile, &names);
            }
            Some(stops_now(None))
        }
        RemoteIn::CardCreate { project, title } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            crate::remote::note(root, device, "created a card");
            crate::remote_start::card(app, &project, &title)
                .err()
                .and_then(failed)
        }
        RemoteIn::SessionStart {
            project,
            card,
            prompt,
            anyway,
        } => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            Some(
                match crate::remote_start::session(app, &project, card.as_deref(), &prompt, anyway)
                {
                    Ok(name) => {
                        crate::remote::note(root, device, &format!("started {name}"));
                        RemoteOut::Started { name }
                    }
                    Err(why) => RemoteOut::Failed { why },
                },
            )
        }
        RemoteIn::SecretSet {
            project,
            name,
            value,
        } => Some(
            match crate::projects::store()
                .map_err(|err| err.message)
                .and_then(|store| {
                    crate::remote_secret::keep(
                        &store,
                        root,
                        &device.id,
                        &project,
                        &name,
                        &value,
                        crate::remote::now(),
                    )
                }) {
                Ok(name) => {
                    crate::vault_rotation::told(app, &project);
                    RemoteOut::SecretKept { project, name }
                }
                Err(why) => RemoteOut::Failed { why },
            },
        ),
        RemoteIn::AgentHealth => Some(RemoteOut::AgentHealth {
            health: crate::agent_door::health_now(app),
        }),
        RemoteIn::AgentRestart => {
            if !device.typing {
                return failed("this device may only watch — allow it to type on the machine");
            }
            crate::remote::note(root, device, "restarted devpit's MCP");
            Some(match crate::agent_door::restart_by_hand(app) {
                Ok(health) => RemoteOut::AgentHealth { health },
                Err(why) => RemoteOut::Failed { why },
            })
        }
    }
}

/// A question answered as the person: a terminal's held one, or a chat's.
fn settle(app: &tauri::AppHandle, id: &str, allow: bool) -> Result<(), String> {
    if id.starts_with("ask_") {
        let verdict = if allow {
            devpit_rpc::IslandVerdict::Allow
        } else {
            devpit_rpc::IslandVerdict::Deny
        };
        return crate::pane_asking::decide(id, verdict);
    }
    let answer = if allow {
        crate::asking::Answer::Allow
    } else {
        crate::asking::Answer::Deny
    };
    let asking = app
        .try_state::<crate::asking::Asking>()
        .ok_or("nothing is asking")?;
    asking
        .answer(id, answer)
        .then_some(())
        .ok_or_else(|| "that question is no longer waiting".to_owned())
}

/// Where an open pane is typed into: its tmux window, by the session it runs
/// in, or the daemon's pane, by the project the tap placed it in.
fn watched_target(pane: &str) -> Option<String> {
    let projects = crate::projects::project_list_now().ok()?.projects;
    let in_tmux = crate::sessions::tmux_server().ok().and_then(|server| {
        projects.iter().map(|one| one.id.as_str()).find(|id| {
            server
                .running(&devpit_tmux::Server::session_name(id))
                .is_ok_and(|running| running.iter().any(|one| one.leaf_id == pane))
        })
    });
    if let Some(project) = in_tmux {
        let session = devpit_tmux::Server::session_name(project);
        return Some(devpit_tmux::Server::target(&session, pane));
    }
    projects
        .iter()
        .any(|one| {
            ptyd_running(&one.id)
                .iter()
                .any(|running| running.pane_id == pane)
        })
        .then(|| crate::ptyd_link::target(pane))
}

/// The daemon's terminals in a project: none where there is no daemon.
#[cfg(unix)]
fn ptyd_running(project_id: &str) -> Vec<devpit_rpc::PaneRunning> {
    crate::ptyd_watch::running_in(project_id)
}

#[cfg(not(unix))]
fn ptyd_running(_project_id: &str) -> Vec<devpit_rpc::PaneRunning> {
    Vec::new()
}

/// A project's open terminals for a viewer, tmux's and then the daemon's, in
/// the one shape an old viewer already reads.
pub(crate) fn terminals(
    tmux: Vec<devpit_tmux::Running>,
    ptyd: Vec<devpit_rpc::PaneRunning>,
) -> Vec<RemoteTerminal> {
    let tmux = tmux.into_iter().map(|running| RemoteTerminal {
        pane: running.leaf_id,
        command: running.command,
    });
    let ptyd = ptyd.into_iter().map(|running| RemoteTerminal {
        pane: running.pane_id,
        command: running.command,
    });
    tmux.chain(ptyd).collect()
}

/// Every project and its open terminals.
fn projects() -> Result<Vec<RemoteProject>, String> {
    let listed = crate::projects::project_list_now()
        .map_err(|err| err.message)?
        .projects;
    let server = crate::sessions::tmux_server().ok();
    Ok(listed
        .into_iter()
        .filter(|one| one.orchestrator.is_none())
        .map(|one| {
            let in_tmux = server
                .as_ref()
                .and_then(|server| {
                    server
                        .running(&devpit_tmux::Server::session_name(&one.id))
                        .ok()
                })
                .unwrap_or_default();
            let terminals = terminals(in_tmux, ptyd_running(&one.id));
            RemoteProject {
                id: one.id,
                name: one.name,
                group: one.group,
                terminals,
            }
        })
        .collect())
}

#[cfg(test)]
#[path = "remote_conn_tests.rs"]
mod tests;
