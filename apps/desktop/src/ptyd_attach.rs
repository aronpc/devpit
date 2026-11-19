//! Attaching a window to a pane devpit-ptyd holds.
//!
//! The counterpart of the `tmux attach` in [`crate::panes::session_attach`]:
//! the same claim, the same sink, but the bytes come from the daemon's socket
//! and nothing is spawned. Output is acknowledged to the daemon when the page
//! says it parsed it (the panes socket), or as it is handed on (the channel),
//! which is what keeps the daemon reading the pty — and a flood from filling
//! memory on either side.
//!
//! What the pane prints is cut into blocks by [`crate::ptyd_tap`], which
//! hears it with or without a window; here only the clipboard, which is the
//! attached side's to answer, as with tmux.

use std::os::unix::net::UnixStream;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use devpit_ptyd::frame::{self, Frame, Kind};
use devpit_rpc::{ErrorCode, RpcError};
use portable_pty::PtySize;
use tauri::{Emitter, State};

use crate::claims::Live;
use crate::live::{Ends, Out, Sink};
use crate::sessions::SessionState;

/// What the drain hands on: bytes for the window, and whether they are new
/// (heard, acknowledged) or the replay of what was printed before.
enum Heard {
    Fresh(Vec<u8>),
    Replay(Vec<u8>),
    Older(Vec<u8>),
    Folder(String),
    /// The daemon refused the attach itself, and why: a pane it does not hold.
    Refused(String),
    Ended,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn attach(
    app: &tauri::AppHandle,
    label: String,
    state: &State<'_, SessionState>,
    pane_id: String,
    client_id: String,
    rows: u16,
    cols: u16,
    sink: Arc<dyn Sink>,
) -> Result<(), RpcError> {
    let home = devpit_core::Store::root()?;
    let pane = crate::ptyd_link::pane_number(&pane_id);
    let stream = crate::ptyd_link::connect(&home).map_err(|err| {
        RpcError::new(
            ErrorCode::Unsupported,
            format!("devpit-ptyd, which holds this terminal, did not answer: {err}"),
        )
    })?;
    let reading = stream
        .try_clone()
        .map_err(|err| RpcError::internal(err.to_string()))?;

    let claimed = state.claims.take_for(&pane_id, &client_id)?;
    if let Some(previous) = claimed.replaced {
        let _ = previous.stop();
    }
    let ring = Arc::new(Mutex::new(devpit_pty::RingBuffer::new(
        devpit_pty::RING_BYTES,
    )));
    let live = Arc::new(Live {
        client_id: client_id.clone(),
        webview: label,
        ends: Ends::Ptyd {
            pane,
            stream: Mutex::new(stream),
            size: Mutex::new(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            }),
        },
        ring: Arc::clone(&ring),
        flow: Default::default(),
        held: Default::default(),
    });
    if !state
        .claims
        .install(&pane_id, claimed.generation, Arc::clone(&live))?
    {
        live.stop()?;
        return Err(RpcError::new(
            ErrorCode::Conflict,
            "a newer client already owns that pane",
        ));
    }

    let begun = (|| {
        let Ends::Ptyd { stream, .. } = &live.ends else {
            return Err(RpcError::internal("not a ptyd pane"));
        };
        let mut stream = stream
            .lock()
            .map_err(|_| RpcError::internal("ptyd stream lock"))?;
        for one in [
            Frame::new(pane, Kind::Attach, Vec::new()),
            Frame::new(pane, Kind::Resize, size_bytes(rows, cols)),
        ] {
            frame::write(&mut *stream, &one).map_err(|err| RpcError::internal(err.to_string()))?;
        }
        crate::ptyd_tap::tap().sized(pane, rows, cols);
        Ok(())
    })();
    if let Err(err) = begun {
        state.claims.forget_exactly(&pane_id, &live);
        let _ = state.claims.release(&pane_id, &client_id);
        return Err(err);
    }

    let (heard_tx, mut heard) = tokio::sync::mpsc::channel::<Heard>(64);
    std::thread::spawn(move || drain(reading, pane, heard_tx));

    let app = app.clone();
    let acks = sink.acks();
    let mut scanner = devpit_pty::Scanner::new();
    let mut refused = None;
    while let Some(one) = heard.recv().await {
        let (out, bytes) = match one {
            Heard::Fresh(bytes) => {
                scanner.scan(&bytes, |told| {
                    if !crate::tap::heard_by_tap(&told) {
                        let _ =
                            app.emit("terminal:happening", crate::happening::said(&pane_id, told));
                    }
                });
                if !acks {
                    let _ = acknowledge(&live, pane, bytes.len());
                }
                live.held.fetch_add(bytes.len() as u64, Ordering::AcqRel);
                (Out::Output, bytes)
            }
            Heard::Replay(bytes) => {
                // Modes, then the tail: the last replay is the history held.
                live.held.store(bytes.len() as u64, Ordering::Release);
                (Out::Replay, bytes)
            }
            Heard::Older(bytes) => {
                let page = bytes.len().saturating_sub(8) as u64;
                live.held.fetch_add(page, Ordering::AcqRel);
                if !sink.send(Out::Older, bytes) {
                    break;
                }
                continue;
            }
            Heard::Folder(cwd) => {
                crate::tap::report(&app, &pane_id, devpit_pty::Told::Cwd(cwd));
                continue;
            }
            Heard::Refused(why) => {
                refused = Some(why);
                break;
            }
            Heard::Ended => break,
        };
        if let Ok(mut ring) = ring.lock() {
            ring.write(&bytes);
        }
        if !sink.send(out, bytes) {
            break;
        }
    }

    live.stop()?;
    state.claims.forget_exactly(&pane_id, &live);
    let _ = state.claims.release(&pane_id, &client_id);
    crate::ptyd_tap::tap().let_go(&pane_id, state.claims.live(&pane_id).is_ok());
    ended(refused)
}

/// How an attach ends: a refused one says why, so the terminal shows that
/// rather than staying blank.
fn ended(refused: Option<String>) -> Result<(), RpcError> {
    match refused {
        Some(why) => Err(RpcError::new(
            ErrorCode::NotFound,
            format!("devpit-ptyd no longer holds this terminal: {why}"),
        )),
        None => Ok(()),
    }
}

fn size_bytes(rows: u16, cols: u16) -> Vec<u8> {
    let mut bytes = rows.to_le_bytes().to_vec();
    bytes.extend_from_slice(&cols.to_le_bytes());
    bytes
}

fn acknowledge(live: &Live, pane: u32, bytes: usize) -> std::io::Result<()> {
    let Ends::Ptyd { stream, .. } = &live.ends else {
        return Ok(());
    };
    let mut stream = stream
        .lock()
        .map_err(|_| std::io::Error::other("ptyd stream lock"))?;
    frame::write(
        &mut *stream,
        &Frame::new(pane, Kind::Ack, (bytes as u32).to_le_bytes().to_vec()),
    )
}

/// The socket's frames for `pane`, until it ends or the connection does. A
/// refusal before anything of the pane's is the attach's own; a later one,
/// of an input or a resize, ends it as it always did.
fn drain(mut stream: UnixStream, pane: u32, heard: tokio::sync::mpsc::Sender<Heard>) {
    let mut held = false;
    loop {
        let one = match frame::read(&mut stream) {
            Ok(Some(one)) if one.pane == pane => one,
            Ok(Some(_)) => continue,
            Ok(None) | Err(_) => break,
        };
        let passed = match one.kind() {
            Some(Kind::Output) => Heard::Fresh(one.bytes),
            Some(Kind::Replay) => Heard::Replay(one.bytes),
            Some(Kind::Older) => Heard::Older(one.bytes),
            Some(Kind::Cwd) => Heard::Folder(String::from_utf8_lossy(&one.bytes).into_owned()),
            Some(Kind::Refused) if !held => {
                Heard::Refused(String::from_utf8_lossy(&one.bytes).into_owned())
            }
            Some(Kind::Exited | Kind::Refused) => Heard::Ended,
            _ => {
                held = true;
                continue;
            }
        };
        held = true;
        let ended = matches!(passed, Heard::Ended | Heard::Refused(_));
        if heard.blocking_send(passed).is_err() || ended {
            return;
        }
    }
    let _ = heard.blocking_send(Heard::Ended);
}

#[cfg(test)]
#[path = "ptyd_attach_tests.rs"]
mod tests;
