use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::*;

fn lines(all: &[Value]) -> Cursor<Vec<u8>> {
    let mut text = String::new();
    for one in all {
        text.push_str(&one.to_string());
        text.push('\n');
    }
    Cursor::new(text.into_bytes())
}

fn written(bytes: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn nothing(_: &str, _: Value) -> Result<Value, String> {
    Err("not asked".to_owned())
}

#[test]
fn the_override_comes_before_the_installed_one() {
    let dir = tempfile::tempdir().unwrap();
    let installed = dir.path().join("devflow");
    std::fs::write(&installed, "").unwrap();
    let found = candidates(
        Some("/opt/mine/devflow".into()),
        Some(installed.clone()),
        std::ffi::OsStr::new(""),
    );
    assert_eq!(found, vec![PathBuf::from("/opt/mine/devflow"), installed]);
}

#[test]
fn an_installed_path_with_no_file_is_not_a_candidate() {
    let found = candidates(
        None,
        Some(PathBuf::from("/nowhere/devflow")),
        std::ffi::OsStr::new(""),
    );
    assert!(found.is_empty());
}

#[test]
fn a_devflow_from_0_5_0_speaking_protocol_1_is_taken() {
    assert_eq!(
        usable(r#"{"version":"0.5.0","protocols":[1]}"#),
        Ok("0.5.0".to_owned())
    );
    assert_eq!(
        usable(r#"{"version":"0.6.2","protocols":[1,2]}"#),
        Ok("0.6.2".to_owned())
    );
}

#[test]
fn an_older_devflow_or_one_without_protocol_1_is_old() {
    for (said, version) in [
        (r#"{"version":"0.4.0","protocols":[1]}"#, "0.4.0"),
        (r#"{"version":"1.0.0","protocols":[2]}"#, "1.0.0"),
    ] {
        assert_eq!(
            usable(said),
            Err(Unusable::Old {
                version: version.to_owned()
            })
        );
    }
}

#[test]
fn a_plain_text_version_is_unreadable() {
    assert!(matches!(
        usable("devflow 0.3.0"),
        Err(Unusable::Unreadable { .. })
    ));
}

#[test]
fn the_first_usable_candidate_is_the_one_used() {
    let all = [PathBuf::from("/a/devflow"), PathBuf::from("/b/devflow")];
    let ask = |path: &Path| {
        Ok(if path.starts_with("/a") {
            r#"{"version":"9.0.0","protocols":[2]}"#.to_owned()
        } else {
            r#"{"version":"0.5.1","protocols":[1]}"#.to_owned()
        })
    };
    assert_eq!(
        found_in(&all, &ask),
        Ok(Devflow {
            path: PathBuf::from("/b/devflow"),
            version: "0.5.1".to_owned(),
        })
    );
}

#[test]
fn with_none_usable_the_first_reason_is_given() {
    let all = [PathBuf::from("/a/devflow")];
    let ask = |_: &Path| Ok(r#"{"version":"9.0.0","protocols":[2]}"#.to_owned());
    assert_eq!(
        found_in(&all, &ask),
        Err(Unusable::Old {
            version: "9.0.0".to_owned()
        })
    );
    assert_eq!(found_in(&[], &ask), Err(Unusable::Missing));
}

#[test]
fn what_is_said_names_the_least_version_and_where_to_install() {
    let said = Unusable::Missing.said();
    assert!(said.contains(LEAST), "{said}");
    assert!(said.contains("Devflow screen"), "{said}");
    assert!(Unusable::Old {
        version: "0.3.0".to_owned()
    }
    .said()
    .contains("0.3.0"));
}

#[test]
fn devflow_s_requests_are_answered_before_its_result() {
    let mut reader = lines(&[
        json!({"jsonrpc": "2.0", "id": "d1", "method": "items/get", "params": {"id": "card_1"}}),
        json!({"jsonrpc": "2.0", "method": "progress", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 7, "result": {"passed": true}}),
    ]);
    let mut out = Vec::new();
    let host = |method: &str, params: Value| {
        assert_eq!(method, "items/get");
        Ok(json!({ "id": params["id"], "title": "Rename" }))
    };
    let answer = asked(
        &mut reader,
        &mut out,
        7,
        "tool",
        json!({"name": "gate"}),
        &host,
        1024,
    );
    assert_eq!(answer, Ok(json!({"passed": true})));
    let sent = written(&out);
    assert_eq!(sent[0]["method"], "tool");
    assert_eq!(sent[0]["id"], 7);
    assert_eq!(
        sent[1],
        json!({"jsonrpc": "2.0", "id": "d1", "result": {"id": "card_1", "title": "Rename"}})
    );
}

#[test]
fn a_request_devpit_cannot_answer_gets_an_error_back() {
    let mut reader = lines(&[
        json!({"jsonrpc": "2.0", "id": 9, "method": "decide", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 1, "result": null}),
    ]);
    let mut out = Vec::new();
    asked(&mut reader, &mut out, 1, "x", Value::Null, &nothing, 1024).unwrap();
    let sent = written(&out);
    assert_eq!(sent[1]["id"], 9);
    assert_eq!(sent[1]["error"]["message"], "not asked");
}

#[test]
fn an_error_from_devflow_is_its_message() {
    let mut reader = lines(&[json!({
        "jsonrpc": "2.0",
        "id": 2,
        "error": {"code": -32000, "message": "devflow is not on in this project", "data": {"kind": "not_on"}}
    })]);
    let mut out = Vec::new();
    let failed = asked(
        &mut reader,
        &mut out,
        2,
        "tool",
        Value::Null,
        &nothing,
        1024,
    )
    .unwrap_err();
    assert_eq!(failed.message, "devflow is not on in this project");
    assert_eq!(failed.data, Some(json!({"kind": "not_on"})));
    assert_eq!(failed.code, Some(-32000));
}

#[test]
fn a_devflow_that_ends_without_answering_says_so() {
    let mut reader = lines(&[]);
    let mut out = Vec::new();
    let failed = asked(
        &mut reader,
        &mut out,
        1,
        "tool",
        Value::Null,
        &nothing,
        1024,
    )
    .unwrap_err();
    assert!(
        failed.message.contains("ended before it answered tool"),
        "{failed:?}"
    );
}

#[test]
fn a_line_over_the_ceiling_is_refused() {
    let long = json!({"jsonrpc": "2.0", "id": 1, "result": "x".repeat(200)});
    let mut reader = lines(&[long]);
    let mut out = Vec::new();
    let failed = asked(&mut reader, &mut out, 1, "tool", Value::Null, &nothing, 64).unwrap_err();
    assert!(failed.message.contains("over 64 bytes"), "{failed:?}");
}

#[test]
fn exchange_initialises_before_the_method() {
    let mut reader = lines(&[
        json!({"jsonrpc": "2.0", "id": 1, "result": {"version": "0.4.0", "protocol": 1, "methods": ["config/read"]}}),
        json!({"jsonrpc": "2.0", "id": 2, "result": {"state": "valid"}}),
    ]);
    let mut out = Vec::new();
    let answer = exchange(
        &mut reader,
        &mut out,
        json!({"protocol": PROTOCOL}),
        "config/read",
        json!({}),
        &nothing,
    );
    assert_eq!(
        answer,
        Ok((
            json!({"version": "0.4.0", "protocol": 1, "methods": ["config/read"]}),
            json!({"state": "valid"})
        ))
    );
    let sent = written(&out);
    assert_eq!(sent[0]["method"], "initialize");
    assert_eq!(sent[0]["params"]["protocol"], 1);
    assert_eq!(sent[1]["method"], "config/read");
}

#[cfg(unix)]
fn script(dir: &Path, body: &str) -> Devflow {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("devflow");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    Devflow {
        path,
        version: "0.4.0".to_owned(),
    }
}

#[cfg(unix)]
#[test]
fn a_call_spawns_devflow_host_in_the_root() {
    let dir = tempfile::tempdir().unwrap();
    // Answers initialize, then the method with the arguments it was run with.
    let devflow = script(
        dir.path(),
        r#"read init
echo '{"jsonrpc":"2.0","id":1,"result":{"protocol":1}}'
read asked
printf '{"jsonrpc":"2.0","id":2,"result":{"args":"%s","pwd":"%s"}}\n' "$*" "$(pwd -P)""#,
    );
    let root = dir.path().canonicalize().unwrap();
    let answer = call(
        &devflow,
        &root,
        json!({}),
        "status",
        json!({}),
        &nothing,
        Duration::from_secs(10),
    )
    .unwrap();
    assert_eq!(
        answer["args"],
        format!("host --root {}", root.display()).as_str()
    );
    assert_eq!(answer["pwd"], root.display().to_string().as_str());
}

#[cfg(unix)]
#[test]
fn a_silent_devflow_is_stopped_at_the_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let devflow = script(dir.path(), "sleep 30");
    let started = std::time::Instant::now();
    let failed = call(
        &devflow,
        dir.path(),
        json!({}),
        "status",
        json!({}),
        &nothing,
        Duration::from_millis(300),
    )
    .unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(
        failed.message.contains("did not answer status"),
        "{failed:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_devflow_that_crashes_shows_what_it_printed() {
    let dir = tempfile::tempdir().unwrap();
    let devflow = script(dir.path(), "echo 'panicked at host.rs' >&2; exit 101");
    let failed = call(
        &devflow,
        dir.path(),
        json!({}),
        "status",
        json!({}),
        &nothing,
        Duration::from_secs(10),
    )
    .unwrap_err();
    assert!(failed.message.contains("panicked at host.rs"), "{failed:?}");
}

/// A devflow that says its version, serves `config/read` alone, and counts
/// each host session it starts in `sessions`.
#[cfg(unix)]
fn serving_config_read(dir: &Path) -> PathBuf {
    let sessions = dir.join("sessions");
    let devflow = script(
        dir,
        &format!(
            r#"if [ "$1" = version ]; then echo '{{"version":"0.5.0","protocols":[1]}}'; exit 0; fi
echo started >> '{}'
read init
echo '{{"jsonrpc":"2.0","id":1,"result":{{"protocol":1,"methods":["config/read","shutdown"]}}}}'
read asked
case "$asked" in
  *config/read*) echo '{{"jsonrpc":"2.0","id":2,"result":{{"state":"valid"}}}}' ;;
  *) echo '{{"jsonrpc":"2.0","id":2,"error":{{"code":-32601,"message":"method not found"}}}}' ;;
esac"#,
            sessions.display()
        ),
    );
    devflow.path
}

#[cfg(unix)]
#[test]
fn a_served_method_is_devflows_answer_and_one_it_lacks_is_left_to_devpit() {
    let dir = tempfile::tempdir().unwrap();
    let bin = serving_config_read(dir.path());
    TEST_DEVFLOW.with(|chosen| *chosen.borrow_mut() = Some(bin));
    let waits = Duration::from_secs(10);
    let ask = |method: &str| asked_of(dir.path(), json!({}), method, json!({}), &nothing, waits);

    let sessions = || {
        std::fs::read_to_string(dir.path().join("sessions"))
            .unwrap()
            .lines()
            .count()
    };
    // Asked before anything is known: devflow says it does not serve it.
    assert_eq!(ask("tool"), None, "a method it lacks is devpit's to answer");
    assert_eq!(sessions(), 1);
    assert_eq!(ask("config/read"), Some(Ok(json!({"state": "valid"}))));
    assert_eq!(sessions(), 2);
    assert_eq!(ask("config/write"), None);
    assert_eq!(sessions(), 2, "what initialize did not list costs no spawn");
    TEST_DEVFLOW.with(|chosen| *chosen.borrow_mut() = None);
}

#[test]
fn with_no_devflow_every_method_is_left_to_devpit() {
    TEST_DEVFLOW.with(|chosen| *chosen.borrow_mut() = None);
    let root = tempfile::tempdir().unwrap();
    let asked = asked_of(
        root.path(),
        json!({}),
        "config/read",
        json!({}),
        &nothing,
        Duration::from_secs(1),
    );
    assert_eq!(asked, None);
    assert_eq!(devflow(), Err(Unusable::Missing));
}

#[test]
fn an_update_that_moved_says_from_and_to() {
    assert_eq!(
        updated(r#"{"updated":true,"from":"0.4.0","to":"0.4.1"}"#),
        Some(("0.4.0".to_owned(), "0.4.1".to_owned()))
    );
    assert_eq!(
        updated(r#"{"updated":false,"from":"0.4.1","to":"0.4.1"}"#),
        None
    );
    assert_eq!(updated(r#"{"error":"no network"}"#), None);
    assert_eq!(updated(""), None);
}

/// Sabotage: hand every project to devflow, and one with its own
/// devflow.yaml at the root has that file read and written instead.
#[cfg(unix)]
#[test]
fn a_project_with_a_devflow_yaml_of_devflows_own_is_never_handed_to_devflow() {
    let scripts = tempfile::tempdir().unwrap();
    let bin = serving_config_read(scripts.path());
    TEST_DEVFLOW.with(|chosen| *chosen.borrow_mut() = Some(bin));
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".devpit/devflow")).unwrap();
    std::fs::write(
        root.path().join(".devpit/devflow/devflow.yaml"),
        "version: 1\n",
    )
    .unwrap();
    std::fs::write(root.path().join("devflow.yaml"), "version: 1\n").unwrap();
    let store = crate::projects::store().unwrap();
    let project: devpit_rpc::Project = serde_json::from_value(json!({
        "id": "prj_devflow_own_file", "name": "own", "rootPath": root.path().display().to_string(),
        "group": null, "accent": "#000000", "worktrees": [], "unreadable": null,
        "orchestrator": null, "origin": null, "lastOpenedAt": null, "icon": null, "color": null,
    }))
    .unwrap();
    let read = || crate::devflow::hosted(&store, &project, root.path(), "config/read", json!({}));

    assert_eq!(read(), None);
    assert!(
        !scripts.path().join("sessions").exists(),
        "devflow was not run"
    );
    std::fs::remove_file(root.path().join("devflow.yaml")).unwrap();
    assert_eq!(read(), Some(Ok(json!({"state": "valid"}))));
    TEST_DEVFLOW.with(|chosen| *chosen.borrow_mut() = None);
}
