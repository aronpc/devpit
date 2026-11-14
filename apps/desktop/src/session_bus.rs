//! Orchestrators and their sessions talking through devpit, whatever CLI each
//! runs on: beside Claude Code's own SendMessage, never in its place.
//!
//! A message waits here until its recipient can take it. A session in a devpit
//! terminal gets it typed at an idle prompt, never mid-turn: its agent's hooks
//! say when a turn runs, and for an agent with none, its screen does. An
//! orchestrator gets it as a turn devpit starts in its chat between turns,
//! marked as devpit's, so nothing a session writes passes for the person's
//! word. Each message is taken off the queue once, so it is given once.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use devpit_rpc::{ErrorCode, Project};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

pub(crate) use crate::card_activity::Doing;

/// How long an agent may take to show the turn devpit typed for it.
pub(crate) const TAKES_A_TURN_MS: f64 = 15_000.0;
/// The longest message: a message, not a document.
const LONGEST: usize = 4000;
/// The most one turn in an orchestrator's chat carries of what waited for it.
const MOST_IN_A_TURN: usize = 16_000;
/// How often what waits looks at its recipient again.
const LOOK_EVERY: Duration = Duration::from_secs(2);

/// Where a message goes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Inbox {
    /// A session in one of devpit's terminals, by its project and pane.
    Pane { project: String, leaf: String },
    /// An orchestrator's chat, by its project.
    Chat { project: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Letter {
    pub to: Inbox,
    /// The words as they are given: devpit's heading, then the sender's.
    pub text: String,
}

/// An orchestrator waiting to hear, once, that a session went idle or ended.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Watch {
    pub pane: Inbox,
    pub orchestrator: String,
    pub name: String,
}

/// devpit typed into a pane: until its agent shows the turn, it is not idle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Typed {
    /// Milliseconds since the epoch.
    pub at: f64,
    pub seen_busy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pane {
    /// Its agent waits at its prompt.
    Free,
    Busy,
    /// Its agent left the pane.
    Ended,
}

/// What a look at a pane found.
pub(crate) struct Seen<'a> {
    /// What its agent last said through a hook, if it ever did.
    pub hook: Option<Doing>,
    /// Whether the agent, not the shell, is in front.
    pub agent: bool,
    /// Its screen, when it could be read.
    pub screen: Option<&'a str>,
}

/// Who wrote a session's message, as the orchestrator is told.
pub(crate) struct Sender {
    /// Its name, else its pane's id.
    pub name: String,
    pub agent: String,
    pub project: String,
    pub card: Option<u32>,
    /// Whether a message can reach it back: a session in a terminal.
    pub answerable: bool,
}

#[derive(Default)]
pub(crate) struct Bus {
    queued: VecDeque<Letter>,
    watches: Vec<Watch>,
    /// Which orchestrator last wrote to each pane: where its answer goes.
    answers: HashMap<String, String>,
    typed: HashMap<String, Typed>,
    /// How many looks in a row found each pane free.
    calm: HashMap<String, u8>,
    /// The chats a turn of devpit's is starting or running in.
    chatting: HashSet<String>,
}

impl Bus {
    pub(crate) fn post(&mut self, letter: Letter) {
        self.queued.push_back(letter);
    }

    /// The first message waiting for `to`, taken off the queue.
    pub(crate) fn take(&mut self, to: &Inbox) -> Option<Letter> {
        let at = self.queued.iter().position(|one| &one.to == to)?;
        self.queued.remove(at)
    }

    /// What waits for `to` in the order sent, as one text: up to `most`
    /// characters, and at least one message.
    pub(crate) fn take_all(&mut self, to: &Inbox, most: usize) -> Option<String> {
        let mut taken: Vec<String> = Vec::new();
        let mut size = 0;
        while let Some(at) = self.queued.iter().position(|one| &one.to == to) {
            let next = self.queued[at].text.chars().count();
            if !taken.is_empty() && size + next > most {
                break;
            }
            size += next;
            taken.extend(self.queued.remove(at).map(|one| one.text));
        }
        (!taken.is_empty()).then(|| taken.join("\n\n"))
    }

    /// A message that could not go after all, back before the rest.
    pub(crate) fn put_back(&mut self, letter: Letter) {
        self.queued.push_front(letter);
    }

    /// Every recipient something waits for, in the order sent.
    pub(crate) fn waiting(&self) -> Vec<Inbox> {
        let mut all: Vec<Inbox> = Vec::new();
        for one in &self.queued {
            if !all.contains(&one.to) {
                all.push(one.to.clone());
            }
        }
        all
    }

    /// False when that orchestrator already watches that pane.
    pub(crate) fn watch(&mut self, watch: Watch) -> bool {
        let twice = self
            .watches
            .iter()
            .any(|one| one.pane == watch.pane && one.orchestrator == watch.orchestrator);
        if !twice {
            self.watches.push(watch);
        }
        !twice
    }

    pub(crate) fn watched(&self) -> Vec<Inbox> {
        let mut all: Vec<Inbox> = Vec::new();
        for one in &self.watches {
            if !all.contains(&one.pane) {
                all.push(one.pane.clone());
            }
        }
        all
    }

    /// The watches on `pane`, let go: each is told once.
    pub(crate) fn idle(&mut self, pane: &Inbox) -> Vec<Watch> {
        let (told, kept) = std::mem::take(&mut self.watches)
            .into_iter()
            .partition(|one| &one.pane == pane);
        self.watches = kept;
        told
    }

    /// A pane whose agent left: what waited for it is dropped, its watches
    /// let go.
    pub(crate) fn ended(&mut self, pane: &Inbox) -> (Vec<Letter>, Vec<Watch>) {
        let (dropped, kept): (Vec<Letter>, Vec<Letter>) = std::mem::take(&mut self.queued)
            .into_iter()
            .partition(|one| &one.to == pane);
        self.queued = kept.into();
        if let Inbox::Pane { leaf, .. } = pane {
            self.typed.remove(leaf);
            self.calm.remove(leaf);
            self.answers.remove(leaf);
        }
        (dropped, self.idle(pane))
    }

    pub(crate) fn wrote(&mut self, leaf: &str, orchestrator: &str) {
        self.answers
            .insert(leaf.to_owned(), orchestrator.to_owned());
    }

    pub(crate) fn answers(&self, leaf: &str) -> Option<&str> {
        self.answers.get(leaf).map(String::as_str)
    }

    fn typed_into(&mut self, leaf: &str, at: f64) {
        let typed = Typed {
            at,
            seen_busy: false,
        };
        self.typed.insert(leaf.to_owned(), typed);
        self.calm.remove(leaf);
    }

    /// `now` for a pane, kept steady: a screen alone must read free twice in
    /// a row, since one look can fall between two steps of a turn.
    pub(crate) fn settled(&mut self, leaf: &str, now: Pane, hooked: bool) -> Pane {
        if now != Pane::Free {
            self.calm.remove(leaf);
            return now;
        }
        if hooked {
            return Pane::Free;
        }
        let calm = self.calm.entry(leaf.to_owned()).or_default();
        *calm = calm.saturating_add(1);
        if *calm >= 2 {
            Pane::Free
        } else {
            Pane::Busy
        }
    }

    /// A look at `leaf` kept: what it says of a turn devpit typed, and the
    /// pane as it stands.
    fn look(&mut self, leaf: &str, seen: &Seen, at: f64) -> Pane {
        if busy(seen) {
            if let Some(typed) = self.typed.get_mut(leaf) {
                typed.seen_busy = true;
            }
        }
        let now = pane_now(seen, self.typed.get(leaf), at);
        if now != Pane::Busy {
            self.typed.remove(leaf);
        }
        self.settled(leaf, now, seen.hook.is_some())
    }
}

/// Whether a look found a turn running or a question waiting.
fn busy(seen: &Seen) -> bool {
    matches!(seen.hook, Some(Doing::Working | Doing::Waiting))
        || seen.screen.is_some_and(|screen| {
            busy_on_screen(screen) || crate::live_prompt::pending(screen).is_some()
        })
}

/// Whether a screen's last lines show a turn running, which the agents devpit
/// starts offer to interrupt, or a choice open under a cursor.
fn busy_on_screen(screen: &str) -> bool {
    screen
        .lines()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .take(12)
        .any(|line| {
            let lower = line.to_lowercase();
            lower.contains("esc to interrupt") || lower.contains("esc to cancel") || a_choice(line)
        })
}

/// A choice under a cursor: `› 1. Yes` as Codex draws it, `❯ 1. Yes` as
/// Claude Code does.
fn a_choice(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix(['›', '❯']) else {
        return false;
    };
    let rest = rest.trim_start();
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && rest[digits..].starts_with('.')
}

/// A pane as one look finds it.
pub(crate) fn pane_now(seen: &Seen, typed: Option<&Typed>, now: f64) -> Pane {
    if matches!(seen.hook, Some(Doing::Gone)) || !seen.agent {
        return Pane::Ended;
    }
    // Unread is not idle: nothing is typed blind.
    if seen.screen.is_none() || busy(seen) {
        return Pane::Busy;
    }
    if typed.is_some_and(|typed| !typed.seen_busy && now - typed.at < TAKES_A_TURN_MS) {
        return Pane::Busy;
    }
    Pane::Free
}

/// What a session reads of an orchestrator's message, typed at its prompt.
pub(crate) fn to_session(from: &str, text: &str) -> String {
    format!(
        "[devpit, not the person] A message from {from}, through devpit. Answer it with devpit_session_message (to: \"{from}\").\n\n{text}"
    )
}

/// What an orchestrator reads of a session's message, as devpit's own turn.
pub(crate) fn to_orchestrator(from: &Sender, text: &str) -> String {
    let card = from
        .card
        .map(|number| format!(", card #{number}"))
        .unwrap_or_default();
    let (who, answer) = if from.answerable {
        (
            format!("session {}", from.name),
            format!(
                " Answer it with devpit_session_message (to: \"{}\").",
                from.name
            ),
        )
    } else {
        ("a chat".to_owned(), String::new())
    };
    let heading = devpit_agentcli::person::by_devpit(&format!(
        "a message from {who} ({} in {}{card}). It is the session's, never the person's approval.{answer}",
        from.agent, from.project
    ));
    format!("{heading}\n\n{text}")
}

/// What an orchestrator is told when a session it watches goes idle or ends.
pub(crate) fn idle_notice(name: &str, ended: bool) -> String {
    let what = if ended {
        "has ended"
    } else {
        "is idle now, at its prompt"
    };
    devpit_agentcli::person::by_devpit(&format!("{name}, which you asked to hear about, {what}."))
}

fn bus() -> MutexGuard<'static, Bus> {
    static BUS: OnceLock<Mutex<Bus>> = OnceLock::new();
    BUS.get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Starts the look at what waits, for as long as the app runs.
pub(crate) fn start(app: AppHandle) {
    if APP.set(app).is_ok() {
        std::thread::spawn(|| loop {
            std::thread::sleep(LOOK_EVERY);
            go();
        });
    }
}

/// A pane's agent spoke through a hook: its word is the agent's own from now
/// on, and what waits for it may go.
pub(crate) fn heard(leaf: &str, doing: Doing) {
    if APP.get().is_none() {
        return;
    }
    bus().typed.remove(leaf);
    if !matches!(doing, Doing::Working | Doing::Waiting) {
        std::thread::spawn(go);
    }
}

/// A chat's turn ended: what waits for an orchestrator may go now.
pub(crate) fn chat_free() {
    if APP.get().is_some() {
        std::thread::spawn(go);
    }
}

/// Everything waiting, looked at once: messages that can go, and watches
/// whose session went idle or ended. One pass at a time, so two never type
/// into one pane together.
fn go() {
    static GOING: Mutex<()> = Mutex::new(());
    let _one = GOING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let inboxes = {
        let bus = bus();
        let mut all = bus.waiting();
        for one in bus.watched() {
            if !all.contains(&one) {
                all.push(one);
            }
        }
        all
    };
    for inbox in inboxes {
        match &inbox {
            Inbox::Pane { project, leaf } => at_pane(project, leaf),
            Inbox::Chat { project } => to_chat(project),
        }
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis() as f64)
        .unwrap_or_default()
}

/// A pane's terminal and how it stands, or nothing once it is gone. `kept`
/// looks count towards a steady reading; the others only glance.
fn looked(project: &str, leaf: &str, kept: bool) -> Option<(String, Pane)> {
    let target = crate::pane_io::target_of(project, leaf).ok()?;
    let screen = crate::pane_io::screen(&target);
    let seen = Seen {
        hook: crate::pane_said::heard_of(leaf),
        agent: !crate::pane_io::shell_in_front(&target).unwrap_or(false),
        screen: screen.as_deref(),
    };
    let mut bus = bus();
    let now = if kept {
        bus.look(leaf, &seen, now_ms())
    } else {
        pane_now(&seen, bus.typed.get(leaf), now_ms())
    };
    Some((target, now))
}

/// A pane's watches told and its next message typed, once it is free.
fn at_pane(project: &str, leaf: &str) {
    let inbox = Inbox::Pane {
        project: project.to_owned(),
        leaf: leaf.to_owned(),
    };
    let Some((target, now)) = looked(project, leaf, true) else {
        return ended(&inbox);
    };
    match now {
        Pane::Busy => {}
        Pane::Ended => ended(&inbox),
        Pane::Free => {
            let told = bus().idle(&inbox);
            for one in told {
                wake(&one.orchestrator, idle_notice(&one.name, false));
            }
            let Some(letter) = bus().take(&inbox) else {
                return;
            };
            match crate::pane_io::paste(&target, &letter.text, true) {
                Ok(()) => bus().typed_into(leaf, now_ms()),
                Err(_) => bus().put_back(letter),
            }
        }
    }
}

fn ended(inbox: &Inbox) {
    let (_, told) = bus().ended(inbox);
    for one in told {
        wake(&one.orchestrator, idle_notice(&one.name, true));
    }
}

/// `text` for orchestrator `project`'s chat, as soon as it is between turns.
fn wake(project: &str, text: String) {
    bus().post(Letter {
        to: Inbox::Chat {
            project: project.to_owned(),
        },
        text,
    });
    to_chat(project);
}

/// What waits for an orchestrator, sent as one turn of devpit's while no
/// other runs in its chat.
fn to_chat(project: &str) {
    let Some(app) = APP.get() else { return };
    let inbox = Inbox::Chat {
        project: project.to_owned(),
    };
    if !bus().chatting.insert(project.to_owned()) {
        return;
    }
    let conversation = crate::channel_talk::latest_conversation(project);
    let running = app
        .state::<crate::chat::Talking>()
        .running
        .lock()
        .map_or(true, |held| held.contains_key(&conversation));
    let text = (!running)
        .then(|| bus().take_all(&inbox, MOST_IN_A_TURN))
        .flatten();
    let Some(text) = text else {
        bus().chatting.remove(project);
        return;
    };
    let (app, project) = (app.clone(), project.to_owned());
    tauri::async_runtime::spawn(async move {
        let ran = match crate::remote_talk::devpits_turn(&project, &conversation, &text) {
            Ok(ask) => crate::channel_talk::turn_in(&app, &conversation, ask)
                .await
                .map(|_| ()),
            Err(why) => Err(devpit_rpc::RpcError::new(ErrorCode::Invalid, why)),
        };
        let mut bus = bus();
        bus.chatting.remove(&project);
        match ran {
            // Another turn took the chat first: this one waits for its end.
            Err(err) if matches!(err.code, ErrorCode::Conflict | ErrorCode::Busy) => {
                bus.put_back(Letter { to: inbox, text });
            }
            Err(err) => eprintln!(
                "a message for an orchestrator was not delivered: {}",
                err.message
            ),
            Ok(()) => {}
        }
    });
}

/// `devpit_session_message`: an orchestrator writing to a session, or a
/// session to its orchestrator.
pub(crate) fn message(
    projects: &[Project],
    here: &Project,
    params: &Value,
    author: &str,
    cwd: &str,
) -> Result<Value, String> {
    let said = |name: &str| params.get(name).and_then(Value::as_str).map(str::trim);
    let text = said("text").unwrap_or_default();
    if text.is_empty() || text.chars().count() > LONGEST {
        return Err(format!("a message is between 1 and {LONGEST} characters"));
    }
    let to = said("to").filter(|to| !to.is_empty());
    if let Some(profile) = here.orchestrator.as_deref() {
        let to = to.ok_or(WHICH_SESSION)?;
        let (inbox, name) = pane_named(here, profile, to)?;
        let from = crate::orchestrator::address(Path::new(&here.root_path))
            .unwrap_or_else(|| here.name.clone());
        if let Inbox::Pane { leaf, .. } = &inbox {
            bus().wrote(leaf, &here.id);
        }
        bus().post(Letter {
            to: inbox.clone(),
            text: to_session(&from, text),
        });
        go();
        let typed = !bus().waiting().contains(&inbox);
        let note = if typed {
            "Typed at its prompt. Its answer reaches this chat as a turn of devpit's, not the person's."
        } else {
            "It waits in devpit and is typed at the session's next idle prompt, never mid-turn."
        };
        return Ok(json!({ "sent": name, "typed": typed, "note": note }));
    }
    let pane = said("fromPane").filter(|pane| !pane.is_empty());
    let orchestrator = orchestrator_for(projects, here, to, pane)?;
    let from = sender(here, orchestrator, pane, author, cwd);
    let address = crate::orchestrator::address(Path::new(&orchestrator.root_path))
        .unwrap_or_else(|| orchestrator.name.clone());
    wake(&orchestrator.id, to_orchestrator(&from, text));
    Ok(json!({
        "sent": address,
        "note": "It reaches the orchestrator as a turn in its chat between its turns, marked as yours and not the person's.",
    }))
}

/// `devpit_session_notify_idle`: a watch on a session, told once.
pub(crate) fn notify(here: &Project, profile: &str, params: &Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or(WHICH_SESSION)?;
    let (inbox, name) = pane_named(here, profile, name)?;
    let Inbox::Pane { project, leaf } = &inbox else {
        return Err(WHICH_SESSION.to_owned());
    };
    match looked(project, leaf, false).map_or(Pane::Ended, |(_, now)| now) {
        Pane::Free => Ok(json!({
            "idle": true,
            "name": name,
            "note": "Idle at its prompt already: there is nothing to wait for.",
        })),
        Pane::Ended => Ok(json!({ "ended": true, "name": name, "note": "It has ended already." })),
        Pane::Busy => {
            bus().watch(Watch {
                pane: inbox,
                orchestrator: here.id.clone(),
                name: name.clone(),
            });
            Ok(json!({
                "watching": name,
                "note": "devpit starts a turn in this chat, once, when it goes idle or exits. That turn is devpit's, not the person's.",
            }))
        }
    }
}

const WHICH_SESSION: &str =
    "which session? pass its name as devpit_sessions gives it, or its pane id as devpit_card lists it";

/// The pane of a session in reach of orchestrator `here`, by its name or its
/// pane's id, and the name to call it by.
fn pane_named(here: &Project, profile: &str, to: &str) -> Result<(Inbox, String), String> {
    if to.starts_with("leaf_") {
        let linked = crate::orchestrator_links::linked(Path::new(&here.root_path));
        let store = crate::projects::store().map_err(|err| err.message)?;
        let project = crate::sessions::project_holding(&store, to)
            .filter(|project| linked.contains(project))
            .ok_or_else(|| format!("no pane {to} in a project linked to this orchestrator"))?;
        let inbox = Inbox::Pane {
            project,
            leaf: to.to_owned(),
        };
        return Ok((inbox, to.to_owned()));
    }
    let live =
        crate::live_sessions::orchestrator_sessions_now(profile).map_err(|err| err.message)?;
    let one = crate::orchestrator_links::reachable(here, live.sessions)
        .into_iter()
        .find(|one| one.name == to)
        .ok_or_else(|| format!("{to} is not running in a project linked to this orchestrator"))?;
    let pane = one
        .pane
        .ok_or_else(|| format!("{to} is not in a devpit terminal: SendMessage reaches it"))?;
    let inbox = Inbox::Pane {
        project: pane.project_id,
        leaf: pane.pane_id,
    };
    Ok((inbox, to.to_owned()))
}

/// The orchestrator a session of `here` writes to: the one named, else the
/// one that last wrote to its pane, else the only one linked to it.
fn orchestrator_for<'a>(
    projects: &'a [Project],
    here: &Project,
    to: Option<&str>,
    pane: Option<&str>,
) -> Result<&'a Project, String> {
    let linking: Vec<&Project> = projects
        .iter()
        .filter(|one| one.orchestrator.is_some())
        .filter(|one| {
            crate::orchestrator_links::linked(Path::new(&one.root_path)).contains(&here.id)
        })
        .collect();
    let address = |one: &Project| crate::orchestrator::address(Path::new(&one.root_path));
    let names = linking
        .iter()
        .filter_map(|one| address(one))
        .collect::<Vec<_>>()
        .join(", ");
    if let Some(to) = to {
        return linking
            .iter()
            .find(|one| {
                address(one).as_deref() == Some(to)
                    || one.id == to
                    || one.name.eq_ignore_ascii_case(to)
            })
            .copied()
            .ok_or_else(|| {
                format!("a session writes to an orchestrator linked to its project, and none is called {to}: {names}")
            });
    }
    let last = pane.and_then(|pane| bus().answers(pane).map(str::to_owned));
    last.and_then(|id| linking.iter().find(|one| one.id == id).copied())
        .or_else(|| (linking.len() == 1).then(|| linking[0]))
        .ok_or_else(|| format!("which orchestrator? pass `to`: {names}"))
}

/// Who a session of `here` is, as `orchestrator` is told: its name there
/// when it has one, else its pane's id.
fn sender(
    here: &Project,
    orchestrator: &Project,
    pane: Option<&str>,
    author: &str,
    cwd: &str,
) -> Sender {
    let named = pane.and_then(|leaf| {
        let profile = orchestrator.orchestrator.as_deref()?;
        crate::live_sessions::orchestrator_sessions_now(profile)
            .ok()?
            .sessions
            .into_iter()
            .find(|one| one.pane.as_ref().is_some_and(|at| at.pane_id == leaf))
            .map(|one| one.name)
    });
    let worktrees = devpit_core::Store::root()
        .ok()
        .and_then(|root| root.join("worktrees").canonicalize().ok())
        .unwrap_or_default();
    let card = crate::live_sessions::card_of(&worktrees, Path::new(cwd)).and_then(|card| {
        crate::projects::store()
            .ok()?
            .card_numbers(&here.id)
            .ok()?
            .get(&card)
            .copied()
    });
    Sender {
        name: named
            .or_else(|| pane.map(str::to_owned))
            .unwrap_or_default(),
        agent: crate::agent_api::signed(author).to_owned(),
        project: here.name.clone(),
        card,
        answerable: pane.is_some(),
    }
}

#[cfg(test)]
#[path = "session_bus_tests.rs"]
mod tests;
