use std::sync::Arc;

use serde_json::{json, Value};

use super::{Asked, Claude, Switch, Wire};
use crate::control::{interrupt_request, refusal, steer_line, stop_task_request, user_line};
use crate::driver::{Driver, Read};

const ASKED: &str = r#"{"type":"control_request","request_id":"r-1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"ls"}}}"#;

/// Claude Code is written to exactly as before the wire existed.
#[test]
fn claudes_wire_writes_the_lines_stream_json_takes() {
    let wire = Claude;
    assert_eq!(wire.prompt("hi"), vec![user_line("hi")]);
    assert_eq!(wire.steer("more", "u-1"), Some(steer_line("more", "u-1")));
    assert_eq!(wire.stop_task("t-1"), Some(stop_task_request("t-1")));
    let stops = wire.interrupt();
    assert_eq!(stops.len(), 1, "one control request stops the turn");
    let interrupt: Value = serde_json::from_str(&stops[0]).expect("json");
    let before: Value = serde_json::from_str(&interrupt_request()).expect("json");
    assert_eq!(interrupt, before);
    assert!(wire.replies(ASKED).is_empty(), "no handshake to answer");
}

#[test]
fn claudes_wire_reads_its_prompt_and_refuses_it_in_its_own_words() {
    let Asked {
        prompt,
        refusal: refused,
    } = Claude.asked(ASKED, Some("s-1")).expect("a prompt");
    let prompt = prompt.expect("readable");
    assert_eq!(
        (prompt.request_id.as_str(), prompt.tool.as_str()),
        ("r-1", "Bash")
    );
    assert_eq!(prompt.session_id.as_deref(), Some("s-1"));
    assert_eq!(Some(refused), refusal(ASKED));
    assert!(Claude.asked(r#"{"type":"assistant"}"#, None).is_none());
}

#[test]
fn claudes_wire_answers_and_switches_in_control_frames() {
    let answer: Value = serde_json::from_str(
        &Claude
            .answer("r-1", json!({ "behavior": "allow" }))
            .expect("an answer"),
    )
    .expect("json");
    assert_eq!(answer["type"], "control_response");
    assert_eq!(answer["response"]["request_id"], "r-1");
    assert_eq!(answer["response"]["response"]["behavior"], "allow");

    let Some(Switch::Write(line)) = Claude.control(
        "switch_1",
        json!({ "subtype": "set_model", "model": "opus" }),
    ) else {
        panic!("Claude Code takes a switch on its stdin");
    };
    let switch: Value = serde_json::from_str(&line).expect("json");
    assert_eq!(switch["type"], "control_request");
    assert_eq!(switch["request_id"], "switch_1");
    assert_eq!(switch["request"]["model"], "opus");

    let said =
        r#"{"type":"control_response","response":{"subtype":"success","request_id":"switch_1"}}"#;
    assert_eq!(
        Claude.control_answer(said),
        Some(json!({ "subtype": "success", "request_id": "switch_1" }))
    );
    assert_eq!(Claude.control_answer(ASKED), None);
}

/// A CLI with a handshake: it is greeted before the prompt, answers `ping`
/// with `pong`, and asks permission in a shape of its own.
struct Greeting;

impl Wire for Greeting {
    fn argv(&self, _turn: &crate::talk::Say<'_>) -> Vec<String> {
        Vec::new()
    }

    fn hello(&self, _turn: &crate::talk::Say<'_>) -> Vec<String> {
        vec!["hello".to_owned()]
    }

    fn prompt(&self, text: &str) -> Vec<String> {
        vec![format!("say {text}")]
    }

    fn interrupt(&self) -> Vec<String> {
        Vec::new()
    }

    fn asked(&self, line: &str, session_id: Option<&str>) -> Option<Asked> {
        let tool = line.strip_prefix("may I ")?;
        Some(Asked {
            prompt: Some(crate::control::Prompt {
                request_id: "q-1".to_owned(),
                tool: tool.to_owned(),
                input: Value::Null,
                session_id: session_id.map(str::to_owned),
            }),
            refusal: "no".to_owned(),
        })
    }

    fn answer(&self, _request_id: &str, response: Value) -> Option<String> {
        response["behavior"].as_str().map(str::to_owned)
    }

    /// The mode is devpit's own; the CLI never hears of it.
    fn control(&self, _id: &str, request: Value) -> Option<Switch> {
        (request["subtype"] == "set_permission_mode").then_some(Switch::Kept)
    }

    fn control_answer(&self, _line: &str) -> Option<Value> {
        None
    }

    fn replies(&self, line: &str) -> Vec<String> {
        if line == "ping" {
            return vec!["pong".to_owned()];
        }
        Vec::new()
    }
}

/// Reads Claude Code's lines, and writes through [`Greeting`].
#[derive(Default)]
struct Greeted(crate::driver::Claude);

impl Driver for Greeted {
    fn name(&self) -> &'static str {
        "greeted"
    }

    fn wire(&self) -> Arc<dyn Wire> {
        Arc::new(Greeting)
    }

    fn models(&self) -> &'static [&'static str] {
        &[]
    }

    fn read(&self, line: &str) -> Read {
        self.0.read(line)
    }

    fn session(&self, line: &str) -> Option<String> {
        self.0.session(line)
    }
}

/// The turn is written through the driver's own wire, whatever it speaks:
/// greeted first, then the message, a reply to what the CLI asked of devpit,
/// and a permission answered in its shape.
#[cfg(unix)]
#[test]
fn a_turn_is_written_through_its_drivers_wire() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let heard = dir.path().join("heard");
    let cli = dir.path().join("greeted");
    std::fs::write(
        &cli,
        format!(
            "#!/bin/sh\n\
             read -r hello; read -r said\n\
             echo ping; read -r pong\n\
             echo 'may I Bash'; read -r allowed\n\
             printf '%s\\n' \"$hello\" \"$said\" \"$pong\" \"$allowed\" > '{}'\n\
             echo '{{\"type\":\"result\",\"subtype\":\"success\",\"session_id\":\"s-1\"}}'\n",
            heard.display()
        ),
    )
    .expect("script");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let command = cli.to_string_lossy().into_owned();
    let control = crate::control::Control::new();
    control.asks_with(Arc::new(
        |prompt: crate::control::Prompt, control: &crate::control::Control| {
            control.answer(&prompt.request_id, json!({ "behavior": "allow" }))
        },
    ));
    let turn = crate::talk::Say {
        command: &command,
        env: &[],
        prompt: "hi",
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
        control: Some(&control),
        on_session: None,
        add_dirs: &[],
        name: None,
    };
    let said = (0..5)
        .find_map(
            |_| match crate::talk::say(&Greeted::default(), &turn, |_| {}, |_| {}) {
                Err(crate::AgentError::NotInstalled) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    None
                }
                other => Some(other),
            },
        )
        .expect("the stand-in never started")
        .expect("a turn");
    assert_eq!(said.end.stop_reason.as_deref(), Some("success"));
    assert_eq!(
        std::fs::read_to_string(&heard).expect("heard"),
        "hello\nsay hi\npong\nallow\n"
    );
}

/// A switch devpit keeps itself is answered at once, the way the CLI answers
/// one it took, so the turn waiting on it goes on without a new process.
#[cfg(unix)]
#[test]
fn a_switch_devpit_keeps_is_answered_without_the_cli() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let cli = dir.path().join("greeted");
    std::fs::write(&cli, "#!/bin/sh\nwhile read -r _line; do :; done\n").expect("script");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let command = cli.to_string_lossy().into_owned();
    let turn = crate::talk::Say {
        command: &command,
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
    let (tell, heard) = std::sync::mpsc::channel();
    let resident = (0..5)
        .find_map(|_| {
            let tell = tell.clone();
            match crate::resident::Resident::start(
                Box::<Greeted>::default(),
                &turn,
                Arc::new(|_: &str| {}),
                move |one| {
                    if let crate::resident::Heard::Control(said) = one {
                        let _ = tell.send(said);
                    }
                },
            ) {
                Err(crate::AgentError::NotInstalled) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    None
                }
                other => Some(other.expect("started")),
            }
        })
        .expect("the stand-in started within five tries");

    let mode = json!({ "subtype": "set_permission_mode", "mode": "plan" });
    assert!(resident.control("switch_1", mode));
    assert_eq!(
        heard
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("answered"),
        json!({ "subtype": "success", "request_id": "switch_1" })
    );
    let model = json!({ "subtype": "set_model", "model": "opus" });
    assert!(
        !resident.control("switch_2", model),
        "one it cannot take asks for a new process"
    );
    resident.close();
}
