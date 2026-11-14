//! Each transcript is an ACP conversation: `>` what devpit writes, `<` what
//! the agent says. OpenCode's handshake was recorded from its CLI; the other
//! agents' halves follow each CLI's documented ACP behaviour. A stand-in plays
//! the agent's half line for line, and the test holds devpit to the other.

use std::path::Path;
use std::sync::{Arc, Mutex};

use devpit_rpc::{CallState, Context, Part};
use serde_json::Value;

use super::acp_wire::allowed;
use super::{too_old, Acp};
use crate::control::{Control, Prompt};
use crate::talk::{Said, Say};

/// One line of a transcript: who says it, what, and how long the agent
/// waits first. `exit` ends the agent there.
struct Line {
    dir: String,
    msg: Value,
    wait: f64,
}

fn transcript(name: &str) -> Vec<Line> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/acp")
        .join(name);
    std::fs::read_to_string(&path)
        .expect("transcript")
        .lines()
        .map(|line| {
            let entry: Value = serde_json::from_str(line).expect("json");
            Line {
                dir: entry["dir"].as_str().expect("dir").to_owned(),
                msg: entry["msg"].clone(),
                wait: entry["wait"].as_f64().unwrap_or_default(),
            }
        })
        .collect()
}

/// What devpit wrote, with the folder and version the transcript leaves open.
fn filled(msg: &Value, cwd: &Path) -> Value {
    let text = msg
        .to_string()
        .replace("{cwd}", &cwd.display().to_string())
        .replace("{version}", env!("CARGO_PKG_VERSION"));
    serde_json::from_str(&text).expect("json")
}

/// A shell script that plays the agent's half: it reads each line devpit is
/// to write before saying what follows it.
#[cfg(unix)]
fn stand_in(dir: &Path, played: &[Line]) -> String {
    use std::os::unix::fs::PermissionsExt;

    let heard = dir.join("heard");
    let mut script = String::from("#!/bin/sh\n");
    for line in played {
        if line.wait > 0.0 {
            script.push_str(&format!("sleep {}\n", line.wait));
        }
        match line.dir.as_str() {
            ">" => script.push_str(&format!(
                "read -r line; printf '%s\\n' \"$line\" >> '{}'\n",
                heard.display()
            )),
            "exit" => script.push_str("exit 0\n"),
            _ => {
                let said = line.msg.to_string().replace('\'', "'\\''");
                script.push_str(&format!("printf '%s\\n' '{said}'\n"));
            }
        }
    }
    script.push_str("while read -r _rest; do :; done\n");
    let cli = dir.join("agent");
    std::fs::write(&cli, script).expect("script");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    cli.to_string_lossy().into_owned()
}

/// How a test sets its turn up.
#[derive(Default)]
struct Turn {
    model: Option<&'static str>,
    session: Option<&'static str>,
    permission: Option<&'static str>,
    prompt: &'static str,
    /// Stops the turn when the agent starts this tool call.
    stop_at: Option<&'static str>,
}

struct Played {
    said: Said,
    parts: Vec<Part>,
    asked: Vec<Prompt>,
    heard: Vec<Value>,
    expected: Vec<Value>,
}

#[cfg(unix)]
fn play(name: &str, setup: Turn) -> Played {
    let dir = tempfile::tempdir().expect("tempdir");
    let played = transcript(name);
    let command = stand_in(dir.path(), &played);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let control = Control::new();
    let noted = asked.clone();
    // The person, answering a beat later from another thread.
    control.asks_with(Arc::new(move |prompt: Prompt, control: &Control| {
        let (id, control) = (prompt.request_id.clone(), control.clone());
        noted.lock().expect("asked").push(prompt);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            control.answer(&id, serde_json::json!({ "behavior": "allow" }));
        });
        true
    }));
    let turn = Say {
        command: &command,
        env: &[],
        prompt: setup.prompt,
        cwd: dir.path(),
        model: setup.model,
        budget_usd: None,
        session_id: setup.session,
        fork_at: None,
        permission: setup.permission,
        disallowed: &[],
        settings: None,
        setting_sources: None,
        mcp_config: None,
        effort: None,
        control: Some(&control),
        on_session: None,
        add_dirs: &[],
        name: None,
    };
    let mut parts = Vec::new();
    let said = (0..5)
        .find_map(|_| {
            let result = crate::talk::say(
                &Acp::new(&[]),
                &turn,
                |part| {
                    if let Part::ToolCall { id, .. } = &part {
                        if setup.stop_at == Some(id.as_str()) {
                            assert!(control.interrupt(), "a running turn takes a stop");
                        }
                    }
                    parts.push(part);
                },
                |_| {},
            );
            match result {
                Err(crate::AgentError::NotInstalled) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    None
                }
                other => Some(other),
            }
        })
        .expect("the stand-in never started")
        .expect("a turn");
    let heard = std::fs::read_to_string(dir.path().join("heard"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("devpit writes json"))
        .collect();
    let expected = expected(&played, dir.path());
    let asked = std::mem::take(&mut *asked.lock().expect("asked"));
    Played {
        said,
        parts,
        asked,
        heard,
        expected,
    }
}

fn expected(played: &[Line], cwd: &Path) -> Vec<Value> {
    played
        .iter()
        .filter(|line| line.dir == ">")
        .map(|line| filled(&line.msg, cwd))
        .collect()
}

fn heard_in(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join("heard"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("devpit writes json"))
        .collect()
}

/// What a resident process said, in the order it said it.
#[derive(Debug, PartialEq)]
enum Spoke {
    Text(String),
    Ended(Option<String>, bool),
    Gone,
}

/// A resident process over a transcript, the way every chat outside
/// Supervised holds its conversation.
#[cfg(unix)]
fn resident(
    name: &str,
) -> (
    crate::resident::Resident,
    std::sync::mpsc::Receiver<Spoke>,
    tempfile::TempDir,
) {
    use crate::resident::{Heard, Resident};

    let dir = tempfile::tempdir().expect("tempdir");
    let command = stand_in(dir.path(), &transcript(name));
    let (tell, said) = std::sync::mpsc::channel();
    let turn = Say {
        command: &command,
        env: &[],
        prompt: "",
        cwd: dir.path(),
        model: None,
        budget_usd: None,
        session_id: None,
        fork_at: None,
        permission: Some("acceptEdits"),
        disallowed: &[],
        settings: None,
        setting_sources: None,
        mcp_config: None,
        effort: None,
        control: None,
        on_session: None,
        add_dirs: &[],
        name: None,
    };
    let resident = (0..5)
        .find_map(|_| {
            let tell = tell.clone();
            let started = Resident::start(
                Box::new(Acp::new(&[])),
                &turn,
                Arc::new(|_: &str| {}),
                move |heard| {
                    let said = match heard {
                        Heard::Part(Part::Text { text, .. }) => Spoke::Text(text),
                        Heard::Ended(said) => Spoke::Ended(said.end.stop_reason, said.end.is_error),
                        Heard::Gone => Spoke::Gone,
                        _ => return,
                    };
                    let _ = tell.send(said);
                },
            );
            match started {
                Err(crate::AgentError::NotInstalled) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    None
                }
                other => Some(other.expect("started")),
            }
        })
        .expect("the stand-in started within five tries");
    (resident, said, dir)
}

/// What the resident says until its turn ends, or it is gone.
#[cfg(unix)]
fn until_ended(said: &std::sync::mpsc::Receiver<Spoke>) -> Vec<Spoke> {
    let mut all = Vec::new();
    while let Ok(one) = said.recv_timeout(std::time::Duration::from_secs(5)) {
        let over = matches!(one, Spoke::Ended(..) | Spoke::Gone);
        all.push(one);
        if over {
            break;
        }
    }
    all
}

fn text_of(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// OpenCode: greeted, a session with the model asked for, the answer
/// streamed, its tool call shown with its result, and its permission prompt
/// put to the person in a mode that asks.
#[cfg(unix)]
#[test]
fn a_chat_on_opencode_streams_its_answer_and_tools_and_asks_the_person() {
    let played = play(
        "opencode-turn.jsonl",
        Turn {
            model: Some("opencode/quick"),
            permission: Some("acceptEdits"),
            prompt: "List the files here.",
            ..Turn::default()
        },
    );
    assert_eq!(
        played.heard, played.expected,
        "devpit's half of the conversation"
    );
    assert_eq!(text_of(&played.parts), "It holds notes.txt.");
    assert!(played.parts.iter().any(|part| matches!(
        part,
        Part::ToolCall { id, name, input, state: CallState::Running, .. }
            if id == "call_1" && name == "execute" && input == r#"{"command":"ls"}"#
    )));
    assert!(played.parts.iter().any(|part| matches!(
        part,
        Part::ToolResult { call_id, output, is_error: false, .. } if call_id == "call_1" && output == "notes.txt"
    )));
    assert!(played
        .parts
        .iter()
        .any(|part| matches!(part, Part::Thinking { text, .. } if text == "Listing the folder.")));
    assert_eq!(played.asked.len(), 1, "the person was asked once");
    assert_eq!(played.asked[0].tool, "ls");
    assert_eq!(played.asked[0].input["kind"], "execute");
    assert_eq!(
        played.said.session_id.as_deref(),
        Some("ses_e253a9d75ffejkRP4tKUxLPGEa")
    );
    assert_eq!(
        played.said.init.and_then(|init| init.model).as_deref(),
        Some("opencode/quick")
    );
    assert_eq!(played.said.end.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(played.said.end.cost_usd, Some(0.0012));
    assert_eq!(
        played.said.end.context,
        Some(Context {
            used: 18000,
            window: 1_000_000
        })
    );
}

/// In Full access the same prompt is answered as the mode says, and nobody is asked.
#[cfg(unix)]
#[test]
fn full_access_answers_a_permission_prompt_without_the_person() {
    let played = play(
        "opencode-turn.jsonl",
        Turn {
            model: Some("opencode/quick"),
            permission: Some("bypassPermissions"),
            prompt: "List the files here.",
            ..Turn::default()
        },
    );
    assert_eq!(played.heard, played.expected);
    assert!(played.asked.is_empty(), "nobody was asked");
}

#[test]
fn a_mode_answers_only_what_it_allows() {
    assert!(allowed(Some("bypassPermissions"), "execute"));
    assert!(allowed(Some("acceptEdits"), "edit"));
    assert!(!allowed(Some("acceptEdits"), "execute"));
    assert!(!allowed(Some("manual"), "edit"));
    assert!(!allowed(Some("plan"), "delete"));
    assert!(allowed(Some("manual"), "read"), "a read changes nothing");
    assert!(!allowed(None, "other"));
}

/// Grok: the models it reports name the one the turn ran on, and Stop
/// cancels the turn in the protocol's way.
#[cfg(unix)]
#[test]
fn a_chat_on_grok_stops_on_stop() {
    let played = play(
        "grok-stopped.jsonl",
        Turn {
            prompt: "Wait for the build.",
            permission: Some("acceptEdits"),
            stop_at: Some("call_9"),
            ..Turn::default()
        },
    );
    assert_eq!(played.heard, played.expected);
    assert_eq!(played.said.end.stop_reason.as_deref(), Some("cancelled"));
    assert!(!played.said.end.is_error);
    assert_eq!(
        played.said.init.and_then(|init| init.model).as_deref(),
        Some("grok-code-fast-1")
    );
    assert!(played.parts.iter().any(|part| matches!(
        part,
        Part::ToolResult { call_id, is_error: true, .. } if call_id == "call_9"
    )));
}

/// Antigravity: after an idle-out the session is resumed, not replayed.
#[cfg(unix)]
#[test]
fn a_chat_on_antigravity_resumes_after_an_idle_out() {
    let played = play(
        "antigravity-resumed.jsonl",
        Turn {
            session: Some("agy-0c41"),
            prompt: "And now?",
            ..Turn::default()
        },
    );
    assert_eq!(played.heard, played.expected);
    assert_eq!(text_of(&played.parts), "Picking up where we left off.");
    assert_eq!(played.said.session_id.as_deref(), Some("agy-0c41"));
    // Resumed: what the session spent before this process is not known.
    assert_eq!(played.said.end.cost_usd, None);
    assert_eq!(played.said.end.context, None);
}

#[cfg(unix)]
#[test]
fn an_agent_that_cannot_resume_says_so_and_starts_over() {
    let played = play(
        "no-resume.jsonl",
        Turn {
            session: Some("gone-1"),
            prompt: "And now?",
            ..Turn::default()
        },
    );
    assert_eq!(played.heard, played.expected);
    let said = text_of(&played.parts);
    assert!(said.contains("does not offer to resume"), "{said}");
    assert!(said.ends_with("Hello."));
    assert_eq!(played.said.session_id.as_deref(), Some("fresh-1"));
}

/// A loaded session replays its history first: none of it is this turn's answer.
#[cfg(unix)]
#[test]
fn a_loaded_sessions_history_is_not_the_answer() {
    let played = play(
        "loads-history.jsonl",
        Turn {
            session: Some("loaded-2"),
            prompt: "And now?",
            ..Turn::default()
        },
    );
    assert_eq!(played.heard, played.expected);
    assert_eq!(text_of(&played.parts), "New answer.");
    // Its cost when loaded is where this process starts counting.
    let spent = played.said.end.cost_usd.expect("priced");
    assert!((spent - 0.02).abs() < 1e-9, "{spent}");
}

#[cfg(unix)]
#[test]
fn an_agent_that_refuses_the_session_ends_the_turn_saying_why() {
    let played = play(
        "signed-out.jsonl",
        Turn {
            prompt: "Hi",
            ..Turn::default()
        },
    );
    assert_eq!(played.heard, played.expected);
    assert!(played.said.end.is_error);
    assert!(text_of(&played.parts).contains("Authentication required"));
}

#[test]
fn a_cli_older_than_the_first_that_serves_acp_is_told_to_update() {
    let refused = too_old("Antigravity", "1.5.0", "agy 1.4.2\n").expect("refused");
    assert!(
        refused.starts_with("Update the Antigravity CLI"),
        "{refused}"
    );
    assert!(refused.contains("1.4.2"));
    assert_eq!(too_old("Antigravity", "1.5.0", "1.5.0"), None);
    assert_eq!(
        too_old("Antigravity", "1.5.0", "agy version 1.12.3 (abc)"),
        None
    );
    assert_eq!(
        too_old("Antigravity", "1.5.0", "unknown"),
        None,
        "no version, no refusal"
    );
}

/// Stop while the session still opens: the held message is never sent, the
/// turn ends stopped, and the same process takes the next message.
#[cfg(unix)]
#[test]
fn stop_while_the_session_opens_sends_nothing_and_the_process_goes_on() {
    let (resident, said, dir) = resident("resident-stopped-early.jsonl");
    assert!(resident.say("First"));
    assert!(
        resident.interrupt(),
        "a stop is taken before the session opens"
    );
    assert_eq!(
        until_ended(&said),
        vec![Spoke::Ended(Some("cancelled".to_owned()), false)]
    );
    assert!(resident.say("Next"));
    assert_eq!(
        until_ended(&said),
        vec![
            Spoke::Text("Ready.".to_owned()),
            Spoke::Ended(Some("end_turn".to_owned()), false)
        ]
    );
    assert_eq!(
        heard_in(dir.path()),
        expected(&transcript("resident-stopped-early.jsonl"), dir.path())
    );
    resident.close();
}

/// A session refused while signed out opens on the next message.
#[cfg(unix)]
#[test]
fn a_refused_session_opens_on_the_next_message() {
    let (resident, said, dir) = resident("resident-signed-in-later.jsonl");
    assert!(resident.say("Hi"));
    let first = until_ended(&said);
    assert!(
        matches!(first.last(), Some(Spoke::Ended(_, true))),
        "{first:?}"
    );
    assert!(resident.say("Again"));
    assert_eq!(
        until_ended(&said),
        vec![
            Spoke::Text("Signed in now.".to_owned()),
            Spoke::Ended(Some("end_turn".to_owned()), false)
        ]
    );
    assert_eq!(
        heard_in(dir.path()),
        expected(&transcript("resident-signed-in-later.jsonl"), dir.path())
    );
    resident.close();
}

/// An agent that is gone takes no message, so the chat starts another.
#[cfg(unix)]
#[test]
fn a_process_gone_before_its_session_opened_takes_no_message() {
    let (resident, said, _dir) = resident("gone-in-handshake.jsonl");
    assert_eq!(until_ended(&said), vec![Spoke::Gone]);
    assert!(!resident.say("Hi"));
}
