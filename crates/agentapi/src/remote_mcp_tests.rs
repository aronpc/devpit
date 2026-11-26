use std::cell::RefCell;

use super::*;

fn never(_: &str, _: Value) -> Result<Value, String> {
    panic!("the app should not have been asked")
}

/// The answer in English, whatever `DEVPIT_LOCALE` the shell running the
/// tests carries: a session devpit started has it set to its own language.
fn reply(line: &str, ask: &dyn Fn(&str, Value) -> Result<Value, String>) -> Value {
    serde_json::from_str(&handle_in("en", line, ask).expect("an answer")).expect("json")
}

fn calling(name: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": { "project": "p1" } } })
        .to_string()
}

/// What a client outside the machine may do, written out: a tool joins it
/// only by changing this line on purpose.
const READS: [&str; 5] = [
    "devpit_projects",
    "devpit_board",
    "devpit_card",
    "devpit_sessions",
    "devpit_waiting",
];

#[test]
fn the_remote_lists_exactly_its_read_only_allowlist() {
    let said = reply(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#, &never);
    let tools = said["result"]["tools"].as_array().expect("tools");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(names, READS);
    for tool in tools {
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
    }
}

#[test]
fn every_other_tool_is_refused_from_outside_without_asking_the_app() {
    let refused: Vec<&str> = crate::mcp::TOOLS
        .iter()
        .map(|tool| tool.name)
        .filter(|name| !READS.contains(name))
        .chain(["devpit_devflow_setup", "devpit_nothing", ""])
        .collect();
    // The ones that change something are among them, by name.
    for write in [
        "devpit_comment",
        "devpit_move_card",
        "devpit_start_session",
        "devpit_send_draft",
    ] {
        assert!(refused.contains(&write), "{write}");
    }
    for name in refused {
        let said = reply(&calling(name), &never);
        assert_eq!(said["result"]["isError"], true, "{name}");
        assert!(said["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("read-only"));
    }
}

#[test]
fn an_allowed_read_is_asked_of_the_app_by_its_method() {
    let asked = RefCell::new(Vec::new());
    let ask = |method: &str, params: Value| {
        asked.borrow_mut().push((method.to_owned(), params));
        Ok(json!({ "questions": [{ "tool": "Bash" }] }))
    };
    let said = reply(&calling("devpit_waiting"), &ask);
    assert_eq!(said["id"], 7);
    assert_eq!(said["result"]["isError"], false);
    assert!(said["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("Bash"));
    reply(&calling("devpit_board"), &ask);
    let methods: Vec<String> = asked.borrow().iter().map(|(m, _)| m.clone()).collect();
    assert_eq!(methods, ["waiting", "board"]);
    assert_eq!(asked.borrow()[1].1["project"], "p1");
}

#[test]
fn the_handshake_says_it_reads_and_offers_tools_alone() {
    let said = reply(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#,
        &never,
    );
    assert_eq!(said["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(
        said["result"]["capabilities"],
        json!({ "tools": {} }),
        "no resources, prompts or list changes from outside"
    );
    assert!(said["result"]["instructions"]
        .as_str()
        .unwrap_or_default()
        .contains("Read-only"));
    let resources = reply(
        r#"{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"ui://devpit/board"}}"#,
        &never,
    );
    assert_eq!(resources["error"]["code"], -32601);
}

#[test]
fn a_notification_is_heard_and_not_answered() {
    assert_eq!(
        handle(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &never
        ),
        None
    );
    let garbled = reply("not json", &never);
    assert_eq!(garbled["error"]["code"], -32700);
}
