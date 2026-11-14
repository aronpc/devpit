use std::path::Path;
use std::sync::Mutex;

use devpit_agentcli::person::{last_said, From};
use devpit_rpc::{Conversation, Message, Part, Privacy, Role, Transcript, TurnFrom};
use serde_json::json;

use super::{
    final_words, gather, heard, offer, shaped, split, together, which, Desk, Heard, Line,
    LONGEST_MESSAGE, NO_VOICE,
};
use crate::channels::Action;
use crate::remote_talk::MOST_VOICE;
use crate::telegram::{read_update, Kept, Update};

/// The paired chat, as a test stands in for Telegram's API.
struct Chat {
    privacy: Privacy,
    /// A voice note waiting to be fetched.
    voice: Vec<u8>,
    said: Mutex<Vec<(String, Vec<Action>)>>,
    fetched: Mutex<usize>,
}

fn chat(privacy: Privacy) -> Chat {
    Chat {
        privacy,
        voice: Vec::new(),
        said: Mutex::default(),
        fetched: Mutex::default(),
    }
}

impl Line for Chat {
    fn id(&self) -> &'static str {
        "telegram"
    }

    fn privacy(&self) -> Privacy {
        self.privacy
    }

    fn orchestrator(&self) -> Option<String> {
        None
    }

    async fn reply(&self, text: &str, actions: &[Action]) {
        let mut said = self.said.lock().expect("said");
        said.push((text.to_owned(), actions.to_vec()));
    }

    async fn fetch(&self, _file: &str, most: usize) -> Result<Vec<u8>, String> {
        *self.fetched.lock().expect("fetched") += 1;
        let mut held = Vec::new();
        for piece in self.voice.chunks(256 * 1024) {
            gather(&mut held, piece, most)?;
        }
        Ok(held)
    }
}

/// This machine, as a test stands in for the orchestrator's turn.
struct Machine {
    project: String,
    voice: bool,
    answer: String,
    /// What the agent does during its turn, through devpit's own paths.
    during: Box<dyn Fn(&str) + Send + Sync>,
    turns: Mutex<Vec<(String, String, TurnFrom)>>,
}

fn machine(project: &str, answer: &str) -> Machine {
    Machine {
        project: project.to_owned(),
        voice: true,
        answer: answer.to_owned(),
        during: Box::new(|_| {}),
        turns: Mutex::default(),
    }
}

impl Desk for Machine {
    fn orchestrators(&self) -> Vec<String> {
        vec![self.project.clone()]
    }

    fn voice_ready(&self) -> bool {
        self.voice
    }

    async fn hear(&self, ext: &str, bytes: Vec<u8>) -> Result<Transcript, String> {
        assert_eq!(ext, "ogg");
        Ok(Transcript {
            text: Some(String::from_utf8(bytes).expect("words")),
            note: None,
        })
    }

    async fn turn(&self, project: &str, words: &str, from: TurnFrom) -> Result<String, String> {
        (self.during)(project);
        let mut turns = self.turns.lock().expect("turns");
        turns.push((project.to_owned(), words.to_owned(), from));
        Ok(self.answer.clone())
    }
}

fn paired() -> Kept {
    serde_json::from_value(json!({ "token": "t", "bot": "my_devpit_bot", "chatId": 42 }))
        .expect("kept")
}

fn message_from(chat: i64, text: &str) -> serde_json::Value {
    json!({ "update_id": 1, "message": { "message_id": 9, "chat": { "id": chat, "type": "private" }, "from": { "id": chat }, "text": text } })
}

fn voice_from(chat: i64, size: usize) -> serde_json::Value {
    json!({ "update_id": 2, "message": { "message_id": 10, "chat": { "id": chat, "type": "private" }, "voice": { "file_id": "AwACAgEAAxkBAAI", "duration": 4, "mime_type": "audio/ogg", "file_size": size } } })
}

fn press_from(chat: i64, key: &str) -> serde_json::Value {
    json!({ "update_id": 3, "callback_query": { "id": "q1", "from": { "id": chat }, "message": { "message_id": 11, "chat": { "id": chat, "type": "private" } }, "data": format!("a:{key}") } })
}

fn said_in(update: &serde_json::Value) -> Heard {
    match read_update(&paired(), update, 0) {
        Update::Said(said) => said,
        other => panic!("not the person's words: {other:?}"),
    }
}

#[tokio::test]
async fn whats_waiting_arrives_as_the_persons_turn_and_the_answer_goes_back() {
    let line = chat(Privacy::Full);
    let desk = machine(
        "orchestrator-c15-a",
        "Two sessions wait on you: api and web.",
    );
    let said = said_in(&message_from(42, "what's waiting on me?"));
    heard(&line, &desk, said).await;

    let turns = desk.turns.lock().expect("turns");
    let (project, words, from) = &turns[0];
    assert_eq!(project, "orchestrator-c15-a");
    assert_eq!(words, "what's waiting on me?");
    // As the agent is sent it and its log keeps it: the person, on Telegram.
    let conversation = conversation(Vec::new());
    let ask = crate::remote_talk::ask_of(&conversation, None, "/orch", words, Some(from.clone()))
        .expect("a turn");
    let sent = crate::session_notice::notices(ask.from.as_ref(), None, &ask.prompt);
    let log = json!({ "type": "user", "uuid": "u1", "promptSource": "sdk", "message": { "role": "user", "content": sent } });
    let person = last_said(&log.to_string()).expect("the person");
    assert_eq!(person.from, From::Channel("telegram"));
    assert_eq!(person.text, "what's waiting on me?");

    let said = line.said.lock().expect("said");
    assert_eq!(
        *said,
        vec![(
            "Two sessions wait on you: api and web.".to_owned(),
            Vec::new()
        )]
    );
}

#[tokio::test]
async fn telling_a_session_to_go_on_drafts_it_with_a_button_whose_press_sends_it() {
    let line = chat(Privacy::Titles);
    let mut desk = machine("orchestrator-c15-b", "I drafted «go on» for noiseless-mvp.");
    // What the orchestrator does with devpit's tools: drafts, then asks to send.
    desk.during = Box::new(|project| {
        let offered = crate::reply_drafts::draft(project, "prof_c15b", "noiseless-mvp", "go on")
            .expect("drafted");
        assert!(
            offered,
            "a channel's turn puts the draft's button under its answer"
        );
        let asked = crate::draft_sending::send(
            project,
            "prof_c15b",
            Path::new("/no/log/read"),
            "noiseless-mvp",
            false,
        )
        .expect("asked");
        assert_eq!(asked["done"], false);
    });
    heard(
        &line,
        &desk,
        said_in(&message_from(42, "tell noiseless to go on")),
    )
    .await;

    let send = Action::SendDraft {
        profile: "prof_c15b".to_owned(),
        session: "noiseless-mvp".to_owned(),
    };
    let said = line.said.lock().expect("said").clone();
    let (asks, buttons) = said.last().expect("the buttons' message");
    assert_eq!(buttons, &vec![send.clone()]);
    assert!(asks.contains("send the draft to noiseless-mvp"));
    // The words alone sent nothing.
    assert_eq!(
        crate::reply_drafts::drafted("prof_c15b", "noiseless-mvp").as_deref(),
        Some("go on")
    );

    // The press, as Telegram sends it from the paired chat, sends the draft.
    let key = crate::channel_actions::remember(&send);
    let Update::Pressed {
        key: Some(pressed), ..
    } = read_update(&paired(), &press_from(42, &key), 0)
    else {
        panic!("a press in the paired chat");
    };
    let action = crate::channel_actions::take(&pressed).expect("the button's action");
    let Action::SendDraft { profile, session } = &action else {
        panic!("the draft's button");
    };
    let typed = Mutex::new(Vec::new());
    let done =
        crate::channel_actions::sent_draft(profile, session, "Telegram", |p, s, text, from| {
            typed.lock().expect("typed").push((
                p.to_owned(),
                s.to_owned(),
                text.to_owned(),
                from.map(str::to_owned),
            ));
            Ok(())
        });
    assert_eq!(done, Ok("Sent to noiseless-mvp.".to_owned()));
    assert_eq!(
        *typed.lock().expect("typed"),
        vec![(
            "prof_c15b".to_owned(),
            "noiseless-mvp".to_owned(),
            "go on".to_owned(),
            Some("Telegram".to_owned())
        )]
    );
}

fn stop(session: &str, losing: Option<&str>) -> Action {
    Action::StopSession {
        profile: "prof_c15".to_owned(),
        session: session.to_owned(),
        pid: None,
        losing: losing.map(str::to_owned),
    }
}

#[test]
fn outside_a_channels_turn_nothing_waits_for_a_press() {
    // The composer's turn: the orchestrator stops a session as it always has.
    assert!(!offer("orchestrator-c15-c", stop("api", None)));
}

/// N stops asked in one channel turn: a button each, and one that stops them
/// all, under a message that says what each loses.
#[tokio::test]
async fn several_stops_asked_from_a_channel_go_in_one_press() {
    let line = chat(Privacy::Titles);
    let mut desk = machine("orchestrator-c15-d", "Stopping api, web and docs.");
    let busy = "it is working right now";
    desk.during = Box::new(move |project| {
        for one in [
            stop("api", None),
            stop("web", Some(busy)),
            stop("docs", None),
        ] {
            assert!(offer(project, one));
        }
    });
    heard(&line, &desk, said_in(&message_from(42, "pare todas"))).await;

    let said = line.said.lock().expect("said").clone();
    let (asks, buttons) = said.last().expect("the buttons' message");
    let each = vec![
        stop("api", None),
        stop("web", Some(busy)),
        stop("docs", None),
    ];
    let mut every = each.clone();
    every.push(Action::All(each));
    assert_eq!(buttons, &every);
    assert_eq!(crate::channel_actions::label(&every[3]), "Stop all 3");
    assert!(
        asks.contains(&format!("stop web, which loses work: {busy}")),
        "{asks}"
    );
    assert!(!asks.contains("Stop all"), "{asks}");
}

#[test]
fn several_drafts_go_in_one_press_and_a_lone_one_stays_alone() {
    let send = |session: &str| Action::SendDraft {
        profile: "prof_c15".to_owned(),
        session: session.to_owned(),
    };
    let offered = together(vec![send("api"), stop("web", None), send("docs")]);
    assert_eq!(
        offered,
        vec![
            send("api"),
            stop("web", None),
            send("docs"),
            Action::All(vec![send("api"), send("docs")])
        ]
    );
    assert_eq!(crate::channel_actions::label(&offered[3]), "Send all 2");
    assert_eq!(together(vec![send("api")]), vec![send("api")]);
    // Pressed, it does each one, as each one's own button would.
    let done = crate::channel_actions::act(&offered[3], "Telegram");
    assert_eq!(
        done,
        "That draft is no longer waiting.\nThat draft is no longer waiting."
    );
}

#[test]
fn an_unpaired_chat_is_ignored() {
    let kept = paired();
    assert_eq!(
        read_update(&kept, &message_from(7, "what's waiting on me?"), 0),
        Update::Unpaired
    );
    assert_eq!(
        read_update(&kept, &voice_from(7, 4_000), 0),
        Update::Unpaired
    );
    // A button pressed anywhere else names no action.
    assert_eq!(
        read_update(&kept, &press_from(7, "01K"), 0),
        Update::Pressed {
            query: "q1".to_owned(),
            key: None
        }
    );
    // Before any chat is paired, nobody is the person.
    let unlinked: Kept = serde_json::from_value(json!({ "token": "t", "bot": "b" })).expect("kept");
    assert_eq!(
        read_update(&unlinked, &message_from(42, "hi"), 0),
        Update::Unpaired
    );
    // The paired chat is; `/start` again is the bot's, not the orchestrator's.
    assert_eq!(
        read_update(&kept, &message_from(42, "hi"), 0),
        Update::Said(Heard::Text("hi".to_owned()))
    );
    assert_eq!(
        read_update(&kept, &message_from(42, "/start"), 0),
        Update::Nothing
    );
}

#[tokio::test]
async fn an_oversize_voice_note_is_refused_before_it_reaches_anyone() {
    // Said too big by Telegram: refused before a byte is fetched.
    let line = chat(Privacy::Full);
    let desk = machine("orchestrator-c15-d", "unused");
    heard(&line, &desk, said_in(&voice_from(42, MOST_VOICE + 1))).await;
    assert_eq!(*line.fetched.lock().expect("fetched"), 0);
    // Said small, and bigger as it arrives: let go once past the most.
    let mut lying = chat(Privacy::Full);
    lying.voice = vec![b'a'; MOST_VOICE + 1];
    heard(&lying, &desk, said_in(&voice_from(42, 4_000))).await;
    assert_eq!(*lying.fetched.lock().expect("fetched"), 1);

    assert!(desk.turns.lock().expect("turns").is_empty());
    for line in [&line, &lying] {
        let said = line.said.lock().expect("said");
        assert!(said[0].0.contains("past the 3 MB"), "{}", said[0].0);
    }
    let mut held = Vec::new();
    assert!(gather(&mut held, &[0; 10], 10).is_ok());
    assert!(gather(&mut held, &[0; 1], 10).is_err());
    assert!(held.is_empty());
}

#[tokio::test]
async fn a_voice_note_is_heard_here_and_enters_as_text() {
    let mut line = chat(Privacy::Full);
    line.voice = b"remind me at 3pm to deploy".to_vec();
    let desk = machine("orchestrator-c15-e", "Done: a reminder at 15:00.");
    heard(&line, &desk, said_in(&voice_from(42, 26))).await;
    let words = desk.turns.lock().expect("turns")[0].1.clone();
    assert_eq!(words, "remind me at 3pm to deploy");

    // No engine: the channel is told how to get one, and nothing runs.
    let mut deaf = machine("orchestrator-c15-e", "unused");
    deaf.voice = false;
    let line = chat(Privacy::Full);
    heard(&line, &deaf, said_in(&voice_from(42, 26))).await;
    assert_eq!(line.said.lock().expect("said")[0].0, NO_VOICE);
    assert_eq!(*line.fetched.lock().expect("fetched"), 0);
    assert!(deaf.turns.lock().expect("turns").is_empty());
}

#[tokio::test]
async fn the_privacy_level_shapes_the_answer() {
    let answer = "api and web wait on you.\n\napi asks to run the migration; web is idle.";
    assert_eq!(
        shaped(answer, Privacy::Minimal),
        vec!["The orchestrator answered: read it in devpit."]
    );
    assert_eq!(
        shaped(answer, Privacy::Titles),
        vec!["api and web wait on you.…"]
    );
    assert_eq!(shaped(answer, Privacy::Full), vec![answer]);
    let long = "word ".repeat(LONGEST_MESSAGE / 2);
    let pieces = shaped(&long, Privacy::Full);
    assert_eq!(pieces.len(), 3);
    assert!(pieces
        .iter()
        .all(|piece| piece.chars().count() <= LONGEST_MESSAGE));
    assert_eq!(
        split("one two three", 8),
        vec!["one two".to_owned(), "three".to_owned()]
    );

    // A button's line names nothing when the person asked for nothing named.
    let line = chat(Privacy::Minimal);
    let mut desk = machine("orchestrator-c15-f", answer);
    desk.during = Box::new(|project| {
        crate::reply_drafts::draft(project, "prof_c15f", "noiseless-mvp", "go on")
            .expect("drafted");
    });
    heard(
        &line,
        &desk,
        Heard::Text("tell noiseless to go on".to_owned()),
    )
    .await;
    let said = line.said.lock().expect("said");
    assert_eq!(said[0].0, "The orchestrator answered: read it in devpit.");
    assert_eq!(said[1].0, "Waiting for your press:\n• send a draft");
    assert!(said.iter().all(|(text, _)| !text.contains("noiseless")));
}

fn conversation(messages: Vec<Message>) -> Conversation {
    Conversation {
        id: "c".to_owned(),
        project_id: "orchestrator-c15".to_owned(),
        card_id: None,
        card_title: None,
        card_on_board: false,
        profile: "claude-work".to_owned(),
        model: None,
        permission: None,
        effort: None,
        cwd: None,
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
fn only_the_words_a_turn_ended_on_go_back() {
    let text = |said: &str, parent: Option<&str>| Part::Text {
        text: said.to_owned(),
        parent: parent.map(str::to_owned),
    };
    let answer = Message {
        id: "m2".to_owned(),
        turn_id: Some("turn_1".to_owned()),
        role: Role::Assistant,
        parts: vec![
            text("Let me look at the sessions.", None),
            Part::ToolCall {
                id: "call_1".to_owned(),
                name: "devpit_sessions".to_owned(),
                input: "{}".to_owned(),
                state: devpit_rpc::CallState::Ok,
                parent: None,
            },
            Part::ToolResult {
                call_id: "call_1".to_owned(),
                output: "[…]".to_owned(),
                is_error: false,
                parent: None,
            },
            text("Two wait on you:", None),
            text("a subagent's aside", Some("call_9")),
            text("api and web.", None),
        ],
        created_at: 0.0,
        streaming: false,
        ended: None,
    };
    let read = conversation(vec![answer]);
    assert_eq!(
        final_words(&read, "turn_1"),
        "Two wait on you:\n\napi and web."
    );
    assert_eq!(final_words(&read, "turn_0"), "");
}

#[test]
fn a_channel_talks_to_the_orchestrator_chosen_or_the_only_one() {
    let two = ["orch-a".to_owned(), "orch-b".to_owned()];
    assert_eq!(which(&two, Some("orch-b")), Ok("orch-b".to_owned()));
    assert!(which(&two, None).is_err());
    assert!(which(&two, Some("gone")).is_err());
    assert_eq!(which(&two[..1], None), Ok("orch-a".to_owned()));
    assert!(which(&[], None).is_err());
}

/// After a switch, what devpit says to an orchestrator goes to its newest
/// conversation on the account it speaks as now, never one begun on the old.
#[test]
fn devpits_turns_go_to_the_conversation_on_the_account_spoken_as_now() {
    let thread = |id: &str, profile: &str| devpit_rpc::Thread {
        id: id.to_owned(),
        title: String::new(),
        profile: profile.to_owned(),
        model: None,
        cost_usd: 0.0,
        last_at: 0.0,
        cwd: None,
        marks: Default::default(),
    };
    let newest_first = vec![
        thread("c3", "claude"),
        thread("c2", "codex"),
        thread("c1", "claude"),
    ];
    assert_eq!(
        super::latest_on(newest_first.clone(), Some("codex")).as_deref(),
        Some("c2")
    );
    assert_eq!(
        super::latest_on(newest_first.clone(), Some("claude")).as_deref(),
        Some("c3")
    );
    assert_eq!(super::latest_on(newest_first, Some("opencode")), None);
}
