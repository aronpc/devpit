use super::evicted;

#[test]
fn a_reachable_chat_is_not_closed_by_another_conversation() {
    let listening = [
        ("orch_a", "orch"),
        ("remote_b", "orch"),
        ("elsewhere", "api"),
    ];
    let closed = evicted(listening.into_iter(), "orch", |key| key == "remote_b");
    assert_eq!(closed, ["orch_a"]);
}

#[test]
fn two_remote_control_chats_in_one_project_both_stay() {
    let listening = [("remote_a", "api")];
    assert!(evicted(listening.into_iter(), "api", |_| true).is_empty());
}

/// The agent's process ending or missing is the agent's, so a turn it cut
/// short answers with a code the error report does not keep.
#[test]
fn a_turn_the_agent_cut_short_is_not_an_internal_error() {
    use devpit_agentcli::AgentError;
    use devpit_rpc::ErrorCode;

    let missing = super::turn_refused(AgentError::NotInstalled);
    let ended = super::turn_refused(AgentError::Unreadable(
        "the conversation's process ended".to_owned(),
    ));

    assert_eq!(missing.code, ErrorCode::NotFound);
    assert_eq!(ended.code, ErrorCode::Busy);
    assert!(ended.message.contains("process ended"), "{}", ended.message);
}

/// A project chat's turns go through its process as an orchestrator's do;
/// only the supervised mode runs a process per turn.
#[test]
fn a_project_chat_keeps_its_process_between_turns() {
    assert!(super::stays(Some("acceptEdits")));
    assert!(super::stays(Some("plan")));
    assert!(super::stays(None));
    assert!(!super::stays(Some(crate::asking::ASKING_MODE)));
}

#[cfg(unix)]
mod with_a_process {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use devpit_agentcli::driver::driver;
    use devpit_agentcli::resident::{Heard, Resident};
    use devpit_agentcli::talk::Say;
    use devpit_agentcli::AgentError;
    use devpit_rpc::Part;
    use serde_json::Value;

    use super::super::{answered, started_as, to_switch, Live, Residents};

    /// Writes down its arguments and every line it is given, and answers each
    /// message with its pid. Refuses a switch to Full access, as the CLI does
    /// for a process not started in it.
    const WRITES_DOWN: &str = r#"#!/bin/sh
printf '%s\n' "$@" > "$DIR/argv"
n=0
while read -r line; do
  printf '%s\n' "$line" >> "$DIR/stdin"
  case "$line" in
    *control_request*)
      id=$(printf '%s' "$line" | sed 's/.*"request_id":"\([^"]*\)".*/\1/')
      case "$line" in
        *bypassPermissions*) answer=error ;;
        *) answer=success ;;
      esac
      printf '{"type":"control_response","response":{"subtype":"%s","request_id":"%s"}}\n' "$answer" "$id"
      continue ;;
  esac
  n=$((n+1))
  printf '{"type":"system","subtype":"init","session_id":"s-1"}\n'
  printf '{"type":"assistant","message":{"id":"m-%s","role":"assistant","content":[{"type":"text","text":"%s"}]},"session_id":"s-1"}\n' "$n" "$$"
  printf '{"type":"result","subtype":"success","session_id":"s-1","total_cost_usd":0}\n'
done
"#;

    fn script(dir: &Path, body: &str) -> String {
        let cli = dir.join("fake-claude");
        std::fs::write(&cli, body).expect("script");
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        cli.to_string_lossy().into_owned()
    }

    fn turn<'a>(command: &'a str, env: &'a [(String, String)]) -> Say<'a> {
        Say {
            command,
            env,
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

    /// A chat's process, as `start` keeps it, with what it says sent to `tell`.
    fn live(turn: &Say<'_>, tell: mpsc::Sender<Heard>) -> Live {
        let switching = Arc::new(Mutex::new(None));
        // A script written a moment ago can be "text file busy" to exec.
        let resident = (0..5)
            .find_map(|_| {
                let tell = tell.clone();
                let answering = switching.clone();
                match Resident::start(
                    driver("claude").expect("driver"),
                    turn,
                    Arc::new(|_: &str| {}),
                    move |heard| match heard {
                        Heard::Control(said) => {
                            to_switch(&answering, said);
                        }
                        heard => {
                            let _ = tell.send(heard);
                        }
                    },
                ) {
                    Err(AgentError::NotInstalled) => {
                        std::thread::sleep(Duration::from_millis(50));
                        None
                    }
                    other => Some(other.expect("started")),
                }
            })
            .expect("the stand-in started within five tries");
        Live {
            resident,
            started_as: started_as(turn),
            model: turn.model.map(str::to_owned),
            permission: turn.permission.map(str::to_owned),
            switching,
            project_id: "api".to_owned(),
            listens: false,
            turn: 0,
            session: Arc::new(Mutex::new(None)),
            ear: Arc::default(),
        }
    }

    fn next(heard: &mpsc::Receiver<Heard>) -> String {
        match heard
            .recv_timeout(Duration::from_secs(5))
            .expect("heard in time")
        {
            Heard::Part(Part::Text { text, .. }) => text,
            Heard::Part(_) => "<part>".to_owned(),
            Heard::Ended(_) => "<ended>".to_owned(),
            Heard::Gone => "<gone>".to_owned(),
            Heard::Control(_) => "<control>".to_owned(),
        }
    }

    /// What the stand-in wrote to `name` once it has, a line each.
    fn written(dir: &Path, name: &str) -> Vec<String> {
        for _ in 0..100 {
            if let Ok(text) = std::fs::read_to_string(dir.join(name)) {
                return text.lines().map(str::to_owned).collect();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("{name} was never written");
    }

    #[test]
    fn switching_model_or_mode_goes_to_the_process_it_has() {
        let dir = tempfile::tempdir().expect("tempdir");
        let command = script(dir.path(), WRITES_DOWN);
        let env = [("DIR".to_owned(), dir.path().display().to_string())];
        let first = Say {
            model: Some("sonnet"),
            permission: Some("acceptEdits"),
            ..turn(&command, &env)
        };
        let (tell, heard) = mpsc::channel();
        let mut live = live(&first, tell);
        let pid = live.resident.pid();

        let switched = Say {
            model: Some("opus"),
            permission: Some("plan"),
            ..first
        };
        assert_eq!(started_as(&switched), live.started_as, "no new process");
        let (answers, count) = live.switch(&switched).expect("asked of it");
        assert!(super::super::switched(&answers, count), "the CLI took both");
        assert!(live.resident.say("hello"));
        assert_eq!(next(&heard), pid.to_string(), "the same process answered");
        assert_eq!(next(&heard), "<ended>");

        let lines: Vec<Value> = written(dir.path(), "stdin")
            .iter()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        let asked: Vec<_> = lines
            .iter()
            .map(|line| {
                line["request"]["subtype"]
                    .as_str()
                    .or(line["type"].as_str())
            })
            .collect();
        assert_eq!(
            asked,
            [Some("set_model"), Some("set_permission_mode"), Some("user")]
        );
        assert_eq!(lines[0]["request"]["model"], "opus");
        assert_eq!(lines[1]["request"]["mode"], "plan");

        // Full access the CLI refuses, back to no model, and a new effort take a new one.
        let (answers, count) = live
            .switch(&Say {
                permission: Some("bypassPermissions"),
                ..switched
            })
            .expect("asked of it");
        assert!(!super::super::switched(&answers, count));
        assert!(
            live.resident.stop_task("t-1"),
            "a later turn's stop reaches it"
        );
        assert!(live
            .switch(&Say {
                model: None,
                ..switched
            })
            .is_none());
        assert_ne!(
            started_as(&Say {
                effort: Some("high"),
                ..switched
            }),
            live.started_as
        );
        live.resident.close();
    }

    #[test]
    fn an_idle_chat_frees_its_process_and_the_next_turn_resumes_the_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let command = script(dir.path(), WRITES_DOWN);
        let env = [("DIR".to_owned(), dir.path().display().to_string())];
        let first = turn(&command, &env);
        let (tell, heard) = mpsc::channel();
        let mut idle = live(&first, tell.clone());
        idle.turn = 7;
        let pid = idle.resident.pid();
        let residents = Residents::default();
        residents
            .0
            .lock()
            .expect("residents")
            .insert("c1".to_owned(), idle);

        let wait = Duration::from_millis(10);
        residents.idle_out("c1", 6, wait, |_| false);
        residents.idle_out("c1", 7, wait, |_| true);
        assert!(
            residents.0.lock().expect("residents").contains_key("c1"),
            "a turn came since, or it is reachable"
        );
        residents.idle_out("c1", 7, wait, |_| false);
        assert_eq!(next(&heard), "<gone>");
        assert!(
            !devpit_pty::process::alive(pid as i32),
            "its process is gone"
        );
        assert!(residents.0.lock().expect("residents").is_empty());

        std::fs::remove_file(dir.path().join("argv")).expect("argv");
        let resumed = live(
            &Say {
                session_id: Some("s-1"),
                ..first
            },
            tell,
        );
        let argv = written(dir.path(), "argv");
        assert!(
            argv.windows(2).any(|two| two == ["--resume", "s-1"]),
            "{argv:?}"
        );
        resumed.resident.close();
    }

    #[test]
    fn an_orchestrators_process_never_idles_out() {
        let dir = tempfile::tempdir().expect("tempdir");
        let command = script(dir.path(), WRITES_DOWN);
        let env = [("DIR".to_owned(), dir.path().display().to_string())];
        let (tell, _heard) = mpsc::channel();
        let mut listening = live(&turn(&command, &env), tell);
        listening.listens = true;
        listening.turn = 7;
        let residents = Residents::default();
        residents
            .0
            .lock()
            .expect("residents")
            .insert("orch".to_owned(), listening);

        residents.idle_out("orch", 7, Duration::from_millis(10), |_| false);
        let kept = residents.0.lock().expect("residents").remove("orch");
        kept.expect("still there").resident.close();
    }

    /// Answers half, then waits for a line that never comes.
    const STOPS_HALF_WAY: &str = r#"#!/bin/sh
read -r _first
printf '{"type":"assistant","message":{"id":"m-1","role":"assistant","content":[{"type":"text","text":"half"}]},"session_id":"s-1"}\n'
read -r _never
"#;

    #[test]
    fn a_process_killed_mid_turn_ends_the_turn_with_what_it_said() {
        let dir = tempfile::tempdir().expect("tempdir");
        let command = script(dir.path(), STOPS_HALF_WAY);
        let asked = turn(&command, &[]);
        let (tell, heard) = mpsc::channel();
        let live = live(&asked, tell);
        let pid = live.resident.pid();
        assert!(live.resident.say("go"));

        let mut said = Vec::new();
        let mut started = Vec::new();
        let ended = answered(
            &heard,
            driver("claude").expect("driver").as_ref(),
            &asked,
            |part| {
                if let Part::Text { text, .. } = part {
                    said.push(text);
                }
                let _ = std::process::Command::new("kill")
                    .args(["-KILL", &pid.to_string()])
                    .status();
            },
            &mut |pid| {
                started.push(pid);
                true
            },
            || true,
        );

        assert!(ended.is_err(), "the turn ended rather than wait");
        assert_eq!(said, ["half"], "what it said is kept");
        assert!(started.is_empty(), "nothing said twice");
        assert!(!live.resident.say("next"), "the next turn starts another");
    }

    /// Dies on its first message; started again, answers with the message.
    const DIES_THE_FIRST_TIME: &str = r#"#!/bin/sh
if [ ! -e "$DIR/died" ]; then
  touch "$DIR/died"
  read -r _first
  exit 1
fi
while read -r line; do
  text=$(printf '%s' "$line" | sed 's/.*"text":"\([^"]*\)".*/\1/')
  printf '{"type":"system","subtype":"init","session_id":"s-1"}\n'
  printf '{"type":"assistant","message":{"id":"m-1","role":"assistant","content":[{"type":"text","text":"%s"}]},"session_id":"s-1"}\n' "$text"
  printf '{"type":"result","subtype":"success","session_id":"s-1","total_cost_usd":0}\n'
done
"#;

    #[test]
    fn a_process_gone_before_answering_has_the_message_said_again() {
        let dir = tempfile::tempdir().expect("tempdir");
        let command = script(dir.path(), DIES_THE_FIRST_TIME);
        let env = [("DIR".to_owned(), dir.path().display().to_string())];
        let asked = Say {
            prompt: "hello",
            ..turn(&command, &env)
        };
        let (tell, heard) = mpsc::channel();
        let live = live(&asked, tell);
        assert!(live.resident.say(asked.prompt));

        let mut said = Vec::new();
        let mut started = Vec::new();
        answered(
            &heard,
            driver("claude").expect("driver").as_ref(),
            &asked,
            |part| {
                if let Part::Text { text, .. } = part {
                    said.push(text);
                }
            },
            &mut |pid| {
                started.push(pid);
                true
            },
            || true,
        )
        .expect("answered by a process of its own");

        assert_eq!(said, ["hello"], "the same message reached it");
        assert_eq!(started.len(), 1);
        assert_ne!(started[0], live.resident.pid());
    }

    #[test]
    fn a_process_let_go_on_purpose_is_not_replaced() {
        let dir = tempfile::tempdir().expect("tempdir");
        let command = script(dir.path(), DIES_THE_FIRST_TIME);
        let env = [("DIR".to_owned(), dir.path().display().to_string())];
        let asked = turn(&command, &env);
        let (tell, heard) = mpsc::channel();
        let live = live(&asked, tell);
        assert!(live.resident.say("hello"));

        let mut started = Vec::new();
        let ended = answered(
            &heard,
            driver("claude").expect("driver").as_ref(),
            &asked,
            |_| {},
            &mut |pid| {
                started.push(pid);
                true
            },
            || false,
        );
        assert!(ended.is_err());
        assert!(started.is_empty());
    }
}
