//! `GET /events`: what the backend emits, carried to a browser over one
//! WebSocket, as `{"t": name, "p": payload}` per frame.
//!
//! No `emit` changes for it. The app listens to every event a page can
//! subscribe to ([`TOPICS`]) once, and each browser socket gets the ones it
//! asked for with `{"listen": [names]}`. A topic outside the list is not
//! carried: an event the window reads only on the desktop (a menu, the
//! island) stays there.

use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tauri::Listener;
use tungstenite::{Message, WebSocket};

use crate::remote_http::{reply, Request};

/// What a page in a browser can subscribe to: the events the web shell reads.
pub(crate) const TOPICS: &[&str] = &[
    "agent:health",
    "board:changed",
    "card:happening",
    "chat:remote",
    "chat:woke",
    "devflow:asked",
    "devflow:changed",
    "learning:asked",
    "lessons:proposed",
    "notice:rang",
    "orchestrator:forwarded",
    "orchestrator:proposed",
    "panes:changed",
    "pause:changed",
    "permission:asked",
    "permission:settled",
    "reminder:changed",
    "reminder:fired",
    "remote:changed",
    "remote:viewers",
    "run:changed",
    "run:progress",
    "schedules:changed",
    "session:tab-closed",
    "session:tab-opened",
    "sync:changed",
    "sync:written",
    "terminal:block",
    "terminal:happening",
    "update:before-restart",
    "update:status",
];

/// The longest frame a browser may send: a list of names.
const MOST_MESSAGE: usize = 16 * 1024;

/// One frame, as the browser reads it.
pub(crate) fn framed(topic: &str, payload: &str) -> String {
    let payload: serde_json::Value =
        serde_json::from_str(payload).unwrap_or(serde_json::Value::Null);
    serde_json::json!({ "t": topic, "p": payload }).to_string()
}

/// The names a `{"listen": [...]}` frame asks for, kept to [`TOPICS`].
pub(crate) fn asked(text: &str) -> Vec<&'static str> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    value["listen"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.as_str())
                .filter_map(|name| TOPICS.iter().copied().find(|topic| *topic == name))
                .collect()
        })
        .unwrap_or_default()
}

/// Every open socket, by what it hears.
struct Socket {
    topics: Mutex<Vec<&'static str>>,
    send: Sender<String>,
}

static SOCKETS: OnceLock<Mutex<Vec<std::sync::Arc<Socket>>>> = OnceLock::new();

fn sockets() -> &'static Mutex<Vec<std::sync::Arc<Socket>>> {
    SOCKETS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Carries one event to the sockets that asked for it, and forgets the ones gone.
pub(crate) fn carried(topic: &str, payload: &str) {
    let Ok(mut open) = sockets().lock() else {
        return;
    };
    let frame = framed(topic, payload);
    open.retain(|socket| {
        let wants = socket
            .topics
            .lock()
            .is_ok_and(|topics| topics.contains(&topic));
        !wants || socket.send.send(frame.clone()).is_ok()
    });
}

/// The app's listeners, once: each topic handed on to the sockets.
pub(crate) fn listen(app: &tauri::AppHandle) {
    for topic in TOPICS {
        app.listen_any(*topic, move |event| carried(topic, event.payload()));
    }
}

/// Answers a WebSocket upgrade; false (and a 400) for anything else.
pub(crate) fn handshake(stream: &mut TcpStream, request: &Request) -> bool {
    let Some(key) = request.header("sec-websocket-key").filter(|_| {
        request
            .header("upgrade")
            .is_some_and(|word| word.eq_ignore_ascii_case("websocket"))
    }) else {
        reply(stream, "400 Bad Request", "text/plain", b"a WebSocket only");
        return false;
    };
    let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
    let head = format!(
        "HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: Upgrade\r\nsec-websocket-accept: {accept}\r\n\r\n"
    );
    std::io::Write::write_all(stream, head.as_bytes()).is_ok()
}

pub(crate) fn upgrade(mut stream: TcpStream, request: &Request) {
    if !handshake(&mut stream, request) {
        return;
    }
    let config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MOST_MESSAGE))
        .max_frame_size(Some(MOST_MESSAGE));
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let ws = WebSocket::from_raw_socket(stream, tungstenite::protocol::Role::Server, Some(config));
    let (send, got) = mpsc::channel();
    let socket = std::sync::Arc::new(Socket {
        topics: Mutex::new(Vec::new()),
        send,
    });
    if let Ok(mut open) = sockets().lock() {
        open.push(socket.clone());
    }
    run(ws, &socket, &got);
}

/// Reads what the browser asks for and writes what it was carried, until it goes.
fn run(mut ws: WebSocket<TcpStream>, socket: &Socket, got: &Receiver<String>) {
    loop {
        match ws.read() {
            Ok(Message::Text(text)) => {
                if let Ok(mut topics) = socket.topics.lock() {
                    *topics = asked(&text);
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(err))
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
        while let Ok(frame) = got.try_recv() {
            if ws.send(Message::text(frame)).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "local_events_tests.rs"]
mod tests;
