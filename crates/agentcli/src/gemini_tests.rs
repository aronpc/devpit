use serde_json::json;

use super::claude_shaped;
use crate::{read_hook, Event};

fn heard(gemini: serde_json::Value) -> Option<crate::Happening> {
    read_hook(&claude_shaped(&gemini.to_string())?)
}

#[test]
fn a_gemini_tool_call_reads_as_a_step_with_its_target() {
    let happening = heard(json!({
        "hook_event_name": "BeforeTool", "session_id": "g1", "cwd": "/w/app",
        "tool_name": "replace", "tool_input": { "absolute_path": "/w/app/src/invoice.ts", "old_string": "a", "new_string": "b" },
    }))
    .expect("read");
    assert_eq!(happening.session_id, "g1");
    assert!(
        matches!(happening.event, Event::Using { ref tool, ref target, .. } if tool == "Edit" && target.as_deref() == Some("invoice.ts"))
    );

    let shell = heard(json!({
        "hook_event_name": "BeforeTool", "session_id": "g1", "cwd": "/w",
        "tool_name": "run_shell_command", "tool_input": { "command": "npm test" },
    }))
    .expect("read");
    assert!(
        matches!(shell.event, Event::Using { ref tool, ref target, .. } if tool == "Bash" && target.as_deref() == Some("npm test"))
    );
}

#[test]
fn a_tool_that_failed_and_a_turn_that_ended_read_as_such() {
    let failed = heard(json!({
        "hook_event_name": "AfterTool", "session_id": "g1", "cwd": "/w",
        "tool_name": "read_file", "tool_input": {}, "tool_response": { "error": "no such file" },
    }))
    .expect("read");
    assert_eq!(
        failed.event,
        Event::UseFailed {
            tool: "Read".to_owned(),
            error: None,
        }
    );

    let done = heard(json!({ "hook_event_name": "AfterAgent", "session_id": "g1", "cwd": "/w", "prompt_response": "All green" }))
        .expect("read");
    assert_eq!(
        done.event,
        Event::Stopped {
            said: Some("All green".to_owned())
        }
    );

    let asks = heard(json!({ "hook_event_name": "Notification", "session_id": "g1", "cwd": "/w", "notification_type": "ToolPermission" }))
        .expect("read");
    assert_eq!(asks.event, Event::Waiting);
}

#[test]
fn an_event_devpit_does_not_listen_to_is_left_alone() {
    assert_eq!(
        claude_shaped(&json!({ "hook_event_name": "AfterModel", "session_id": "g1" }).to_string()),
        None
    );
    assert_eq!(claude_shaped("not json"), None);
}

#[test]
fn the_defaults_keep_the_systems_own_and_add_ours_once() {
    use std::path::Path;

    use super::settings_json;

    let (endpoint, auth) = (
        Path::new("/h/.devpit/hook-endpoint"),
        Path::new("/h/.devpit/hook-auth"),
    );
    let theirs = r#"{"general":{"vimMode":true},"hooks":{"BeforeTool":[{"matcher":"*","hooks":[{"type":"command","command":"audit"}]}]}}"#;
    let written = settings_json(endpoint, auth, Some(theirs), None).expect("settings");
    let settings: serde_json::Value = serde_json::from_str(&written).expect("json");
    assert_eq!(settings["general"]["vimMode"], true);
    let before = settings["hooks"]["BeforeTool"].as_array().expect("list");
    assert_eq!(before.len(), 2);
    assert_eq!(before[0]["hooks"][0]["command"], "audit");
    let ours = before[1]["hooks"][0]["command"].as_str().expect("command");
    assert!(ours.contains("from=gemini"), "{ours}");
    assert!(settings["hooks"]["AfterModel"].is_null());

    // Written again over itself, still one copy of ours.
    let again: serde_json::Value = serde_json::from_str(
        &settings_json(endpoint, auth, Some(&written), None).expect("settings"),
    )
    .expect("json");
    assert_eq!(
        again["hooks"]["BeforeTool"].as_array().map(Vec::len),
        Some(2)
    );

    // A system file that is not settings is not hidden behind ours.
    assert_eq!(
        settings_json(endpoint, auth, Some("{ not json"), None),
        None
    );
    assert!(settings_json(endpoint, auth, None, None).is_some());
}

/// Gemini merges `mcpServers` from the defaults under the person's own, key
/// by key: devpit's server joins theirs and the system's without writing either.
#[test]
fn the_defaults_carry_devpits_server_beside_the_systems_own() {
    use std::path::Path;

    use super::settings_json;

    let (endpoint, auth) = (Path::new("/h/e"), Path::new("/h/a"));
    let server = serde_json::json!({ "command": "/opt/devpit", "args": ["mcp"] });
    let theirs = r#"{"mcpServers":{"search":{"command":"search-mcp"}}}"#;
    let written = settings_json(endpoint, auth, Some(theirs), Some(&server)).expect("settings");
    let settings: serde_json::Value = serde_json::from_str(&written).expect("json");
    assert_eq!(settings["mcpServers"]["search"]["command"], "search-mcp");
    assert_eq!(settings["mcpServers"]["devpit"], server);
    let again = settings_json(endpoint, auth, Some(&written), Some(&server)).expect("settings");
    assert_eq!(again, written, "written over itself, nothing changes");
}

/// As Gemini CLI 0.55 sent it, read from the defaults file a devpit terminal
/// points it at.
#[test]
fn a_recorded_gemini_session_start_is_heard() {
    let recorded = r#"{"session_id":"10322785-2d25-4499-a0ea-300fd67659ae","transcript_path":"/home/u/.gemini/tmp/gemini/chats/session-2026-10-02T03-17-10322785.jsonl","cwd":"/home/u/w","hook_event_name":"SessionStart","timestamp":"2026-10-02T03:17:54.295Z","source":"startup"}"#;
    let happening = read_hook(&claude_shaped(recorded).expect("shaped")).expect("read");
    assert_eq!(happening.session_id, "10322785-2d25-4499-a0ea-300fd67659ae");
    assert_eq!(happening.event, Event::SessionStarted);
}
