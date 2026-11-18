use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use devpit_rpc::Part;

use super::{Heard, Resident};
use crate::driver::driver;
use crate::talk::Say;

/// A CLI that answers each line it is given with a turn, and after the first
/// one wakes by itself for a second — the way a message from another session
/// wakes the real one.
const STAND_IN: &str = r#"#!/bin/sh
answer() {
  printf '{"type":"system","subtype":"init","session_id":"s-1"}\n'
  printf '{"type":"assistant","message":{"id":"m-%s","role":"assistant","content":[{"type":"text","text":"%s"}]},"session_id":"s-1"}\n' "$1" "$1"
  printf '{"type":"result","subtype":"success","session_id":"s-1","total_cost_usd":0}\n'
}
read -r _first
answer asked
sleep 0.3
answer woken
while read -r _next; do answer again; done
"#;

fn turn(command: &str) -> Say<'_> {
    Say {
        command,
        env: &[],
        prompt: "",
        cwd: Path::new("/"),
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
    }
}

/// Starts the stand-in, with the retry the turn tests use: a script written a
/// moment ago can be "text file busy" while another test forks with it open.
fn started(command: &str, tell: mpsc::Sender<String>) -> Resident {
    (0..5)
        .find_map(|_| {
            let tell = tell.clone();
            match Resident::start(
                driver("claude").expect("driver"),
                &turn(command),
                Arc::new(|_: &str| {}),
                move |one| {
                    if let Some(line) = said(&one) {
                        let _ = tell.send(line);
                    }
                },
            ) {
                Err(crate::AgentError::NotInstalled) => {
                    std::thread::sleep(Duration::from_millis(50));
                    None
                }
                other => Some(other.expect("started")),
            }
        })
        .expect("the stand-in started within five tries")
}

fn said(heard: &Heard) -> Option<String> {
    match heard {
        Heard::Part(Part::Text { text, .. }) => Some(text.clone()),
        Heard::Ended(_) => Some("<ended>".to_owned()),
        Heard::Gone => Some("<gone>".to_owned()),
        _ => None,
    }
}

#[test]
fn it_stays_between_turns_and_is_heard_when_woken() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = dir.path().join("fake-claude");
    std::fs::write(&cli, STAND_IN).expect("script");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let command = cli.to_string_lossy().into_owned();

    let (tell, heard) = mpsc::channel();
    let resident = started(&command, tell);

    let next = || {
        heard
            .recv_timeout(Duration::from_secs(5))
            .expect("heard in time")
    };
    assert!(resident.say("hello"));
    assert_eq!((next(), next()), ("asked".to_owned(), "<ended>".to_owned()));
    // Nobody asked: the process woke on its own and was still heard.
    assert_eq!((next(), next()), ("woken".to_owned(), "<ended>".to_owned()));
    // And it is still there to be asked again.
    assert!(resident.say("again"));
    assert_eq!((next(), next()), ("again".to_owned(), "<ended>".to_owned()));

    resident.close();
    assert_eq!(next(), "<gone>");
}

/// A process that dies halfway through a turn is reported gone, with no end
/// frame: the turn failed, and whoever waits on it hears so instead of waiting.
#[test]
fn a_process_that_dies_mid_turn_is_heard_as_gone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = dir.path().join("fake-claude");
    std::fs::write(
        &cli,
        "#!/bin/sh\nread -r _first\nprintf '{\"type\":\"assistant\",\"message\":{\"id\":\"m-1\",\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"half\"}]},\"session_id\":\"s-1\"}\\n'\nexit 3\n",
    )
    .expect("script");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let command = cli.to_string_lossy().into_owned();

    let (tell, heard) = mpsc::channel();
    let resident = started(&command, tell);
    assert!(resident.say("go"));
    let next = || {
        heard
            .recv_timeout(Duration::from_secs(5))
            .expect("heard in time")
    };
    assert_eq!((next(), next()), ("half".to_owned(), "<gone>".to_owned()));
}

fn script(dir: &Path, body: &str) -> String {
    let cli = dir.join("fake-claude");
    std::fs::write(&cli, body).expect("script");
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    cli.to_string_lossy().into_owned()
}

/// A CLI that takes a while to start, then answers each line with its pid.
const STARTS_SLOWLY: &str = r#"#!/bin/sh
sleep 0.5
n=0
while read -r _next; do
  n=$((n+1))
  printf '{"type":"system","subtype":"init","session_id":"s-1"}\n'
  printf '{"type":"assistant","message":{"id":"m-%s","role":"assistant","content":[{"type":"text","text":"%s"}]},"session_id":"s-1"}\n' "$n" "$$"
  printf '{"type":"result","subtype":"success","session_id":"s-1","total_cost_usd":0}\n'
done
"#;

/// Turn start, measured from the prompt to the first word of the answer: a
/// process per turn starts the CLI each time, the process that stays does not.
#[test]
fn a_resident_starts_its_next_turn_without_starting_the_cli_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let command = script(dir.path(), STARTS_SLOWLY);

    let per_turn = (0..5)
        .find_map(|_| {
            let asked = std::time::Instant::now();
            let mut first = None;
            match crate::talk::say(
                driver("claude").expect("driver").as_ref(),
                &turn(&command),
                |_| {
                    first.get_or_insert_with(|| asked.elapsed());
                },
                |_| {},
            ) {
                Err(crate::AgentError::NotInstalled) => {
                    std::thread::sleep(Duration::from_millis(50));
                    None
                }
                other => {
                    other.expect("a turn of its own");
                    first
                }
            }
        })
        .expect("a process of its own answered");

    let (tell, heard) = mpsc::channel();
    let resident = started(&command, tell);
    let next = || {
        heard
            .recv_timeout(Duration::from_secs(5))
            .expect("heard in time")
    };
    assert!(resident.say("one"));
    assert_eq!(next(), resident.pid().to_string());
    assert_eq!(next(), "<ended>");
    let asked = std::time::Instant::now();
    assert!(resident.say("two"));
    assert_eq!(next(), resident.pid().to_string(), "the same process");
    let staying = asked.elapsed();
    eprintln!("turn start: {per_turn:?} with a process per turn, {staying:?} on one that stays");
    assert!(
        staying * 2 < per_turn,
        "{staying:?} on the process that stays, {per_turn:?} per turn"
    );
    resident.close();
}

/// Interrupts a turn that runs until it is stopped, and echoes a steer back.
const RUNS_UNTIL_STOPPED: &str = r#"#!/bin/sh
n=0
while read -r line; do
  case "$line" in
    *'"subtype":"interrupt"'*)
      printf '{"type":"result","subtype":"error_during_execution","session_id":"s-1","total_cost_usd":0}\n' ;;
    *'"uuid"'*)
      uuid=$(printf '%s' "$line" | sed 's/.*"uuid":"\([^"]*\)".*/\1/')
      printf '{"type":"user","isReplay":true,"uuid":"%s","session_id":"s-1","message":{"role":"user","content":[{"type":"text","text":"more"}]}}\n' "$uuid" ;;
    *)
      n=$((n+1))
      printf '{"type":"system","subtype":"init","session_id":"s-1"}\n'
      printf '{"type":"assistant","message":{"id":"m-%s","role":"assistant","content":[{"type":"text","text":"%s"}]},"session_id":"s-1"}\n' "$n" "$$" ;;
  esac
done
"#;

#[test]
fn an_interrupt_ends_the_turn_and_the_same_process_takes_the_next() {
    let dir = tempfile::tempdir().expect("tempdir");
    let command = script(dir.path(), RUNS_UNTIL_STOPPED);
    let (tell, heard) = mpsc::channel();
    let resident = started(&command, tell);
    let pid = resident.pid().to_string();
    let until_ended = || {
        let mut said = Vec::new();
        while said.last().map(String::as_str) != Some("<ended>") {
            said.push(
                heard
                    .recv_timeout(Duration::from_secs(5))
                    .expect("heard in time"),
            );
        }
        said
    };

    assert!(resident.say("go"));
    assert!(resident.steer("more", "u-1", "said-1"));
    assert!(resident.interrupt());
    assert!(until_ended().contains(&pid));
    assert!(resident.unheard().is_empty(), "the steer was taken in");

    assert!(resident.say("again"));
    assert!(resident.interrupt());
    assert!(until_ended().contains(&pid), "the same process answered");
    resident.close();
}

/// The CLI's result counts what the whole process has spent, measured on
/// 2.1.325: 0.0038, then 0.0076, then 0.0080 over three turns.
#[test]
fn each_turn_costs_what_it_added_to_the_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    let command = script(
        dir.path(),
        r#"#!/bin/sh
for total in 0.5 0.75; do
  read -r _next
  printf '{"type":"result","subtype":"success","session_id":"s-1","total_cost_usd":%s}\n' "$total"
done
"#,
    );
    let (tell, heard) = mpsc::channel();
    let resident = (0..5)
        .find_map(|_| {
            let tell = tell.clone();
            match Resident::start(
                driver("claude").expect("driver"),
                &turn(&command),
                Arc::new(|_: &str| {}),
                move |one| {
                    if let Heard::Ended(said) = one {
                        let _ = tell.send(said.end.cost_usd);
                    }
                },
            ) {
                Err(crate::AgentError::NotInstalled) => {
                    std::thread::sleep(Duration::from_millis(50));
                    None
                }
                other => Some(other.expect("started")),
            }
        })
        .expect("the stand-in started within five tries");
    let cost = || {
        heard
            .recv_timeout(Duration::from_secs(5))
            .expect("heard in time")
    };
    assert!(resident.say("one"));
    assert_eq!(cost(), Some(0.5));
    assert!(resident.say("two"));
    assert_eq!(cost(), Some(0.25));
    resident.close();
}

/// The resident chat is offered the Artifact tools too: it is the
/// orchestrator's chat, where the missing tools were first seen.
#[test]
fn a_resident_chat_is_offered_the_artifact_tools() {
    let dir = tempfile::tempdir().expect("tempdir");
    let command = script(
        dir.path(),
        r#"#!/bin/sh
read -r _first
printf '{"type":"system","subtype":"init","session_id":"s-1"}\n'
printf '{"type":"assistant","message":{"id":"m-1","role":"assistant","content":[{"type":"text","text":"[%s]"}]},"session_id":"s-1"}\n' "$CLAUDE_CODE_ARTIFACT"
printf '{"type":"result","subtype":"success","session_id":"s-1","total_cost_usd":0}\n'
"#,
    );
    let (tell, heard) = mpsc::channel();
    let resident = started(&command, tell);
    assert!(resident.say("hello"));
    let first = heard
        .recv_timeout(Duration::from_secs(5))
        .expect("heard in time");
    // `[1]`, or the value this run inherited, which devpit leaves as it is.
    let inherited = std::env::var("CLAUDE_CODE_ARTIFACT");
    assert_eq!(first, format!("[{}]", inherited.as_deref().unwrap_or("1")));
    resident.close();
}
