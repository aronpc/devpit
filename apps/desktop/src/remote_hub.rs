//! The viewers connected now, and what every one of them is told: a board
//! that changed, the questions agents are waiting on.
//!
//! Each connection has a queue; the hub puts messages on it and its own
//! loop writes them out. Forgetting a device drops its queues, which ends
//! its connections.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use devpit_rpc::{RemoteOut, RemoteQuestion};
use tauri::{Emitter as _, Listener as _};

struct Conn {
    id: u64,
    device: String,
    out: Sender<RemoteOut>,
}

static CONNS: Mutex<Vec<Conn>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(1);
static QUESTIONS: Mutex<Vec<RemoteQuestion>> = Mutex::new(Vec::new());

/// A connection of `device`, and the queue its loop reads.
pub(crate) fn join(
    app: &tauri::AppHandle,
    device: &str,
) -> (u64, Sender<RemoteOut>, Receiver<RemoteOut>) {
    let joined = enter(device);
    told(app);
    joined
}

/// [`join`], without telling the window.
pub(crate) fn enter(device: &str) -> (u64, Sender<RemoteOut>, Receiver<RemoteOut>) {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let (out, heard) = channel();
    if let Ok(mut conns) = CONNS.lock() {
        conns.push(Conn {
            id,
            device: device.to_owned(),
            out: out.clone(),
        });
    }
    (id, out, heard)
}

pub(crate) fn leave(app: &tauri::AppHandle, id: u64) {
    if let Ok(mut conns) = CONNS.lock() {
        conns.retain(|one| one.id != id);
    }
    told(app);
}

/// Ends every connection of a device: forgotten, or no longer allowed what
/// it was connected with.
pub(crate) fn drop_device(app: &tauri::AppHandle, device: &str) {
    drop_conns(device);
    told(app);
}

/// [`drop_device`], without telling the window.
pub(crate) fn drop_conns(device: &str) {
    if let Ok(mut conns) = CONNS.lock() {
        conns.retain(|one| one.device != device);
    }
}

pub(crate) fn drop_all(app: &tauri::AppHandle) {
    if let Ok(mut conns) = CONNS.lock() {
        conns.clear();
    }
    told(app);
}

/// Whether a connection is still the hub's: dropped, its loop ends.
pub(crate) fn holds(id: u64) -> bool {
    CONNS
        .lock()
        .is_ok_and(|conns| conns.iter().any(|one| one.id == id))
}

pub(crate) fn connected() -> HashSet<String> {
    CONNS
        .lock()
        .map(|conns| conns.iter().map(|one| one.device.clone()).collect())
        .unwrap_or_default()
}

fn broadcast(said: &RemoteOut) {
    if let Ok(conns) = CONNS.lock() {
        for one in conns.iter() {
            let _ = one.out.send(said.clone());
        }
    }
}

/// The desk says how many devices are watching, always.
fn told(app: &tauri::AppHandle) {
    let _ = app.emit("remote:viewers", connected().len());
}

pub(crate) fn questions() -> Vec<RemoteQuestion> {
    QUESTIONS
        .lock()
        .map(|held| held.clone())
        .unwrap_or_default()
}

pub(crate) fn question(id: &str) -> Option<RemoteQuestion> {
    QUESTIONS
        .lock()
        .ok()?
        .iter()
        .find(|question| question.id == id)
        .cloned()
}

pub(crate) fn chat_question(question: crate::asking::Question) -> RemoteQuestion {
    RemoteQuestion {
        id: question.id,
        from: "chat".to_owned(),
        project: None,
        tool: question.tool,
        input: question.input,
        touch: question.touch,
    }
}

fn terminal_question(question: devpit_rpc::IslandQuestion) -> RemoteQuestion {
    RemoteQuestion {
        id: question.id,
        from: "terminal".to_owned(),
        project: question.project,
        tool: question.tool,
        input: question.input,
        touch: None,
    }
}

fn asked(question: RemoteQuestion) {
    if let Ok(mut held) = QUESTIONS.lock() {
        held.retain(|one| one.id != question.id);
        held.push(question);
    }
    broadcast(&RemoteOut::Waiting {
        questions: questions(),
    });
}

fn settled(id: &str) {
    if let Ok(mut held) = QUESTIONS.lock() {
        held.retain(|one| one.id != id);
    }
    broadcast(&RemoteOut::Waiting {
        questions: questions(),
    });
}

/// Hears what the window hears, for the viewers.
pub(crate) fn listen(app: &tauri::AppHandle) {
    app.listen_any("board:changed", |event| {
        if let Ok(project) = serde_json::from_str::<String>(event.payload()) {
            broadcast(&RemoteOut::BoardChanged { project });
        }
    });
    app.listen_any("island:asked", |event| {
        if let Ok(question) = serde_json::from_str::<devpit_rpc::IslandQuestion>(event.payload()) {
            asked(terminal_question(question));
        }
    });
    app.listen_any("permission:asked", |event| {
        if let Ok(question) = serde_json::from_str::<crate::asking::Question>(event.payload()) {
            asked(chat_question(question));
        }
    });
    app.listen_any(crate::stop_asked::STOP_ASKED, |_| {
        broadcast(&crate::remote_conn::stops_now(None));
    });
    for name in ["island:settled", "permission:settled"] {
        app.listen_any(name, |event| {
            if let Ok(id) = serde_json::from_str::<String>(event.payload()) {
                settled(&id);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{drop_conns, enter, holds};

    #[test]
    fn dropping_a_device_lets_go_of_its_connections_and_no_other() {
        let (laptop, _, _) = enter("dev_hub_laptop");
        let (phone, _, _) = enter("dev_hub_phone");
        assert!(holds(laptop) && holds(phone));
        drop_conns("dev_hub_laptop");
        assert!(!holds(laptop), "its loop ends on its next turn");
        assert!(holds(phone));
        drop_conns("dev_hub_phone");
    }
}
