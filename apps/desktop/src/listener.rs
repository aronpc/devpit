//! The other end of the hooks: a loopback listener the agent posts to.
//!
//! Small on purpose. It answers one route, on 127.0.0.1, on a port the OS
//! picks, and writes that address to a file the hook reads on every invocation
//! — so a session that outlived a restart finds the new port instead of posting
//! into a dead one.
//!
//! Loopback and nothing else. This receives a payload and turns it into
//! something the window draws; binding it anywhere reachable would be a way in
//! to a process that runs terminals.

use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use devpit_agentcli::{read_hook, Event, Happening};
use devpit_core::Store;
use devpit_rpc::SessionKind;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::asking::{decision, Asking};
use crate::asking_picks::{reply_for, Picks};
use crate::card_activity::{hear, next_seq, pane_word, state_of_event, Activities, Key, Place};
use crate::card_route::{card_of_leaf, card_of_session, notice_for, Ring};
use crate::happening::{agent_said, session_said, subagent_said};
use crate::post::{read_request, Posted};
use crate::question::question_in;

/// Starts listening, and writes the address where the hook will look for it.
///
/// Failure is not fatal: hooks are how the board hears about work as it
/// happens, and without them it still polls. A window that refuses to open
/// because a port was busy would be worse than one that is a little less live.
pub fn start(app: AppHandle, root: &Path) {
    // Where one run of the app begins in the trace: sequence numbers start
    // again with every process, and a reader pairing them has to know that.
    trace("listening");
    if let Err(why) = open(app, root) {
        devpit_core::reports::background("hook listener", &why);
    }
}

/// Which listener is the current one: a restart opens another, and the old
/// accept loop ends at its next connection.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// The current listener's address, for a restart to wake the old loop with.
static LISTENING: Mutex<Option<SocketAddr>> = Mutex::new(None);

/// Opens a listener and publishes it, retiring the one before if there was.
///
/// The secret stays for the life of the process: a hook already carrying it
/// must still get in after a restart.
pub(crate) fn open(app: AppHandle, root: &Path) -> Result<SocketAddr, String> {
    open_with(root, move |stream| {
        // Stamped here, in the order posts arrived: each is served on its
        // own thread, and threads finish in any order.
        let seq = next_seq();
        let app = app.clone();
        // One thread per post, and they are short: a hook that has to wait
        // for the one before it is a hook holding up the agent that sent it.
        std::thread::spawn(move || serve(app, stream, seq));
    })
}

/// [`open`], with what each connection is handed to.
pub(crate) fn open_with(
    root: &Path,
    take: impl Fn(TcpStream) + Send + 'static,
) -> Result<SocketAddr, String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|err| format!("none, the board will poll instead: {err}"))?;
    let address = listener
        .local_addr()
        .map_err(|err| format!("no address: {err}"))?;
    let endpoint = devpit_agentcli::endpoint_file(root);
    if let Some(parent) = endpoint.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // The secret before the address, and for the same reason the address is
    // written at all: a hook that found a port to post to but no secret to
    // carry would post and, from the next story on, be refused.
    let secret = match SECRET.get() {
        Some(secret) => secret.clone(),
        None => fresh_secret().ok_or("none: the system gave out no randomness")?,
    };
    devpit_core::home::write_private(
        &devpit_agentcli::auth_file(root),
        format!("{}: {secret}\n", devpit_agentcli::HOOK_HEADER).as_bytes(),
    )
    .map_err(|err| format!("could not write the hook secret: {err}"))?;
    let _ = SECRET.set(secret);
    // Private: the port is what a post has to know, so a file anyone can read
    // is an invitation to post as the agent.
    devpit_core::home::write_private(&endpoint, format!("http://{address}/hook").as_bytes())
        .map_err(|err| format!("could not publish the hook endpoint: {err}"))?;

    let mine = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let before = LISTENING
        .lock()
        .ok()
        .and_then(|mut listening| listening.replace(address));
    std::thread::spawn(move || accept(listener, mine, take));
    if let Some(before) = before {
        let _ = TcpStream::connect_timeout(&before, Duration::from_millis(200));
    }
    Ok(address)
}

fn accept(listener: TcpListener, mine: u64, take: impl Fn(TcpStream)) {
    for stream in listener.incoming() {
        if GENERATION.load(Ordering::SeqCst) != mine {
            return;
        }
        match stream {
            Ok(stream) => take(stream),
            // Out of descriptors, most often: spinning on it starves the rest.
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// This run's secret, kept for the door to compare against.
static SECRET: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The hook secret this run of the app is using, if it has one.
///
/// Read by `kept_out`, which takes it back out of a command's log before the
/// log is stored: a hook that fails prints the request it sent.
pub(crate) fn secret_now() -> Option<String> {
    SECRET.get().cloned()
}

/// Whether a post showed this run's secret.
///
/// Compared to the end even once it is known to differ: a comparison that
/// stops at the first wrong byte says, in how long it took, how much of the
/// secret the caller already has.
///
/// What this defends against: another **user** on the machine, who can see the
/// port but not read a file at 0600. Not against code running as you — that
/// code can read the secret as easily as devpit can.
pub(crate) fn authorized(carried: Option<&str>, secret: &str) -> bool {
    let Some(carried) = carried else {
        return false;
    };
    if carried.len() != secret.len() {
        return false;
    }
    carried
        .bytes()
        .zip(secret.bytes())
        .fold(0u8, |differs, (mine, theirs)| differs | (mine ^ theirs))
        == 0
}

/// The secret this run's hooks carry, thirty-two bytes from the system's own
/// randomness.
///
/// Not `fresh_session_id` and not a ULID: those are a timestamp and a counter,
/// and a secret anything can recompute from the moment the app started is not
/// one. `None` when the system refuses, which is a reason not to listen at all
/// rather than to listen without a door.
fn fresh_secret() -> Option<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn serve(app: AppHandle, mut stream: TcpStream, seq: u64) {
    trace(&format!("post seq={seq}"));
    let Some(posted) = read_request(&mut stream, SECRET.get().map(String::as_str)) else {
        trace(&format!("post seq={seq} bad request"));
        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\n\r\n");
        return;
    };
    // Refused before anything is read out of the payload. Counted, never
    // logged with what it carried: a refusal written to a log is the guess
    // written to a log.
    if let Some(secret) = SECRET.get() {
        if !authorized(posted.secret.as_deref(), secret) {
            trace(&format!("post seq={seq} refused"));
            let _ = stream.write_all(b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\n\r\n");
            return;
        }
    }
    let body = &posted.body;

    // A question from an agent, not a report about one: answered with what it
    // asked for, and nothing about it reaches the hook path below.
    if posted.agent {
        trace(&format!("post seq={seq} agent"));
        reply(
            &mut stream,
            &crate::agent_api::answer_in_time(&app, body.clone()),
        );
        return;
    }

    // A terminal session about to ask: held for the island while nobody is at
    // the window, and answered with nothing otherwise, so the terminal asks.
    if let Some(pane) = posted.pane.as_deref() {
        if crate::pane_asking::is_permission_request(body) {
            trace(&format!("post seq={seq} permission"));
            // The dialog is about to show: waiting, before any notification.
            crate::pane_said::heard(pane, crate::card_activity::Doing::Waiting);
            let _ = app.emit("terminal:happening", agent_said(pane, "waiting"));
            let said = crate::pane_asking::hold(&app, body, pane);
            reply(&mut stream, &said);
            return;
        }
    }

    // A tool this session wants to be asked about holds the connection until
    // a person answers. Everything else is answered at once and empty, which
    // leaves the CLI's own permission mode in charge — a board step running
    // where nobody is watching must never wait on a window.
    let held = question_in(body).filter(|question| {
        let supervised = crate::question::changes_something(&question.tool)
            && app
                .try_state::<Asking>()
                .map(|asking| asking.asks(&question.session_id))
                .unwrap_or(false);
        // A question to the person waits for them in any mode a chat runs in.
        supervised
            || app
                .try_state::<Picks>()
                .is_some_and(|picks| picks.holds(&question.session_id, &question.tool))
    });
    // Already allowed for the rest of this conversation: answered as the
    // person answered it then, without asking again.
    if let Some(question) = &held {
        let kept = !crate::asking_picks::to_the_person(&question.tool)
            && app
                .try_state::<crate::asking_kept::Kept>()
                .is_some_and(|kept| {
                    kept.covers(&question.session_id, &question.tool, &question.input)
                });
        if kept {
            reply(&mut stream, &decision(crate::asking::Answer::Allow));
            let settled = hear_post(&app, &posted, seq);
            trace(&format!("post seq={seq} {settled}"));
            return;
        }
    }

    if let Some(question) = held {
        let settled = hear_post(&app, &posted, seq);
        trace(&format!("post seq={seq} {settled}"));
        let asking = app.state::<Asking>();
        let hear = asking.opened(&question.id);
        let _ = app.emit("permission:asked", &question);
        crate::island_notify::asked(&app, &question.tool, &question.input);
        let answer = asking.wait(&question.id, hear);
        let said = reply_for(&app.state::<Picks>(), &question.id, &question.input, answer);
        reply(&mut stream, &said);
        // Answered here or anywhere else, or timed out: every window that
        // drew the question puts it away.
        let _ = app.emit("permission:settled", &question.id);
        return;
    }

    // A card's session past its share of the context window leaves the
    // handoff before anything else runs.
    let over = posted.pane.as_deref().and_then(|pane| {
        let store = crate::projects::store().ok()?;
        crate::card_relay::over_share(&store, body, pane)
    });
    if let Some(said) = over {
        reply(&mut stream, &said);
        let settled = hear_post(&app, &posted, seq);
        trace(&format!("post seq={seq} handoff {settled}"));
        return;
    }

    // Answered before the payload is looked at. The agent is waiting on this
    // reply with a short budget, and nothing it says changes what we reply
    // with.
    reply(&mut stream, "");
    let settled = hear_post(&app, &posted, seq);
    trace(&format!("post seq={seq} {settled}"));
}

/// What a post becomes once it has been answered: the listener's core, apart
/// from the window.
///
/// Answers how it settled, in a word the trace prints: a post let in and never
/// accounted for is the one a test cannot see missing.
fn hear_post(sink: &impl HookSink, posted: &Posted, seq: u64) -> &'static str {
    match read_hook(&posted.body) {
        // An event this app does not read: settled, by being left alone.
        None => "ignored",
        Some(happening) => {
            if let Event::UseFailed {
                tool,
                error: Some(error),
            } = &happening.event
            {
                if let Some(missing) = crate::credentials::missing_in(tool, error) {
                    sink.lacks(&happening, &missing);
                }
            }
            sink.to_window("agent:happening", describe(&happening));
            crate::island_feed::heard(sink, posted.pane.as_deref(), &happening, now_ms());
            heard(sink, posted.pane.as_deref(), &happening, seq)
        }
    }
}

/// Relays an agent's own report to the pane it is running in.
///
/// Only when the hook said which pane, which it does whenever the agent was
/// started inside one of our terminals. A headless turn belongs to a card and
/// has no pane to tell; it takes the `agent:happening` path above and this
/// leaves it alone.
///
/// This is the difference between knowing an agent is *open* and knowing what
/// it is *doing*. The first is asked of the process table on a timer; the
/// second only the agent can say, and it says it here.
fn heard(
    sink: &impl HookSink,
    pane: Option<&str>,
    happening: &Happening,
    seq: u64,
) -> &'static str {
    let Some(pane) = pane else {
        return heard_without_pane(sink, happening, seq);
    };
    let doing = state_of_event(&happening.event);
    if let Some(doing) = doing {
        crate::pane_said::heard(pane, doing);
        crate::session_bus::heard(pane, doing);
    }
    let state = pane_word(doing);
    // One Store for the event. Resolved before `remember`, which forgets the
    // pane's agent on SessionEnded — the end has to reach the card too.
    let store = sink.store();
    let route = store.as_ref().and_then(|store| card_of_leaf(store, pane));
    if let Some(store) = &store {
        crate::restoring::remember(store, pane, happening);
    }
    if let Some(state) = state {
        sink.to_window("terminal:happening", agent_said(pane, state));
    }
    if let Some(session) = session_said(pane, happening) {
        sink.to_window("terminal:happening", session);
    }
    if let Some(subagent) = subagent_said(pane, &happening.event) {
        sink.to_window("terminal:happening", subagent);
    }
    let mut settled = match (&route, doing) {
        (_, None) => "no state",
        (None, _) if store.is_none() => "no store",
        (None, _) => "no card",
        (Some(_), Some(_)) => "unchanged",
    };
    if let (Some(route), Event::Prompted) = (&route, &happening.event) {
        sink.card_prompted(&route.card_id);
    }
    if let (Some(route), Some(path)) = (&route, written(&happening.event)) {
        sink.card_touched(&route.card_id, path);
    }
    if let (Some(route), Some(doing)) = (&route, doing) {
        let key = Key {
            card_id: route.card_id.clone(),
            kind: SessionKind::Pane,
            reference: pane.to_owned(),
        };
        let place = Place {
            tab_id: Some(route.tab_id.clone()),
            leaf_id: Some(pane.to_owned()),
        };
        let told = sink
            .activities()
            .lock()
            .ok()
            .and_then(|mut activities| hear(&mut activities, key, seq, doing, place));
        if let Some(happening) = told {
            trace(&format!("emit card:happening seq={seq}"));
            sink.to_window("card:happening", happening);
            settled = "emitted";
        }
    }

    // Only `waiting` reaches the bell. An agent that is working is an agent
    // you can watch; one that has stopped and is waiting for a person is the
    // reason somebody left the window and the reason to call them back. The
    // other two would be a bell that rings through every turn.
    if let (Some(ring), Some(store)) = (notice_for(route.as_ref(), state), &store) {
        sink.ring(store, ring, pane);
    }
    settled
}

/// The file an event says its tool is about to edit or write.
fn written(event: &Event) -> Option<&str> {
    match event {
        Event::Using {
            touch:
                Some(devpit_rpc::Touch::Edit { path, .. } | devpit_rpc::Touch::Write { path, .. }),
            ..
        } => Some(path),
        _ => None,
    }
}

/// A hook from a session with no pane — a run's turn, or a session a step
/// started in the background — reaching the card that holds its id.
///
/// Heard like a pane is and combined when read: a background session's word
/// stands only while the CLI still lists it (see `background_state`).
fn heard_without_pane(sink: &impl HookSink, happening: &Happening, seq: u64) -> &'static str {
    let Some(doing) = state_of_event(&happening.event) else {
        return "no state";
    };
    let Some(store) = sink.store() else {
        return "no store";
    };
    let Some((card_id, kind)) = card_of_session(&store, &happening.session_id) else {
        return "no card";
    };
    // A run is a step's: its card already sits where the step put it.
    if matches!(happening.event, Event::Prompted) && kind != SessionKind::Run {
        sink.card_prompted(&card_id);
    }
    if let Some(path) = written(&happening.event) {
        sink.card_touched(&card_id, path);
    }
    let key = Key {
        card_id,
        kind,
        reference: happening.session_id.clone(),
    };
    let told = sink
        .activities()
        .lock()
        .ok()
        .and_then(|mut activities| hear(&mut activities, key, seq, doing, Place::default()));
    match told {
        Some(happening) => {
            trace(&format!("emit card:happening seq={seq}"));
            sink.to_window("card:happening", happening);
            "emitted"
        }
        None => "unchanged",
    }
}

/// A timestamped line on stderr, only with `DEVPIT_TRACE_HOOKS` set: how long a
/// hook's post takes to reach the window is measured from these, by a person
/// running the app, and nobody else pays for the lines.
fn trace(what: &str) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| std::env::var_os("DEVPIT_TRACE_HOOKS").is_some()) {
        return;
    }
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or_default();
    eprintln!("devpit-trace {millis} {what}");
}

/// Where what a hook says goes: the window, and the bell.
///
/// A trait rather than the `AppHandle`, so the listener's core runs without a
/// window.
pub(crate) trait HookSink {
    fn to_window<P: serde::Serialize + Clone>(&self, channel: &str, payload: P);
    /// A pane worth coming back to, with the Store the event already opened.
    fn ring(&self, store: &Store, ring: Ring<'_>, pane: &str);
    /// The Store for one event, or nothing when it cannot be opened.
    fn store(&self) -> Option<Store>;
    /// What has been heard about cards so far.
    fn activities(&self) -> &Mutex<Activities>;
    /// A session changed on the island. Nothing, where there is no island.
    fn to_island(&self, _change: devpit_rpc::IslandChange) {}
    /// A session's state changed from `was`: the person may want to know.
    fn tell_person(&self, _session: &devpit_rpc::IslandSession, _was: Option<devpit_rpc::Doing>) {}
    /// A session of this card took a prompt: the card may move to work in
    /// progress. Nothing, where there is no board to move it on.
    fn card_prompted(&self, _card_id: &str) {}
    /// A session of a card edited or wrote a file, perhaps outside its checkout.
    fn card_touched(&self, _card_id: &str, _path: &str) {}
    /// A tool failed for want of a sign-in: the person is told what fixes it.
    fn lacks(&self, _happening: &Happening, _missing: &crate::credentials::Missing) {}
}

impl HookSink for AppHandle {
    fn to_window<P: serde::Serialize + Clone>(&self, channel: &str, payload: P) {
        let _ = self.emit(channel, payload);
    }

    fn ring(&self, store: &Store, ring: Ring<'_>, pane: &str) {
        crate::notices::ring_in(
            store,
            self,
            ring.project_id,
            crate::notices::kind::AGENT,
            "An agent is waiting on you",
            ring.card_id,
            pane,
        );
    }

    fn store(&self) -> Option<Store> {
        Store::open_default().ok()
    }

    fn lacks(&self, happening: &Happening, missing: &crate::credentials::Missing) {
        if !crate::credentials::first_time(&happening.session_id, &missing.command) {
            return;
        }
        let project = crate::projects::project_list_unread_now()
            .ok()
            .and_then(|listed| {
                crate::agent_api::project_at(&listed.projects, Path::new(&happening.cwd))
                    .map(|one| one.id.clone())
            });
        crate::notices::ring(
            self,
            project.as_deref(),
            crate::notices::kind::AGENT,
            &format!("A session is missing a sign-in: {}", missing.what),
            Some(&format!(
                "Run `{}`, then tell the session to go on.",
                missing.command
            )),
            None,
        );
    }

    fn activities(&self) -> &Mutex<Activities> {
        crate::card_activity::registry()
    }

    fn to_island(&self, change: devpit_rpc::IslandChange) {
        crate::island::tell(self, change);
    }

    fn tell_person(&self, session: &devpit_rpc::IslandSession, was: Option<devpit_rpc::Doing>) {
        crate::island_notify::changed(self, session, was);
    }

    fn card_touched(&self, card_id: &str, path: &str) {
        let (card_id, path) = (card_id.to_owned(), path.to_owned());
        std::thread::spawn(move || crate::card_elsewhere::touched(&card_id, &path));
    }

    fn card_prompted(&self, card_id: &str) {
        crate::card_follows::prompted(self, card_id);
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis() as f64)
        .unwrap_or_default()
}

fn reply(stream: &mut TcpStream, body: &str) {
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
    let _ = stream.flush();
}

/// What the window is told, in the words it draws.
fn describe(happening: &Happening) -> (String, String) {
    let said = match &happening.event {
        Event::Prompted => "took a prompt".to_owned(),
        Event::Using {
            tool,
            target: Some(target),
            ..
        } => format!("running {tool} {target}"),
        Event::Using { tool, .. } => format!("running {tool}"),
        Event::Used { tool } => format!("finished {tool}"),
        Event::UseFailed { tool, .. } => format!("{tool} failed"),
        Event::Stopped { said } => said
            .clone()
            .unwrap_or_else(|| "finished the turn".to_owned()),
        Event::Failed { error: Some(error) } => format!("stopped on an error: {error}"),
        Event::Failed { error: None } => "stopped on an error".to_owned(),
        Event::SubagentStarted {
            kind: Some(kind), ..
        } => format!("started a {kind} subagent"),
        Event::SubagentStarted { kind: None, .. } => "started a subagent".to_owned(),
        Event::Delegated {
            description: Some(description),
            ..
        } => format!("handed off: {description}"),
        Event::Delegated { .. } => "handed work to a subagent".to_owned(),
        Event::SubagentDone { .. } => "a subagent finished".to_owned(),
        Event::Waiting => "waiting on you".to_owned(),
        Event::SessionStarted => "started a session".to_owned(),
        Event::SessionEnded { .. } => "ended the session".to_owned(),
    };
    (happening.session_id.clone(), said)
}

#[cfg(test)]
#[path = "listener_tests.rs"]
mod tests;
