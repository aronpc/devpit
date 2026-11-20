use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use devpit_agentcli::{AgentSession, Kind, Status};
use devpit_rpc::{Doing, LayoutNode};

use super::*;
use crate::card_sessions::card_sessions;
use crate::post::Posted;
use crate::sessions::tab_for_card;

/// What the listener's core said, and to where.
struct Recorded {
    store_path: PathBuf,
    activities: Mutex<Activities>,
    said: Mutex<Vec<(String, serde_json::Value)>>,
    /// The cards a session took a prompt on.
    prompted: Mutex<Vec<String>>,
    /// The commands a missing sign-in was told with.
    lacked: Mutex<Vec<String>>,
}

impl HookSink for Recorded {
    fn to_window<P: serde::Serialize + Clone>(&self, channel: &str, payload: P) {
        let value = serde_json::to_value(payload).expect("payload");
        self.said
            .lock()
            .expect("said")
            .push((channel.to_owned(), value));
    }

    fn ring(&self, _store: &Store, _ring: Ring<'_>, _pane: &str) {}

    fn store(&self) -> Option<Store> {
        Store::open(&self.store_path).ok()
    }

    fn activities(&self) -> &Mutex<Activities> {
        &self.activities
    }

    fn lacks(&self, _happening: &Happening, missing: &crate::credentials::Missing) {
        self.lacked
            .lock()
            .expect("lacked")
            .push(missing.command.clone());
    }

    fn card_prompted(&self, card_id: &str) {
        self.prompted
            .lock()
            .expect("prompted")
            .push(card_id.to_owned());
    }
}

const PAYLOAD: &str = r#"{"hook_event_name":"Stop","session_id":"s1","cwd":"/tmp"}"#;
const LEAF: &str = "leaf_01HOOK";

/// The real hook command for one event, out of the settings a turn is given.
fn command_for(settings: &str, event: &str) -> String {
    let settings: serde_json::Value = serde_json::from_str(settings).expect("settings");
    settings["hooks"][event][0]["hooks"][0]["command"]
        .as_str()
        .expect("command")
        .to_owned()
}

/// Runs the real hook command for one event, the way the CLI does, and
/// answers the post it made. `pane` is set only for an agent inside a pane.
fn posted_by_hook(dir: &Path, event: &str, payload: &str, pane: Option<&str>) -> Posted {
    hooked(dir, event, payload, pane, None, "").0
}

/// [`posted_by_hook`], answering with `body`, and what the hook printed for
/// the CLI to read.
fn hooked(
    dir: &Path,
    event: &str,
    payload: &str,
    pane: Option<&str>,
    chat: Option<&str>,
    body: &str,
) -> (Posted, String) {
    let body = body.to_owned();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind");
    listener.set_nonblocking(true).expect("nonblocking");
    let endpoint = dir.join("hook-endpoint");
    let port = listener.local_addr().expect("address").port();
    std::fs::write(&endpoint, format!("http://127.0.0.1:{port}/hook")).expect("endpoint");
    let auth = endpoint.with_file_name("hook-auth");
    std::fs::write(
        &auth,
        format!("{}: a-test-secret\n", devpit_agentcli::HOOK_HEADER),
    )
    .expect("secret");

    let served = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).expect("blocking");
                    let posted = read_request(&mut stream, None);
                    reply(&mut stream, &body);
                    return posted;
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => return None,
            }
        }
    });

    let command = command_for(&devpit_agentcli::settings_json(&endpoint, &auth), event);
    let mut hook = Command::new("sh");
    hook.arg("-c")
        .arg(&command)
        .env_remove("DEVPIT_PANE")
        .env_remove(devpit_agentapi::hook::CHAT_ENV)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(pane) = pane {
        hook.env("DEVPIT_PANE", pane);
    }
    if let Some(chat) = chat {
        hook.env(devpit_agentapi::hook::CHAT_ENV, chat);
    }
    let mut child = hook.spawn().expect("hook");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("payload");
    let printed = child.wait_with_output().expect("hook ran").stdout;
    let posted = served
        .join()
        .expect("served")
        .unwrap_or_else(|| panic!("the {event} hook never posted"));
    (posted, String::from_utf8_lossy(&printed).into_owned())
}

/// What the person typed when refusing is what the CLI reads back from the
/// hook it ran, and hands the agent.
#[test]
fn a_refusal_reaches_the_cli_with_the_persons_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let asking = Asking::default();
    let picks = Picks::default();
    let hear = asking.opened("q1");
    assert!(crate::asking_picks::refuse(
        &asking,
        &picks,
        "q1",
        " use rg, not grep "
    ));
    let input = r#"{"command":"grep -r x ."}"#;
    let body = reply_for(&picks, "q1", input, asking.wait("q1", hear));
    let payload = r#"{"hook_event_name":"PreToolUse","session_id":"s1","cwd":"/tmp","tool_name":"Bash","tool_input":{"command":"grep -r x ."}}"#;

    let (_, printed) = hooked(dir.path(), "PreToolUse", payload, None, None, &body);

    let read: serde_json::Value = serde_json::from_str(&printed).expect("the CLI reads JSON");
    let said = &read["hookSpecificOutput"];
    assert_eq!(said["permissionDecision"], "deny");
    assert_eq!(
        said["permissionDecisionReason"],
        "Refused in devpit: use rg, not grep"
    );
}

/// A question a chat's process asks names that chat, through the real hook
/// (which also keeps the curl line and the Windows hook naming it alike);
/// one from a session started anywhere else names none, and goes by session.
#[test]
fn a_question_names_the_chat_whose_process_asked_and_none_outside_a_chat() {
    let dir = tempfile::tempdir().expect("tempdir");
    let payload = r#"{"hook_event_name":"PreToolUse","session_id":"ses_shared","cwd":"/tmp","tool_use_id":"q1","tool_name":"Bash","tool_input":{"command":"ls"}}"#;

    let (in_chat, _) = hooked(
        dir.path(),
        "PreToolUse",
        payload,
        None,
        Some("conv_01A"),
        "",
    );
    let asked = crate::question::asked_by(&in_chat).expect("a question");
    assert_eq!(asked.conversation_id.as_deref(), Some("conv_01A"));
    assert_eq!(asked.session_id, "ses_shared");

    let (in_terminal, _) = hooked(dir.path(), "PreToolUse", payload, Some(LEAF), None, "");
    let asked = crate::question::asked_by(&in_terminal).expect("a question");
    assert_eq!(asked.conversation_id, None);
    assert_eq!(asked.session_id, "ses_shared");
}

/// Every way a hook reaches a card, each one fired through the real hook
/// command: the pane route, a run's session, a background session, a post that
/// arrives out of order, and a session ending.
#[test]
fn a_hook_reaches_the_card() {
    for (tool, check) in [("sh", "true"), ("curl", "command -v curl")] {
        let found = Command::new("sh")
            .arg("-c")
            .arg(check)
            .status()
            .is_ok_and(|status| status.success());
        assert!(found, "a_hook_reaches_the_card needs {tool} on the PATH");
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let store_path = dir.path().join("state.db");
    let store = Store::open(&store_path).expect("store");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    let project = store.add_project(&root, None).expect("project");
    store.ensure_board(&project).expect("board");
    let column = store.columns(&project).expect("columns")[0].id.clone();
    let card = store
        .create_card(&project, &column, "a card", "")
        .expect("card");
    let tree = serde_json::to_string(&LayoutNode::leaf(LEAF, "s:leaf")).expect("tree");
    store
        .set_pane_layout(&project, &tab_for_card(&card), &tree, LEAF)
        .expect("layout");
    let step = store
        .create_step(&project, "agent", "review", "{}", false)
        .expect("step");
    let run = store.start_run(&card, &step, None).expect("run");
    store.set_run_session(&run, "s-run").expect("run session");
    store
        .link_session(&card, "a1b2", "s-bg", None, None, None)
        .expect("background");
    drop(store);

    let sink = Recorded {
        store_path,
        activities: Mutex::default(),
        said: Mutex::default(),
        prompted: Mutex::default(),
        lacked: Mutex::default(),
    };
    let state_of = |kind: SessionKind, reference: &str| {
        sink.activities
            .lock()
            .expect("activities")
            .happening(&card)
            .sessions
            .iter()
            .find(|one| one.kind == kind && one.reference == reference)
            .and_then(|one| one.state)
    };

    // The pane route: the agent in the card's pane stops.
    let stopped = posted_by_hook(
        dir.path(),
        "Stop",
        r#"{"hook_event_name":"Stop","session_id":"s-pane","cwd":"/w"}"#,
        Some(LEAF),
    );
    hear_post(&sink, &stopped, 10);
    assert_eq!(state_of(SessionKind::Pane, LEAF), Some(Doing::Done));
    {
        let said = sink.said.lock().expect("said");
        let (_, happening) = said
            .iter()
            .rev()
            .find(|(channel, _)| channel == "card:happening")
            .expect("the card heard it");
        assert_eq!(happening["cardId"], card.as_str());
        assert_eq!(happening["sessions"][0]["ref"], LEAF);
        assert_eq!(
            happening["sessions"][0]["tabId"],
            tab_for_card(&card).as_str()
        );
    }

    // A run's session id, from a headless turn with no pane.
    let used = posted_by_hook(
        dir.path(),
        "PostToolUse",
        r#"{"hook_event_name":"PostToolUse","tool_name":"Edit","session_id":"s-run","cwd":"/w"}"#,
        None,
    );
    assert_eq!(used.pane, None);
    hear_post(&sink, &used, 11);
    assert_eq!(state_of(SessionKind::Run, "s-run"), Some(Doing::Working));

    // A background session's id.
    let asked = posted_by_hook(
        dir.path(),
        "Notification",
        r#"{"hook_event_name":"Notification","session_id":"s-bg","cwd":"/w","message":"needs you"}"#,
        None,
    );
    hear_post(&sink, &asked, 12);
    assert_eq!(
        state_of(SessionKind::Background, "s-bg"),
        Some(Doing::Waiting)
    );

    // Out of order: a post stamped before the stop, served after it.
    let late = posted_by_hook(
        dir.path(),
        "Notification",
        r#"{"hook_event_name":"Notification","session_id":"s-pane","cwd":"/w","message":"needs you"}"#,
        Some(LEAF),
    );
    hear_post(&sink, &late, 9);
    assert_eq!(state_of(SessionKind::Pane, LEAF), Some(Doing::Done));

    // SessionEnded: the pane's session ends.
    let ended = posted_by_hook(
        dir.path(),
        "SessionEnd",
        r#"{"hook_event_name":"SessionEnd","session_id":"s-pane","cwd":"/w","reason":"prompt_input_exit"}"#,
        Some(LEAF),
    );
    hear_post(&sink, &ended, 13);
    assert_eq!(state_of(SessionKind::Pane, LEAF), Some(Doing::Gone));
    assert_eq!(
        sink.activities
            .lock()
            .expect("activities")
            .happening(&card)
            .activity,
        Some(Doing::Waiting)
    );
}

/// What a background session's hook posts when it stops on a person: the
/// `Notification` shape the CLI sends, with no pane in the query.
const BACKGROUND_WAITING: &str = r#"{"hook_event_name":"Notification","session_id":"s-bg","cwd":"/w/card","message":"Claude needs your permission to use Bash","notification_type":"permission_prompt"}"#;

#[test]
fn a_background_session_waiting_reaches_its_card() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store_path = dir.path().join("state.db");
    let store = Store::open(&store_path).expect("store");
    let project = store.add_project(dir.path(), None).expect("project");
    store.ensure_board(&project).expect("board");
    let column = store.columns(&project).expect("columns")[0].id.clone();
    let card = store
        .create_card(&project, &column, "a card", "")
        .expect("card");
    store
        .link_session(&card, "a1b2", "s-bg", None, Some("/w/card"), None)
        .expect("link");
    let sink = Recorded {
        store_path,
        activities: Mutex::default(),
        said: Mutex::default(),
        prompted: Mutex::default(),
        lacked: Mutex::default(),
    };

    let posted = Posted {
        body: BACKGROUND_WAITING.to_owned(),
        pane: None,
        secret: None,
        agent: false,
        chat: None,
    };
    hear_post(&sink, &posted, next_seq());

    let heard = sink.activities.lock().expect("activities").happening(&card);
    assert_eq!(heard.activity, Some(Doing::Waiting));
    assert_eq!(heard.sessions[0].kind, SessionKind::Background);
    assert_eq!(heard.sessions[0].reference, "s-bg");
    assert!(sink
        .said
        .lock()
        .expect("said")
        .iter()
        .any(|(channel, _)| channel == "card:happening"));

    // Combined when read: while the CLI lists it, its hook's word stands; once
    // the CLI no longer lists it, it is gone whatever it said last.
    let listed = AgentSession {
        session_id: "s-bg".to_owned(),
        short_id: Some("a1b2".to_owned()),
        name: None,
        cwd: "/w/card".to_owned(),
        pid: None,
        started_at: None,
        kind: Kind::Background,
        status: Status::Busy,
    };
    let read = card_sessions(&store, &project, &card, &[listed], &heard);
    assert_eq!(read[0].state, Some(Doing::Waiting));
    let unlisted = card_sessions(&store, &project, &card, &[], &heard);
    assert_eq!(unlisted[0].state, Some(Doing::Gone));
}

#[test]
fn a_hook_from_an_archived_cards_pane_does_not_bring_it_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store_path = dir.path().join("state.db");
    let store = Store::open(&store_path).expect("store");
    let project = store.add_project(dir.path(), None).expect("project");
    store.ensure_board(&project).expect("board");
    let column = store.columns(&project).expect("columns")[0].id.clone();
    let card = store
        .create_card(&project, &column, "a card", "")
        .expect("card");
    let tree = serde_json::to_string(&LayoutNode::leaf(LEAF, "s:leaf")).expect("tree");
    store
        .set_pane_layout(&project, &tab_for_card(&card), &tree, LEAF)
        .expect("layout");
    let sink = Recorded {
        store_path,
        activities: Mutex::default(),
        said: Mutex::default(),
        prompted: Mutex::default(),
        lacked: Mutex::default(),
    };
    let posted = Posted {
        body: PAYLOAD.to_owned(),
        pane: Some(LEAF.to_owned()),
        secret: None,
        agent: false,
        chat: None,
    };
    hear_post(&sink, &posted, next_seq());
    assert!(!sink
        .activities
        .lock()
        .expect("lock")
        .happening(&card)
        .sessions
        .is_empty());

    // Archived: the card forgets its sessions, and the agent keeps talking.
    store.archive_card(&card).expect("archive");
    crate::card_activity::forget_card(&mut sink.activities.lock().expect("lock"), &card);
    sink.said.lock().expect("said").clear();
    hear_post(&sink, &posted, next_seq());

    assert!(sink
        .activities
        .lock()
        .expect("lock")
        .happening(&card)
        .sessions
        .is_empty());
    assert!(!sink
        .said
        .lock()
        .expect("said")
        .iter()
        .any(|(channel, _)| channel == "card:happening"));
}

/// Only this run's secret opens the door.
///
/// The port is visible to anything on the machine that can list sockets; the
/// secret is in a file only its owner can read. Length is checked first and
/// then every byte is compared, so a caller learns nothing from how long the
/// refusal took.
#[test]
fn only_the_secret_this_run_made_is_heard() {
    assert!(authorized(Some("6f1c"), "6f1c"));
    assert!(!authorized(None, "6f1c"), "a post with no header was heard");
    assert!(!authorized(Some(""), "6f1c"));
    assert!(!authorized(Some("6f1d"), "6f1c"));
    assert!(
        !authorized(Some("6f1c00"), "6f1c"),
        "a longer guess was heard"
    );
}

/// A card's session taking a prompt asks for the card to move to work in
/// progress; anything else it says does not.
#[test]
fn a_prompt_in_a_cards_session_moves_its_card_along() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store_path = dir.path().join("state.db");
    let store = Store::open(&store_path).expect("store");
    let project = store.add_project(dir.path(), None).expect("project");
    store.ensure_board(&project).expect("board");
    let column = store.columns(&project).expect("columns")[0].id.clone();
    let card = store
        .create_card(&project, &column, "a card", "")
        .expect("card");
    store
        .link_session(&card, "a1b2", "s-bg", None, Some("/w/card"), None)
        .expect("link");
    let sink = Recorded {
        store_path,
        activities: Mutex::default(),
        said: Mutex::default(),
        prompted: Mutex::default(),
        lacked: Mutex::default(),
    };
    let said = |event: &str| Posted {
        body: format!(r#"{{"hook_event_name":"{event}","session_id":"s-bg","cwd":"/w/card"}}"#),
        pane: None,
        secret: None,
        agent: false,
        chat: None,
    };
    hear_post(&sink, &said("Stop"), next_seq());
    assert!(sink.prompted.lock().expect("prompted").is_empty());
    hear_post(&sink, &said("UserPromptSubmit"), next_seq());
    assert_eq!(*sink.prompted.lock().expect("prompted"), [card]);
}

/// A tool that failed for want of a sign-in tells the person the command; any
/// other failure tells nothing.
#[test]
fn a_tool_that_fails_for_a_sign_in_says_what_fixes_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sink = Recorded {
        store_path: dir.path().join("state.db"),
        activities: Mutex::default(),
        said: Mutex::default(),
        prompted: Mutex::default(),
        lacked: Mutex::default(),
    };
    let posted = |error: &str| Posted {
        body: serde_json::json!({
            "hook_event_name": "PostToolUseFailure",
            "session_id": "s-cred",
            "cwd": "/w",
            "tool_name": "Bash",
            "error": error,
        })
        .to_string(),
        pane: None,
        secret: None,
        agent: false,
        chat: None,
    };
    hear_post(&sink, &posted("error[E0425]: cannot find value"), 1);
    hear_post(
        &sink,
        &posted("An error occurred (ExpiredToken) ... --profile asc"),
        2,
    );
    assert_eq!(
        *sink.lacked.lock().expect("lacked"),
        ["aws sso login --profile asc"]
    );
}

/// A `PreToolUse` the hook posted for `tool`: from a session a chat draws
/// when `chat` names the conversation, and from a terminal's own or a
/// board step's when it does not.
fn posted_asking(tool: &str, chat: Option<&str>) -> Posted {
    Posted {
        body: format!(
            r#"{{"hook_event_name":"PreToolUse","session_id":"s1","cwd":"/w","tool_use_id":"q1","tool_name":"{tool}","tool_input":{{}}}}"#
        ),
        pane: None,
        secret: None,
        agent: false,
        chat: chat.map(str::to_owned),
    }
}

/// A chat's question stays on the CLI prompt, outside the hook's curl budget.
#[test]
fn a_chats_question_to_the_person_is_never_held_by_the_hook() {
    let asking = Asking::default();
    assert!(
        held_by_the_hook(&posted_asking("AskUserQuestion", Some("conv_01A")), &asking).is_none(),
        "a chat's mode is not one that asks"
    );
    // A chat a person put in Supervised asks about its changes; its
    // question to the person is still the prompt's, not the hook's.
    asking.ask_from_now("s1", true);
    assert!(
        held_by_the_hook(&posted_asking("AskUserQuestion", Some("conv_01A")), &asking).is_none(),
        "not even a Supervised chat's"
    );
}

/// Supervised is unchanged: a change in a session that asks about changes
/// is still held by the hook, and a read still is not.
#[test]
fn a_supervised_change_is_still_held_by_the_hook() {
    let asking = Asking::default();
    asking.ask_from_now("s1", true);
    let held = held_by_the_hook(&posted_asking("Bash", Some("conv_01A")), &asking).expect("held");
    assert_eq!(held.tool, "Bash");
    assert!(
        held_by_the_hook(&posted_asking("Read", Some("conv_01A")), &asking).is_none(),
        "a read is the CLI's permission mode's to allow"
    );
    assert!(
        held_by_the_hook(&posted_asking("ExitPlanMode", Some("conv_01A")), &asking).is_none(),
        "a plan still goes to the CLI prompt"
    );
}

/// Outside a chat the hook still answers empty: a session nobody put in
/// Supervised — a terminal's own, a board step's — never waits on a window.
#[test]
fn outside_a_chat_the_hook_still_answers_empty() {
    let asking = Asking::default();
    assert!(held_by_the_hook(&posted_asking("Bash", None), &asking).is_none());
    assert!(held_by_the_hook(&posted_asking("AskUserQuestion", None), &asking).is_none());
}
