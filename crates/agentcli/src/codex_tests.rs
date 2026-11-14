//! Each transcript is a real `codex app-server` 0.160 conversation, recorded
//! through this wire: `>` what devpit wrote, `<` what Codex said. A stand-in
//! plays Codex's half line for line, and the test holds devpit to the other.

use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use devpit_rpc::{CallState, Context, Part};
use serde_json::{json, Value};

use super::codex_wire::{allowed, policy};
use super::{held, Codex};
use crate::control::{Control, Prompt};
use crate::driver::{Driver, Read};
use crate::resident::{Heard, Resident};
use crate::talk::{Said, Say};
use crate::wire::Switch;

const THREAD: &str = "01a1db4f-6c27-7970-b4a2-7d437ed52823";
const WINDOW: u32 = 475_000;

struct Line {
    dir: String,
    msg: Value,
}

fn transcript(name: &str) -> Vec<Line> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/codex")
        .join(format!("codex-0.160-{name}.jsonl"));
    std::fs::read_to_string(path)
        .expect("transcript")
        .lines()
        .map(|line| {
            let entry: Value = serde_json::from_str(line).expect("json");
            Line {
                dir: entry["dir"].as_str().expect("dir").to_owned(),
                msg: entry["msg"].clone(),
            }
        })
        .collect()
}

/// A line with the folder, home and version the transcript leaves open.
fn filled(msg: &Value, dir: &Path) -> Value {
    let text = msg
        .to_string()
        .replace("{cwd}", &dir.display().to_string())
        .replace("{home}", &dir.join("home").display().to_string())
        .replace("{version}", env!("CARGO_PKG_VERSION"));
    serde_json::from_str(&text).expect("json")
}

/// A shell script that plays Codex's half: it reads each line devpit is to
/// write before saying what follows, and notes the `CODEX_HOME` it was given.
#[cfg(unix)]
fn stand_in(dir: &Path, played: &[Line]) -> String {
    use std::os::unix::fs::PermissionsExt;

    let heard = dir.join("heard");
    let mut script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$CODEX_HOME\" > '{}'\n",
        dir.join("home-seen").display()
    );
    for line in played {
        if line.dir == ">" {
            script.push_str(&format!(
                "read -r line; printf '%s\\n' \"$line\" >> '{}'\n",
                heard.display()
            ));
        } else {
            let said = filled(&line.msg, dir).to_string().replace('\'', "'\\''");
            script.push_str(&format!("printf '%s\\n' '{said}'\n"));
        }
    }
    script.push_str("while read -r _rest; do :; done\n");
    let cli = dir.join("codex");
    std::fs::write(&cli, script).expect("script");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    cli.to_string_lossy().into_owned()
}

/// What devpit wrote, and what the transcript says it should have.
fn compared(dir: &Path, played: &[Line]) -> (Vec<Value>, Vec<Value>) {
    let heard = std::fs::read_to_string(dir.join("heard"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("devpit writes json"))
        .collect();
    let expected = played
        .iter()
        .filter(|line| line.dir == ">")
        .map(|line| filled(&line.msg, dir))
        .collect();
    (heard, expected)
}

fn turn<'a>(
    command: &'a str,
    env: &'a [(String, String)],
    dir: &'a Path,
    prompt: &'a str,
    permission: &'a str,
    session: Option<&'a str>,
    control: &'a Control,
) -> Say<'a> {
    Say {
        command,
        env,
        prompt,
        cwd: dir,
        model: None,
        budget_usd: None,
        session_id: session,
        fork_at: None,
        permission: Some(permission),
        disallowed: &[],
        settings: None,
        mcp_config: None,
        effort: Some("low"),
        control: Some(control),
        on_session: None,
        add_dirs: &[],
        name: None,
    }
}

/// The person, answering every question a beat later from another thread.
fn answering(control: &Control, allow: bool) -> Arc<Mutex<Vec<Prompt>>> {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let noted = asked.clone();
    control.asks_with(Arc::new(move |prompt: Prompt, control: &Control| {
        let (id, control) = (prompt.request_id.clone(), control.clone());
        noted.lock().expect("asked").push(prompt);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let said = match allow {
                true => json!({ "behavior": "allow" }),
                false => json!({ "behavior": "deny", "message": "no" }),
            };
            control.answer(&id, said);
        });
        true
    }));
    asked
}

/// The answer's text blocks: what the chat joins, split where a tool ran.
fn blocks(parts: &[Part]) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    let mut open = false;
    for part in parts {
        match part {
            Part::Text { text, .. } if open => all.last_mut().expect("open").push_str(text),
            Part::Text { text, .. } => {
                all.push(text.clone());
                open = true;
            }
            _ => open = false,
        }
    }
    all
}

/// The answer's text, as one string.
fn answer_of(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn calls(parts: &[Part]) -> Vec<(String, Value)> {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::ToolCall {
                name, input, state, ..
            } => {
                assert_eq!(*state, CallState::Running);
                Some((name.clone(), serde_json::from_str(input).expect("input")))
            }
            _ => None,
        })
        .collect()
}

fn results(parts: &[Part]) -> Vec<(String, bool)> {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::ToolResult {
                output, is_error, ..
            } => Some((output.clone(), *is_error)),
            _ => None,
        })
        .collect()
}

/// A turn of its own process, as the supervised mode runs one.
#[cfg(unix)]
fn said_once(name: &str, allow: bool) -> (Said, Vec<Part>, Vec<Prompt>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let played = transcript(name);
    let command = stand_in(dir.path(), &played);
    let control = Control::new();
    let asked = answering(&control, allow);
    let prompt = played
        .iter()
        .find(|line| line.msg["method"] == "turn/start")
        .and_then(|line| line.msg["params"]["input"][0]["text"].as_str())
        .expect("a prompt")
        .to_owned();
    let mut parts = Vec::new();
    let codex = Codex::default();
    let say = turn(&command, &[], dir.path(), &prompt, "manual", None, &control);
    // A script just written can be busy for a moment when another test forks.
    let said = (0..5)
        .find_map(
            |_| match crate::talk::say(&codex, &say, |part| parts.push(part), |_| {}) {
                Err(crate::AgentError::NotInstalled) => {
                    std::thread::sleep(Duration::from_millis(50));
                    None
                }
                said => Some(said),
            },
        )
        .expect("started")
        .expect("said");
    let (heard, expected) = compared(dir.path(), &played);
    assert_eq!(
        heard, expected,
        "devpit wrote what Codex was recorded hearing"
    );
    let asked = std::mem::take(&mut *asked.lock().expect("asked"));
    (said, parts, asked, dir)
}

/// What a process that stays heard, and the handle that steers or stops it.
struct Staying {
    resident: Resident,
    heard: mpsc::Receiver<Heard>,
    dir: tempfile::TempDir,
    played: Vec<Line>,
}

#[cfg(unix)]
fn staying(name: &str, mode: &str, home: &str, session: Option<&str>) -> Staying {
    let dir = tempfile::tempdir().expect("tempdir");
    let played = transcript(name);
    let command = stand_in(dir.path(), &played);
    let env = vec![("CODEX_HOME".to_owned(), home.to_owned())];
    let control = Control::new();
    answering(&control, false);
    let prompt = played
        .iter()
        .find(|line| line.msg["method"] == "turn/start")
        .and_then(|line| line.msg["params"]["input"][0]["text"].as_str())
        .expect("a prompt")
        .to_owned();
    let say = turn(&command, &env, dir.path(), &prompt, mode, session, &control);
    let (tell, heard) = mpsc::channel();
    let tell = Arc::new(Mutex::new(tell));
    let resident = (0..5)
        .find_map(|_| {
            let tell = tell.clone();
            let started = Resident::start(
                Box::<Codex>::default(),
                &say,
                Arc::new(|_: &str| {}),
                move |one| {
                    let _ = tell.lock().expect("tell").send(one);
                },
            );
            match started {
                Err(crate::AgentError::NotInstalled) => {
                    std::thread::sleep(Duration::from_millis(50));
                    None
                }
                started => Some(started),
            }
        })
        .expect("spawned")
        .expect("started");
    assert!(resident.say(&prompt));
    Staying {
        resident,
        heard,
        dir,
        played,
    }
}

impl Staying {
    /// Its parts until the turn ends, with `at_call` run on the first tool call.
    fn until_ended(&self, mut at_call: impl FnMut(&Resident)) -> (Said, Vec<Part>) {
        let mut parts = Vec::new();
        loop {
            match self.heard.recv_timeout(Duration::from_secs(20)) {
                Ok(Heard::Part(part)) => {
                    if matches!(part, Part::ToolCall { .. }) {
                        at_call(&self.resident);
                    }
                    parts.push(part);
                }
                Ok(Heard::Ended(said)) => return (*said, parts),
                Ok(Heard::Control(_)) => {}
                Ok(Heard::Gone) | Err(_) => panic!("the turn never ended"),
            }
        }
    }

    fn heard_all(&self) {
        self.resident.close();
        let (heard, expected) = compared(self.dir.path(), &self.played);
        assert_eq!(
            heard, expected,
            "devpit wrote what Codex was recorded hearing"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_turn_streams_its_answer_shows_the_command_and_the_edit_and_asks_before_each() {
    let (said, parts, asked, dir) = said_once("approved", true);

    assert_eq!(
        blocks(&parts),
        [
            "I’ll list the directory, then create `hello.txt` containing `hi`.",
            "Ran `ls -a` and created `hello.txt` containing `hi`."
        ]
    );
    let calls = calls(&parts);
    assert_eq!(calls[0].0, "Bash");
    assert_eq!(calls[0].1["command"], "ls -a");
    assert_eq!(calls[1].0, "Edit");
    let file = dir.path().join("hello.txt").display().to_string();
    assert_eq!(calls[1].1["file_path"], file.as_str());
    let results = results(&parts);
    assert_eq!(results[0], (".\n..\n.git\nnotes.txt\n".to_owned(), false));
    assert_eq!(results[1], (format!("add {file}\nhi\n"), false));

    let tools: Vec<&str> = asked.iter().map(|one| one.tool.as_str()).collect();
    assert_eq!(tools, ["Bash", "Edit"], "each change waited for the person");
    assert_eq!(asked[0].input["command"], "ls -a");
    assert_eq!(asked[0].session_id.as_deref(), Some(THREAD));

    assert_eq!(said.session_id.as_deref(), Some(THREAD));
    assert_eq!(
        said.init.and_then(|init| init.model).as_deref(),
        Some("gpt-6.1-sol")
    );
    assert_eq!(said.end.stop_reason.as_deref(), Some("completed"));
    assert!(!said.end.is_error);
    assert_eq!(
        said.end.context,
        Some(Context {
            used: 15_353,
            window: WINDOW
        })
    );
    assert_eq!(
        said.end.cost_usd, None,
        "the price table does not know the model"
    );
}

#[cfg(unix)]
#[test]
fn a_refused_command_is_declined_and_the_turn_goes_on() {
    let (said, parts, asked, _dir) = said_once("declined", false);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].input["command"], "touch denied.txt");
    assert_eq!(results(&parts), [("declined".to_owned(), true)]);
    assert!(answer_of(&parts).ends_with("The command `touch denied.txt` was refused."));
    assert_eq!(said.end.stop_reason.as_deref(), Some("completed"));
}

#[cfg(unix)]
#[test]
fn stop_interrupts_the_turn_and_keeps_the_process() {
    let run = staying("stopped", "bypassPermissions", "/codex/a", None);
    let (said, parts) = run.until_ended(|resident| assert!(resident.interrupt()));
    assert_eq!(calls(&parts)[0].1["command"], "sleep 30");
    assert_eq!(said.end.stop_reason.as_deref(), Some("interrupted"));
    assert!(!said.end.is_error);
    run.heard_all();
}

#[cfg(unix)]
#[test]
fn a_message_steered_in_joins_the_running_turn() {
    let run = staying("steered", "bypassPermissions", "/codex/a", None);
    let (said, parts) = run.until_ended(|resident| {
        assert!(resident.steer(
            "Also end your answer with the word banana.",
            "u-steer-1",
            "steer-1"
        ));
    });
    assert!(answer_of(&parts).ends_with("banana"));
    assert!(run.resident.unheard().is_empty(), "Codex took the steer in");
    assert_eq!(said.end.stop_reason.as_deref(), Some("completed"));
    run.heard_all();
}

/// After an idle-out the process is gone; the next turn resumes the thread.
#[cfg(unix)]
#[test]
fn the_next_process_resumes_the_thread_where_it_was() {
    let run = staying("resumed", "bypassPermissions", "/codex/a", Some(THREAD));
    let (said, parts) = run.until_ended(|_| {});
    assert_eq!(answer_of(&parts), "I created `hello.txt` containing `hi`.");
    assert_eq!(said.session_id.as_deref(), Some(THREAD));
    run.heard_all();
}

#[cfg(unix)]
#[test]
fn two_profiles_with_their_own_homes_run_side_by_side() {
    let first = staying("resumed", "bypassPermissions", "/codex/work", Some(THREAD));
    let second = staying("stopped", "bypassPermissions", "/codex/personal", None);
    let (stopped, _) = second.until_ended(|resident| assert!(resident.interrupt()));
    let (resumed, parts) = first.until_ended(|_| {});

    let home =
        |run: &Staying| std::fs::read_to_string(run.dir.path().join("home-seen")).expect("home");
    assert_eq!(home(&first), "/codex/work\n");
    assert_eq!(home(&second), "/codex/personal\n");
    assert_eq!(resumed.session_id.as_deref(), Some(THREAD));
    assert_ne!(stopped.session_id, resumed.session_id);
    assert_eq!(stopped.end.stop_reason.as_deref(), Some("interrupted"));
    assert_eq!(resumed.end.stop_reason.as_deref(), Some("completed"));
    assert!(answer_of(&parts).contains("hello.txt"));
    first.heard_all();
    second.heard_all();
}

/// Codex's lines, read straight through the driver, on `model` when given.
fn read_all(codex: &Codex, name: &str, model: Option<&str>) -> Vec<Read> {
    let wire = codex.wire();
    let dir = Path::new("/work");
    let control = Control::new();
    wire.hello(&turn(
        "codex",
        &[],
        dir,
        "",
        "bypassPermissions",
        None,
        &control,
    ));
    transcript(name)
        .iter()
        .filter(|line| line.dir == "<")
        .map(|line| {
            let mut line = filled(&line.msg, dir).to_string();
            if let Some(model) = model {
                line = line.replace("gpt-6.1-sol", model);
            }
            wire.replies(&line);
            codex.read(&line)
        })
        .collect()
}

fn cost_of_turn(reads: Vec<Read>) -> Option<f64> {
    match reads
        .into_iter()
        .rev()
        .find(|read| matches!(read, Read::Ended { .. }))
    {
        Some(Read::Ended { cost_usd, .. }) => cost_usd,
        _ => panic!("the turn ended"),
    }
}

#[test]
fn a_turn_costs_its_tokens_at_the_models_price_or_nothing_when_unknown() {
    assert_eq!(
        cost_of_turn(read_all(&Codex::default(), "approved", None)),
        None
    );

    // The same turn on a model the table prices. Its usage at the end, as the
    // transcript says: 30523 in, 24576 of them cached, 95 out.
    let cost = cost_of_turn(read_all(
        &Codex::default(),
        "approved",
        Some("claude-sonnet-4-5"),
    ));
    // $3 in, $0.30 cached, $15 out, per million.
    let expected = (5_947.0 * 3.0 + 24_576.0 * 0.3 + 95.0 * 15.0) / 1_000_000.0;
    assert!((cost.expect("priced") - expected).abs() < 1e-9);
}

#[test]
fn each_turn_of_a_process_adds_its_cost_to_what_the_process_spent() {
    let codex = Codex::default();
    let wire = codex.wire();
    let control = Control::new();
    let dir = Path::new("/work");
    wire.hello(&turn(
        "codex",
        &[],
        dir,
        "",
        "bypassPermissions",
        None,
        &control,
    ));
    held(&codex.thread).ran_on = Some("claude-sonnet-4-5".to_owned());
    let usage = |total: u64, last: u64| {
        json!({ "method": "thread/tokenUsage/updated", "params": { "tokenUsage": {
            "total": { "inputTokens": total, "outputTokens": 0, "totalTokens": total },
            "last": { "inputTokens": last, "outputTokens": 0, "totalTokens": last },
            "modelContextWindow": WINDOW,
        } } })
        .to_string()
    };
    let ended = r#"{"method":"turn/completed","params":{"turn":{"status":"completed"}}}"#;
    let started = r#"{"method":"turn/started","params":{"turn":{"id":"t"}}}"#;
    let mut spent = Vec::new();
    // A resumed thread counts the turns before this process too: 1M of them.
    for line in [
        started.to_owned(),
        usage(1_100_000, 100_000),
        ended.to_owned(),
        started.to_owned(),
        usage(1_300_000, 200_000),
        ended.to_owned(),
    ] {
        if let Read::Ended { cost_usd, .. } = codex.read(&line) {
            spent.push(cost_usd.expect("priced"));
        }
    }
    // $3 per million in: 0.1M, then 0.2M more.
    assert!(
        (spent[0] - 0.3).abs() < 1e-9 && (spent[1] - 0.9).abs() < 1e-9,
        "{spent:?}"
    );
}

#[test]
fn every_turn_names_the_model_it_runs_on() {
    let codex = Codex::default();
    let models: Vec<_> = read_all(&codex, "steered", None)
        .into_iter()
        .filter_map(|read| match read {
            Read::Init(init) => Some(init.model),
            _ => None,
        })
        .collect();
    let model = Some("gpt-6.1-sol".to_owned());
    assert_eq!(models, [model.clone(), model], "the thread, then its turn");
}

#[test]
fn the_permission_mode_is_codexs_approval_policy_and_sandbox() {
    assert_eq!(
        policy(Some("manual")),
        Some(("untrusted", "workspace-write"))
    );
    assert_eq!(
        policy(Some("acceptEdits")),
        Some(("untrusted", "workspace-write"))
    );
    assert_eq!(policy(Some("plan")), Some(("untrusted", "read-only")));
    assert_eq!(
        policy(Some("bypassPermissions")),
        Some(("never", "danger-full-access"))
    );
    assert_eq!(policy(None), None, "the person's own Codex config decides");
    assert_eq!(policy(Some("default")), None);
}

#[test]
fn accept_edits_takes_the_edit_and_asks_about_the_command() {
    const COMMAND: &str = "item/commandExecution/requestApproval";
    const EDIT: &str = "item/fileChange/requestApproval";
    assert!(allowed(Some("acceptEdits"), EDIT));
    assert!(!allowed(Some("acceptEdits"), COMMAND));
    assert!(!allowed(Some("manual"), EDIT));
    assert!(allowed(Some("bypassPermissions"), COMMAND));

    let codex = Codex::default();
    let wire = codex.wire();
    let control = Control::new();
    let dir = Path::new("/work");
    wire.hello(&turn("codex", &[], dir, "", "acceptEdits", None, &control));
    let mut answered = Vec::new();
    let mut asked = Vec::new();
    for line in transcript("approved").iter().filter(|line| line.dir == "<") {
        let line = filled(&line.msg, dir).to_string();
        answered.extend(wire.replies(&line));
        if let Some(prompt) = wire.asked(&line, None).and_then(|one| one.prompt) {
            asked.push(prompt.tool);
        }
        codex.read(&line);
    }
    assert_eq!(asked, ["Bash"]);
    let decisions: Vec<Value> = answered
        .iter()
        .map(|line| serde_json::from_str(line).expect("json"))
        .filter(|said: &Value| said["result"].get("decision").is_some())
        .collect();
    assert_eq!(
        decisions,
        [json!({ "id": 1, "result": { "decision": "accept" } })]
    );
}

#[test]
fn a_model_or_mode_switched_mid_conversation_goes_with_the_next_turn() {
    let codex = Codex::default();
    let wire = codex.wire();
    let control = Control::new();
    let dir = Path::new("/work");
    wire.hello(&turn("codex", &[], dir, "", "manual", None, &control));
    for line in transcript("approved")
        .iter()
        .filter(|line| line.dir == "<")
        .take(8)
    {
        wire.replies(&filled(&line.msg, dir).to_string());
    }
    let model = json!({ "subtype": "set_model", "model": "gpt-6-luna" });
    let mode = json!({ "subtype": "set_permission_mode", "mode": "bypassPermissions" });
    assert!(matches!(wire.control("s1", model), Some(Switch::Kept)));
    assert!(matches!(wire.control("s2", mode), Some(Switch::Kept)));
    let back = json!({ "subtype": "set_model", "model": "default" });
    assert!(
        wire.control("s3", back).is_none(),
        "only a new process goes back"
    );

    let sent: Value = serde_json::from_str(&wire.prompt("next")[0]).expect("json");
    assert_eq!(sent["method"], "turn/start");
    assert_eq!(sent["params"]["model"], "gpt-6-luna");
    assert_eq!(sent["params"]["threadId"], THREAD);
    assert_eq!(sent["params"]["approvalPolicy"], "never");
    assert_eq!(sent["params"]["sandboxPolicy"]["type"], "dangerFullAccess");
    let after: Value = serde_json::from_str(&wire.prompt("again")[0]).expect("json");
    assert!(after["params"].get("approvalPolicy").is_none(), "sent once");
}

#[test]
fn a_thread_codex_no_longer_has_starts_over_and_says_so() {
    let codex = Codex::default();
    let wire = codex.wire();
    let control = Control::new();
    let dir = Path::new("/work");
    let hello = wire.hello(&turn(
        "codex",
        &[],
        dir,
        "",
        "manual",
        Some("gone"),
        &control,
    ));
    assert!(hello[0].contains("initialize"));
    assert!(wire.prompt("hi").is_empty(), "held until the thread opens");
    let opened = wire.replies(r#"{"id":0,"result":{}}"#);
    assert!(opened[1].contains("thread/resume"));
    let refused = r#"{"id":1,"error":{"code":-32600,"message":"no rollout found"}}"#;
    let again = wire.replies(refused);
    assert!(again[0].contains("thread/start"));
    let Read::Parts(parts) = codex.read(refused) else {
        panic!("a notice");
    };
    assert!(answer_of(&parts).contains("could not resume this conversation (no rollout found)"));
}

#[test]
fn a_turn_codex_refuses_ends_with_its_reason() {
    let codex = Codex::default();
    let wire = codex.wire();
    let control = Control::new();
    let dir = Path::new("/work");
    wire.hello(&turn("codex", &[], dir, "", "manual", None, &control));
    wire.replies(r#"{"id":0,"result":{}}"#);
    wire.replies(r#"{"id":1,"result":{"thread":{"id":"t"},"model":"m"}}"#);
    assert!(wire.prompt("hi")[0].contains("turn/start"));
    let refused = r#"{"id":2,"error":{"code":-32600,"message":"model not found"}}"#;
    wire.replies(refused);
    assert_eq!(
        codex.read(refused),
        Read::Failed("model not found".to_owned())
    );
}

#[test]
fn two_messages_in_a_row_read_as_two_paragraphs() {
    let codex = Codex::default();
    let delta = |item: &str, text: &str| {
        json!({ "method": "item/agentMessage/delta", "params": { "itemId": item, "delta": text } })
            .to_string()
    };
    let parts: Vec<Part> = [
        delta("a", "First."),
        delta("a", " Still."),
        delta("b", "Second."),
    ]
    .iter()
    .flat_map(|line| match codex.read(line) {
        Read::Parts(parts) => parts,
        _ => Vec::new(),
    })
    .collect();
    assert_eq!(answer_of(&parts), "First. Still.\n\nSecond.");
}

#[test]
fn a_request_devpit_does_not_serve_is_refused_rather_than_left_waiting() {
    let codex = Codex::default();
    let said = codex
        .wire()
        .replies(r#"{"id":7,"method":"item/tool/requestUserInput","params":{}}"#);
    let said: Value = serde_json::from_str(&said[0]).expect("json");
    assert_eq!(said["id"], 7);
    assert_eq!(said["error"]["code"], -32601);
}
