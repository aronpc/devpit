use std::path::{Path, PathBuf};

use devpit_agentcli::person::{last_said, From};
use devpit_core::home::ProjectHome;
use devpit_core::Store;
use devpit_rpc::{CallState, Conversation, Message, Part, Role, Transcript, TurnFrom};
use serde_json::json;

use super::{
    ask_of, for_phone, heard, photo, plain, voice_kind, Talk, Uploads, MOST_VOICE, PHONE_MESSAGES,
    PHONE_OUTPUT,
};
use crate::chat_relay::Relay;
use crate::pasting::MOST_BYTES;

fn message(role: Role, parts: Vec<Part>) -> Message {
    Message {
        id: format!("msg_{}", ulid::Ulid::generate()),
        turn_id: None,
        role,
        parts,
        created_at: 0.0,
        streaming: false,
        ended: None,
    }
}

fn text(said: &str) -> Part {
    Part::Text {
        text: said.to_owned(),
        parent: None,
    }
}

fn conversation(profile: &str, messages: Vec<Message>) -> Conversation {
    Conversation {
        id: "4f0c2a8e-9d1b-4c55-8a3e-2b7d9e6f1a00".to_owned(),
        project_id: "orchestrator-q2x9k1".to_owned(),
        card_id: None,
        card_title: None,
        card_on_board: false,
        profile: profile.to_owned(),
        model: Some("opus".to_owned()),
        permission: Some("acceptEdits".to_owned()),
        effort: Some("high".to_owned()),
        cwd: (!profile.is_empty()).then(|| "/home/me/.devpit/orchestrator/work".to_owned()),
        session_id: None,
        messages,
        older: false,
        cost_usd: 0.0,
        context: None,
        rewindable: Vec::new(),
        created_at: 0.0,
    }
}

#[test]
fn the_phone_reads_the_latest_of_a_conversation_with_long_output_cut() {
    let mut messages: Vec<Message> = (0..PHONE_MESSAGES + 20)
        .map(|at| message(Role::User, vec![text(&format!("message {at}"))]))
        .collect();
    messages.push(message(
        Role::Assistant,
        vec![
            Part::ToolCall {
                id: "call_1".to_owned(),
                name: "Bash".to_owned(),
                input: r#"{"command":"cargo test"}"#.to_owned(),
                state: CallState::Ok,
                parent: None,
            },
            Part::ToolResult {
                call_id: "call_1".to_owned(),
                output: "test passed\n".repeat(2_000),
                is_error: false,
                parent: None,
            },
            text("All green."),
        ],
    ));
    let read = for_phone(conversation("claude-work", messages));
    assert_eq!(read.messages.len(), PHONE_MESSAGES);
    let Part::Text { text: first, .. } = &read.messages[0].parts[0] else {
        panic!("a message of the person's");
    };
    assert_eq!(first, "message 21");
    let last = read.messages.last().expect("the answer");
    let Part::ToolResult { output, .. } = &last.parts[1] else {
        panic!("the output");
    };
    assert_eq!(output.chars().count(), PHONE_OUTPUT + 1);
    assert!(output.ends_with('…'));
    // What the row names and what the answer says go whole.
    assert_eq!(
        last.parts[0],
        Part::ToolCall {
            id: "call_1".to_owned(),
            name: "Bash".to_owned(),
            input: r#"{"command":"cargo test"}"#.to_owned(),
            state: CallState::Ok,
            parent: None,
        }
    );
    assert_eq!(last.parts[2], text("All green."));
}

#[test]
fn a_message_from_the_phone_is_the_persons_turn_marked_from_the_remote() {
    let talk = conversation("claude-work", Vec::new());
    let ask = ask_of(
        &talk,
        None,
        "/elsewhere",
        "  envie  ",
        Some(TurnFrom::Remote),
    )
    .expect("a turn");
    assert_eq!(ask.from, Some(TurnFrom::Remote));
    assert_eq!(ask.prompt, "envie");
    assert_eq!(ask.profile_id, "claude-work");
    assert_eq!(ask.model.as_deref(), Some("opus"));
    assert_eq!(ask.cwd, "/home/me/.devpit/orchestrator/work");

    // What the agent is sent, as its log keeps a turn from the composer's
    // door: the person's words, said from the Remote.
    let sent = crate::session_notice::notices(
        ask.from.as_ref(),
        Some("[devpit, not the person] Since your last message: api ended."),
        &ask.prompt,
    );
    let log = json!({ "type": "user", "uuid": "u1", "promptSource": "sdk", "message": { "role": "user", "content": sent } });
    let person = last_said(&log.to_string()).expect("the person");
    assert_eq!(person.text, "envie");
    assert_eq!(person.from, From::Remote);

    // A chat with no turn yet speaks as its orchestrator's account, in its
    // folder; anywhere else it waits for the machine.
    let fresh = conversation("", Vec::new());
    let first = ask_of(
        &fresh,
        Some("claude-work"),
        "/orchestrator",
        "oi",
        Some(TurnFrom::Remote),
    )
    .expect("a turn");
    assert_eq!(first.profile_id, "claude-work");
    assert_eq!(first.cwd, "/orchestrator");
    assert!(ask_of(&fresh, None, "/orchestrator", "oi", Some(TurnFrom::Remote)).is_err());
    assert!(ask_of(&talk, None, "/elsewhere", "   ", Some(TurnFrom::Remote)).is_err());
}

struct Machine {
    _dir: tempfile::TempDir,
    home: PathBuf,
    store: Store,
    project: String,
    checkout: PathBuf,
}

fn machine() -> Machine {
    let dir = tempfile::tempdir().expect("tempdir");
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).expect("home");
    let store = Store::open(&home.join("state.db")).expect("store");
    let checkout = dir.path().join("api");
    std::fs::create_dir_all(&checkout).expect("checkout");
    let project = store.add_project(&checkout, None).expect("project");
    Machine {
        _dir: dir,
        home,
        store,
        project,
        checkout,
    }
}

const JPEG: &[u8] = b"\xff\xd8\xff\xe0 a photo of the error on screen";

#[test]
fn a_photo_from_the_phone_is_kept_in_the_project_where_its_path_names_it() {
    let machine = machine();
    let kept = photo(
        &machine.store,
        &machine.home,
        &machine.project,
        "image/jpeg",
        JPEG,
    )
    .expect("kept");
    let path = Path::new(&kept.path);
    assert_eq!(std::fs::read(path).expect("the file"), JPEG);
    assert_eq!(kept.kind, "jpg");
    assert!(kept.name.starts_with("pasted-") && kept.name.ends_with(".jpg"));
    // In the project's own pasted pictures, the folder a paste goes to.
    let pasted = ProjectHome::of(&machine.store, &machine.home, &machine.project)
        .expect("home")
        .pasted();
    assert_eq!(
        path.canonicalize().expect("path").parent(),
        Some(pasted.canonicalize().expect("pasted").as_path())
    );
    // Nothing is written into the repository, or out of the project's home.
    assert_eq!(
        std::fs::read_dir(&machine.checkout)
            .expect("checkout")
            .count(),
        0
    );
    assert!(photo(
        &machine.store,
        &machine.home,
        "../escape",
        "image/jpeg",
        JPEG
    )
    .is_err());
    assert!(!machine.home.join("escape").exists());
}

#[test]
fn an_oversize_or_foreign_file_from_the_phone_is_refused() {
    let machine = machine();
    let at = |media_type: &str, bytes: &[u8]| {
        photo(
            &machine.store,
            &machine.home,
            &machine.project,
            media_type,
            bytes,
        )
    };
    assert!(at("image/svg+xml", b"<svg/>").is_err());
    assert!(at("audio/webm", JPEG).is_err());
    assert!(at("image/jpeg", &vec![0; MOST_BYTES + 1]).is_err());

    assert_eq!(voice_kind("audio/webm;codecs=opus", 40_000), Ok("webm"));
    assert_eq!(voice_kind("audio/mp4", MOST_VOICE), Ok("m4a"));
    assert!(voice_kind("audio/mp4", MOST_VOICE + 1).is_err());
    assert!(voice_kind("video/mp4", 40_000).is_err());
    assert!(voice_kind("image/png", 40_000).is_err());

    // In pieces, the file is refused the moment it would pass the most a
    // file may be, and what came before it is let go.
    let mut uploads = Uploads::default();
    let piece = "A".repeat(4 * 1024 * 1024);
    assert!(uploads.add("photo-1", &piece).is_ok());
    assert!(uploads.add("photo-1", &piece).is_ok());
    assert!(uploads.add("photo-1", &piece).is_err());
    assert_eq!(uploads.take("photo-1"), None);
    // Two at a time, under a name that reaches nowhere.
    assert!(uploads.add("a", "AAAA").is_ok());
    assert!(uploads.add("b", "AAAA").is_ok());
    assert!(uploads.add("c", "AAAA").is_err());
    assert!(uploads.add("../a", "AAAA").is_err());
    assert_eq!(uploads.take("a"), Some(vec![0, 0, 0]));
}

#[tokio::test]
async fn a_recording_from_the_phone_is_heard_and_not_kept() {
    let dir = tempfile::tempdir().expect("tempdir");
    let words = heard(dir.path(), "m4a", b"AAC", |audio| async move {
        assert_eq!(std::fs::read(&audio).expect("the recording"), b"AAC");
        Ok(Transcript {
            text: Some("deploy the api".to_owned()),
            note: None,
        })
    })
    .await
    .expect("heard");
    assert_eq!(words.text.as_deref(), Some("deploy the api"));
    assert_eq!(std::fs::read_dir(dir.path()).expect("dir").count(), 0);
    // Not heard, it goes all the same.
    let failed = heard(dir.path(), "webm", b"OPUS", |_| async {
        Err(devpit_rpc::RpcError::internal("no whisper"))
    })
    .await;
    assert!(failed.is_err());
    assert_eq!(std::fs::read_dir(dir.path()).expect("dir").count(), 0);
}

#[test]
fn a_device_names_a_conversation_or_a_file_only_plainly() {
    assert!(plain("4f0c2a8e-9d1b-4c55-8a3e-2b7d9e6f1a00"));
    assert!(plain("01k2abc_photo"));
    assert!(!plain(""));
    assert!(!plain("../sessions/other"));
    assert!(!plain("a/b"));
    assert!(!plain(&"x".repeat(65)));
}

fn nobody() -> tauri::ipc::Channel<devpit_rpc::Frame> {
    tauri::ipc::Channel::new(|_| Ok(()))
}

#[test]
fn a_device_reading_a_conversation_is_sent_a_turn_started_elsewhere_and_no_other() {
    let relay = Relay::default();
    let talk = Talk::default();
    let (out, phone) = std::sync::mpsc::channel();
    talk.view("p1", "c1");
    let _watch = relay.watch(talk.watching(out));

    let (turn, _running) = relay.open("c1", crate::chat_relay::Sink::whole(nobody()));
    let (_other, _also) = relay.open("c2", crate::chat_relay::Sink::whole(nobody()));
    assert!(talk.follows("c1"));
    assert!(!talk.follows("c2"));

    turn.send(devpit_rpc::Frame::Session {
        session_id: "s1".to_owned(),
    })
    .unwrap();
    let sent = phone
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the turn reached the device");
    let devpit_rpc::RemoteOut::ChatFrame {
        project,
        conversation,
        frame,
    } = sent
    else {
        panic!("not a chat frame");
    };
    assert_eq!((project.as_str(), conversation.as_str()), ("p1", "c1"));
    assert!(matches!(*frame, devpit_rpc::Frame::Session { session_id } if session_id == "s1"));
}
