//! Each transcript is an ACP conversation: `>` what devpit writes, `<` what
//! the agent says. OpenCode's handshake and Cursor's conversations were
//! recorded from their CLIs; the other agents' halves follow each CLI's
//! documented ACP behaviour. A stand-in plays
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
/// to write before saying what follows it. One that `outlives` its stdin
/// stays after it closes, as Cursor's does.
#[cfg(unix)]
fn stand_in(dir: &Path, played: &[Line], outlives: bool) -> String {
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
    script.push_str(if outlives {
        "exec sleep 600\n"
    } else {
        "while read -r _rest; do :; done\n"
    });
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
    /// The catalogue's agent the driver is started for, its sign-in included.
    base: Option<&'static str>,
    /// The agent stays after its stdin closes.
    outlives_stdin: bool,
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
    let command = stand_in(dir.path(), &played, setup.outlives_stdin);
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
    let driver: Box<dyn crate::driver::Driver> = match setup.base {
        Some(base) => crate::driver::driver_for("acp", base).expect("an ACP agent"),
        None => Box::new(Acp::new(&[])),
    };
    let mut parts = Vec::new();
    let said = (0..5)
        .find_map(|_| {
            let result = crate::talk::say(
                driver.as_ref(),
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
        // A line the stand-in waited for and never got reads as empty.
        .filter(|line| !line.is_empty())
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
        // A line the stand-in waited for and never got reads as empty.
        .filter(|line| !line.is_empty())
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
    resident_with(name, false)
}

/// [`resident`], over a stand-in that stays after its stdin closes when it `outlives`.
#[cfg(unix)]
fn resident_with(
    name: &str,
    outlives: bool,
) -> (
    crate::resident::Resident,
    std::sync::mpsc::Receiver<Spoke>,
    tempfile::TempDir,
) {
    use crate::resident::{Heard, Resident};

    let dir = tempfile::tempdir().expect("tempdir");
    let command = stand_in(dir.path(), &transcript(name), outlives);
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
    assert_eq!(played.asked[0].tool, "Bash", "a shell call, by its command");
    assert_eq!(played.asked[0].input["command"], "ls");
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

/// What the person is asked is keyed by what runs, not by the agent's title:
/// two shell calls titled alike are two commands, so an "Always" on one never
/// allows the other, and a request whose command is unknown asks every time.
#[test]
fn a_permission_prompt_is_keyed_by_the_command_not_the_agents_title() {
    use crate::driver::Driver;

    let acp = Acp::new(&[]);
    let wire = acp.wire();
    let call = |id: &str, kind: &str, raw: Value| {
        let update = serde_json::json!({ "jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "s", "update": { "sessionUpdate": "tool_call", "toolCallId": id,
            "title": "bash", "kind": kind, "status": "pending", "rawInput": raw } } });
        acp.read(&update.to_string());
    };
    let ask = |n: u64, id: &str, extra: Value| {
        let mut tool_call = serde_json::json!({ "toolCallId": id, "title": "bash" });
        if let (Some(into), Some(more)) = (tool_call.as_object_mut(), extra.as_object()) {
            into.extend(more.clone());
        }
        let line =
            serde_json::json!({ "jsonrpc": "2.0", "id": n, "method": "session/request_permission",
            "params": { "sessionId": "s", "toolCall": tool_call,
            "options": [{ "optionId": "once", "name": "Allow once", "kind": "allow_once" }] } })
            .to_string();
        assert!(wire.replies(&line).is_empty(), "it waits for the person");
        wire.asked(&line, Some("s"))
            .and_then(|asked| asked.prompt)
            .expect("asked")
    };
    call(
        "c1",
        "execute",
        serde_json::json!({ "command": "rm -rf build" }),
    );
    call(
        "c2",
        "execute",
        serde_json::json!({ "command": "rm -rf ~" }),
    );
    call("c3", "execute", Value::Null);
    let (one, other) = (ask(1, "c1", Value::Null), ask(2, "c2", Value::Null));
    assert_eq!((one.tool.as_str(), other.tool.as_str()), ("Bash", "Bash"));
    assert_eq!(one.input["command"], "rm -rf build");
    assert_eq!(other.input["command"], "rm -rf ~");
    assert_ne!(one.input["always"], false);
    let unknown = ask(3, "c3", Value::Null);
    assert_eq!(
        unknown.input["always"], false,
        "no command known: asks every time"
    );
    let deleting = ask(4, "c4", serde_json::json!({ "kind": "delete" }));
    assert_eq!(
        deleting.input["always"], false,
        "a kind with no rule asks every time"
    );
    let editing = ask(
        5,
        "c5",
        serde_json::json!({ "kind": "edit", "locations": [{ "path": "/w/a.rs" }] }),
    );
    assert_eq!(editing.tool, "Edit");
    assert_eq!(editing.input["file_path"], "/w/a.rs");
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
    assert!(resident.interrupt(), "and so is a second one");
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

/// An agent that exits after its turn was stopped, before its session opened,
/// ends that turn stopped: a turn ended any other way is sent again.
#[cfg(unix)]
#[test]
fn a_turn_stopped_before_its_agent_exits_ends_stopped() {
    let (resident, said, _dir) = resident("stopped-then-gone.jsonl");
    assert!(resident.say("First"));
    assert!(resident.interrupt());
    assert_eq!(
        until_ended(&said),
        vec![Spoke::Ended(Some("cancelled".to_owned()), false)]
    );
    assert_eq!(until_ended(&said), vec![Spoke::Gone]);
}

/// A turn on an ACP agent, as far as its wire: in `mode`, handed `mcp`.
fn wired(
    mode: &str,
    mcp: Option<&str>,
    session: Option<&str>,
) -> (Acp, Arc<dyn crate::wire::Wire>) {
    let acp = Acp::new(&[]);
    let wire = crate::driver::Driver::wire(&acp);
    let turn = Say {
        command: "opencode",
        env: &[],
        prompt: "",
        cwd: Path::new("/work"),
        model: None,
        budget_usd: None,
        session_id: session,
        fork_at: None,
        permission: Some(mode),
        disallowed: &[],
        settings: None,
        setting_sources: None,
        mcp_config: mcp,
        effort: None,
        control: None,
        on_session: None,
        add_dirs: &[],
        name: None,
    };
    wire.hello(&turn);
    (acp, wire)
}

fn line(text: &str) -> Value {
    serde_json::from_str(text).expect("json")
}

/// OpenCode's answer to `initialize`, as recorded.
const OPENCODE_HELLO: &str = r#"{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"sessionCapabilities":{"resume":{}}}}}"#;
/// OpenCode's session, in its `build` mode with `plan` beside it, as recorded.
const OPENCODE_SESSION: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"sessionId":"ses_1","configOptions":[{"id":"mode","name":"Session Mode","category":"mode","type":"select","currentValue":"build","options":[{"value":"build","name":"build"},{"value":"plan","name":"plan"}]}]}}"#;

/// The session request hands the agent devpit's server, new or resumed.
#[test]
fn an_acp_session_is_opened_with_devpits_tools() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("mcp-opencode.json");
    let server = serde_json::json!({ "mcpServers": { "devpit": {
        "command": "/opt/devpit", "args": ["mcp"], "env": { "DEVPIT_AGENT_ID": "opencode" }, "timeout": 1_260_000,
    } } });
    std::fs::write(&file, server.to_string()).expect("file");
    let handed = serde_json::json!([{
        "name": "devpit", "command": "/opt/devpit", "args": ["mcp"],
        "env": [{ "name": "DEVPIT_AGENT_ID", "value": "opencode" }],
    }]);
    for (session, method) in [(None, "session/new"), (Some("ses_1"), "session/resume")] {
        let (_, wire) = wired("manual", file.to_str(), session);
        let opened = line(&wire.replies(OPENCODE_HELLO)[0]);
        assert_eq!(opened["method"], method);
        assert_eq!(opened["params"]["mcpServers"], handed, "{method}");
    }
    let (_, wire) = wired("manual", None, None);
    assert_eq!(
        line(&wire.replies(OPENCODE_HELLO)[0])["params"]["mcpServers"],
        serde_json::json!([])
    );
}

/// Plan is the agent's own read-only mode, set before the message goes; and
/// out of Plan the session goes back to the mode it opened in.
#[test]
fn plan_puts_the_agent_in_its_own_plan_mode_and_back() {
    let (_, wire) = wired("plan", None, None);
    wire.replies(OPENCODE_HELLO);
    assert!(
        wire.prompt("Look around").is_empty(),
        "held while the session opens"
    );
    let set = line(&wire.replies(OPENCODE_SESSION)[0]);
    assert_eq!(set["method"], "session/set_config_option");
    assert_eq!(
        set["params"],
        serde_json::json!({ "sessionId": "ses_1", "configId": "mode", "value": "plan" })
    );
    let sent = wire.replies(r#"{"jsonrpc":"2.0","id":2,"result":{}}"#);
    assert_eq!(
        line(&sent[0])["method"],
        "session/prompt",
        "then the message"
    );

    let Some(crate::wire::Switch::Write(back)) = wire.control(
        "s-1",
        serde_json::json!({ "subtype": "set_permission_mode", "mode": "manual" }),
    ) else {
        panic!("the agent is asked to leave plan");
    };
    assert_eq!(line(&back)["params"]["value"], "build");
}

/// An agent that reports `modes` rather than a config option takes `session/set_mode`.
#[test]
fn plan_on_an_agent_with_session_modes_sets_its_mode() {
    let (_, wire) = wired("plan", None, None);
    wire.replies(OPENCODE_HELLO);
    let session = r#"{"jsonrpc":"2.0","id":1,"result":{"sessionId":"s","modes":{"currentModeId":"code","availableModes":[{"id":"code","name":"Code"},{"id":"plan","name":"Plan"}]}}}"#;
    let set = line(&wire.replies(session)[0]);
    assert_eq!(set["method"], "session/set_mode");
    assert_eq!(set["params"]["modeId"], "plan");
}

/// With no read-only mode to put the agent in, Plan is refused, saying so,
/// rather than run as a chat that may still write.
#[test]
fn plan_on_an_agent_without_a_read_only_mode_is_refused_saying_why() {
    let (acp, wire) = wired("plan", None, None);
    wire.replies(OPENCODE_HELLO);
    assert!(wire.prompt("Look around").is_empty());
    let session = r#"{"jsonrpc":"2.0","id":1,"result":{"sessionId":"agy-1","configOptions":[{"id":"model","category":"model","currentValue":"agy-pro","options":[{"value":"agy-pro"}]}]}}"#;
    assert!(wire.replies(session).is_empty(), "the message is not sent");
    assert_eq!(
        crate::driver::Driver::read(&acp, session),
        crate::driver::Read::Failed(super::acp_wire::NO_PLAN.to_owned())
    );
    assert!(
        wire.control(
            "s-2",
            serde_json::json!({ "subtype": "set_permission_mode", "mode": "plan" })
        )
        .is_none(),
        "switched to Plan, it takes a new process, whose session says why"
    );
}

/// Cursor: signed in with its own login, the answer streamed, its tool call
/// shown with its result, and its permission prompt put to the person by the
/// command it runs.
#[cfg(unix)]
#[test]
fn a_chat_on_cursor_streams_its_answer_shows_its_tool_and_asks_the_person() {
    let played = play(
        "cursor-approved.jsonl",
        Turn {
            prompt: "Run exactly this shell command: `touch made.txt`. Then reply with the single word: done.",
            permission: Some("acceptEdits"),
            base: Some("cursor"),
            ..Turn::default()
        },
    );
    assert_eq!(
        played.heard, played.expected,
        "devpit's half, sign-in included"
    );
    assert_eq!(text_of(&played.parts), "done.");
    let call = "call-df0e7045-94aa-437b-9fd3-ccd7c4a75892-composer_call_aQUtEHBb\nfc_36df6fa9-3c6d-95a7-9fb4-c06633504ced_0";
    assert!(played.parts.iter().any(|part| matches!(
        part,
        Part::ToolCall { id, input, .. } if id == call && input == r#"{"command":"touch made.txt"}"#
    )));
    assert!(played.parts.iter().any(|part| matches!(
        part,
        Part::ToolResult { call_id, is_error: false, .. } if call_id == call
    )));
    assert_eq!(played.asked.len(), 1, "the person was asked once");
    assert_eq!(played.asked[0].tool, "Bash");
    assert_eq!(played.asked[0].input["command"], "touch made.txt");
    assert_eq!(played.said.end.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(
        played.said.session_id.as_deref(),
        Some("aea49d70-8146-496f-bb07-a5cf17e4a1d6")
    );
}

/// The models a chat on Cursor lists are the ones its session offers.
#[cfg(unix)]
#[test]
fn the_models_a_cursor_chat_offers_come_from_the_agent() {
    let played = play(
        "cursor-approved.jsonl",
        Turn {
            prompt: "Run exactly this shell command: `touch made.txt`. Then reply with the single word: done.",
            permission: Some("acceptEdits"),
            base: Some("cursor"),
            ..Turn::default()
        },
    );
    let init = played.said.init.expect("the session described itself");
    assert_eq!(init.model.as_deref(), Some("default[]"));
    let offered: Vec<(&str, &str)> = init
        .models
        .iter()
        .map(|one| (one.id.as_str(), one.name.as_str()))
        .collect();
    assert_eq!(offered.len(), 18, "every model the session offered");
    assert_eq!(offered[0], ("default[]", "Auto"));
    assert!(offered.contains(&("composer-2.5[fast=false]", "Composer 2.5")));
    assert!(init.models.iter().all(|one| one.unavailable.is_none()));
}

/// A model Cursor refuses is listed as unavailable with the agent's reason,
/// said in the chat, and the turn goes on on the model the agent runs.
#[cfg(unix)]
#[test]
fn a_model_cursor_refuses_is_unavailable_with_its_reason() {
    let played = play(
        "cursor-refused-model.jsonl",
        Turn {
            model: Some("gpt-5"),
            prompt: "Say hi in one word.",
            base: Some("cursor"),
            ..Turn::default()
        },
    );
    assert_eq!(played.heard, played.expected);
    let refused = played
        .said
        .init
        .as_ref()
        .and_then(|init| init.models.iter().find(|one| one.id == "gpt-5"))
        .cloned()
        .expect("the refused model is listed");
    assert_eq!(
        refused.unavailable.as_deref(),
        Some("Invalid model value: gpt-5")
    );
    let said = text_of(&played.parts);
    assert!(said.contains("Invalid model value: gpt-5"), "{said}");
    assert!(said.ends_with("Oi."), "{said}");
    assert!(!played.said.end.is_error);
}

/// Stop cancels a Cursor turn in the protocol's way.
#[cfg(unix)]
#[test]
fn a_chat_on_cursor_stops_on_stop() {
    let played = play(
        "cursor-stopped.jsonl",
        Turn {
            prompt: "Run the shell command `sleep 30`, then say done.",
            permission: Some("acceptEdits"),
            base: Some("cursor"),
            stop_at: Some("call-4f3ce070-b101-434f-8531-e332858bb3d4-composer_call_Edsxhyzu\nfc_517dbe81-1207-98da-9939-f9afd66d3c65_0"),
            ..Turn::default()
        },
    );
    assert_eq!(played.heard, played.expected);
    assert_eq!(played.said.end.stop_reason.as_deref(), Some("cancelled"));
    assert!(!played.said.end.is_error);
}

/// After an idle-out Cursor loads the session: its history is replayed, and
/// only the new answer is this turn's.
#[cfg(unix)]
#[test]
fn a_chat_on_cursor_resumes_after_an_idle_out() {
    let played = play(
        "cursor-resumed.jsonl",
        Turn {
            session: Some("aea49d70-8146-496f-bb07-a5cf17e4a1d6"),
            prompt: "Which file did you create earlier in this conversation? Answer with the file name only.",
            base: Some("cursor"),
            ..Turn::default()
        },
    );
    assert_eq!(played.heard, played.expected);
    assert_eq!(text_of(&played.parts), "made.txt");
    assert_eq!(
        played.said.session_id.as_deref(),
        Some("aea49d70-8146-496f-bb07-a5cf17e4a1d6")
    );
}

/// An agent that stays after its stdin closes, as Cursor's does, is ended
/// once its turn is over rather than waited on forever.
#[cfg(unix)]
#[test]
fn a_turn_on_an_agent_that_outlives_its_stdin_still_ends() {
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let played = play(
            "cursor-refused-model.jsonl",
            Turn {
                model: Some("gpt-5"),
                prompt: "Say hi in one word.",
                base: Some("cursor"),
                outlives_stdin: true,
                ..Turn::default()
            },
        );
        let _ = done.send(played.said.end.stop_reason);
    });
    let ended = finished.recv_timeout(std::time::Duration::from_secs(20));
    assert_eq!(ended.ok().flatten().as_deref(), Some("end_turn"));
}

/// Cursor's CLI is started as its ACP server and handed no credential: the
/// chat signs in with the login the CLI already keeps.
#[test]
fn a_chat_on_cursor_starts_its_acp_server_with_no_credential() {
    let driver = crate::driver::driver_for("acp", "cursor").expect("Cursor holds a chat");
    let dir = tempfile::tempdir().expect("tempdir");
    let turn = Say {
        command: "cursor-agent",
        env: &[],
        prompt: "",
        cwd: dir.path(),
        model: None,
        budget_usd: None,
        session_id: None,
        fork_at: None,
        permission: None,
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
    assert_eq!(driver.wire().argv(&turn), vec!["acp".to_owned()]);
    let agent = devpit_pty::agents::known("cursor").expect("Cursor is known");
    assert_eq!(agent.chat_sign_in, Some("cursor_login"));
}

/// A Cursor chat's process does not outlive its chat, nor devpit.
#[cfg(unix)]
#[test]
fn a_cursor_chat_leaves_no_process_when_it_ends_or_devpit_quits() {
    // Polled in tenths of a second.
    let gone_within = |pid: u32, tenths: u32| {
        (0..tenths).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            !crate::resident::still_running(pid)
        })
    };
    let (ended, _said, _dir) = resident_with("cursor-approved.jsonl", true);
    ended.close();
    assert!(gone_within(ended.pid(), 40), "its chat ended");
    // Quitting ends it at once, well inside the grace a closed chat gets.
    let (quit, _said, _dir) = resident_with("cursor-approved.jsonl", true);
    quit.quit();
    assert!(gone_within(quit.pid(), 10), "devpit quit");
}
