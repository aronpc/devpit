//! Gemini CLI's hooks, in Claude Code's words.
//!
//! Gemini fires hooks of nearly the same shape under other names, and its
//! tools are named its own way. devpit reads one shape, so a Gemini report is
//! put into Claude Code's on arrival: the event's name, the tool's, the path
//! under the field the rest reads, a turn's last reply where a `Stop` keeps
//! it. What devpit has no use for is left out, as for Claude Code.
//!
//! Gemini takes its hooks from a settings file it merges under the person's
//! own — `GEMINI_CLI_SYSTEM_DEFAULTS_PATH`, set in devpit's terminals — and
//! its hook arrays are concatenated, so the person's own hooks still run and
//! their settings file is never written.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

/// The Gemini events devpit listens to, and the Claude Code event each is.
/// `AfterModel` is left out on purpose: it fires on every chunk of a reply.
pub const EVENTS: [(&str, &str); 7] = [
    ("BeforeTool", "PreToolUse"),
    ("AfterTool", "PostToolUse"),
    ("BeforeAgent", "UserPromptSubmit"),
    ("AfterAgent", "Stop"),
    ("Notification", "Notification"),
    ("SessionStart", "SessionStart"),
    ("SessionEnd", "SessionEnd"),
];

/// Gemini's tools, as the island names them.
fn tool_of(name: &str) -> &str {
    match name {
        "run_shell_command" => "Bash",
        "write_file" => "Write",
        "replace" | "edit" => "Edit",
        "read_file" | "read_many_files" => "Read",
        "glob" => "Glob",
        "search_file_content" | "grep" => "Grep",
        "list_directory" => "LS",
        "web_fetch" => "WebFetch",
        "google_web_search" => "WebSearch",
        "save_memory" => "Remember",
        other => other,
    }
}

/// A Gemini hook payload as a Claude Code one, or `None` for an event devpit
/// does not listen to.
pub fn claude_shaped(body: &str) -> Option<String> {
    let given: Map<String, Value> = serde_json::from_str::<Value>(body)
        .ok()?
        .as_object()?
        .clone();
    let event = given.get("hook_event_name")?.as_str()?;
    let (_, claude) = EVENTS.iter().find(|(gemini, _)| *gemini == event)?;
    let mut out = Map::new();
    for key in ["session_id", "cwd", "transcript_path"] {
        if let Some(value) = given.get(key) {
            out.insert(key.to_owned(), value.clone());
        }
    }
    let mut claude = (*claude).to_owned();
    if let Some(name) = given.get("tool_name").and_then(Value::as_str) {
        out.insert("tool_name".into(), json!(tool_of(name)));
    }
    if let Some(Value::Object(input)) = given.get("tool_input") {
        let mut input = input.clone();
        // The path under the field the rest of devpit reads.
        if !input.contains_key("file_path") {
            if let Some(path) = ["absolute_path", "path", "dir_path"]
                .iter()
                .find_map(|key| input.get(*key).cloned())
            {
                input.insert("file_path".into(), path);
            }
        }
        out.insert("tool_input".into(), Value::Object(input));
    }
    // A tool that answered with an error failed.
    if claude == "PostToolUse"
        && given
            .get("tool_response")
            .and_then(|response| response.get("error"))
            .is_some_and(|error| !error.is_null())
    {
        claude = "PostToolUseFailure".to_owned();
    }
    if let Some(said) = given.get("prompt_response").and_then(Value::as_str) {
        out.insert("last_assistant_message".into(), json!(said));
    }
    // Gemini notifies only to ask for a tool's permission: the session waits.
    if claude == "Notification" {
        out.insert("notification_type".into(), json!("permission_prompt"));
    }
    if let Some(source) = given.get("source") {
        out.insert("source".into(), source.clone());
    }
    if let Some(reason) = given.get("reason") {
        out.insert("reason".into(), reason.clone());
    }
    out.insert("hook_event_name".into(), json!(claude));
    Some(Value::Object(out).to_string())
}

/// Where Gemini reads the settings it merges under the person's own: the
/// variable when one is set, the system's file otherwise — as Gemini decides.
pub fn defaults_path() -> PathBuf {
    if let Some(path) = std::env::var_os(DEFAULTS_ENV).filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    let system = if cfg!(target_os = "macos") {
        "/Library/Application Support/GeminiCli"
    } else if cfg!(windows) {
        r"C:\ProgramData\gemini-cli"
    } else {
        "/etc/gemini-cli"
    };
    Path::new(system).join("system-defaults.json")
}

/// The variable that points Gemini at a defaults file.
pub const DEFAULTS_ENV: &str = "GEMINI_CLI_SYSTEM_DEFAULTS_PATH";

/// The defaults file a devpit terminal points Gemini at: the system's own,
/// when there is one, with devpit's hooks added to its own and `server`, devpit's
/// MCP server, beside its servers.
///
/// `None` when the system's file cannot be read as settings: pointed
/// elsewhere, Gemini would lose them, and losing someone's settings is worse
/// than not hearing from Gemini.
pub fn settings_json(
    endpoint_file: &Path,
    auth_file: &Path,
    theirs: Option<&str>,
    server: Option<&Value>,
) -> Option<String> {
    let mut settings = match theirs {
        None => Map::new(),
        Some(text) => serde_json::from_str::<Value>(text)
            .ok()?
            .as_object()?
            .clone(),
    };
    let post = crate::hook_settings::post(
        &endpoint_file.display().to_string(),
        &auth_file.display().to_string(),
        "1.5",
        false,
        Some("gemini"),
    );
    let hooks = settings.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks.as_object_mut()?;
    for (event, _) in EVENTS {
        let ours = json!({ "matcher": "*", "hooks": [{ "type": "command", "command": post, "timeout": 2000 }] });
        let list = hooks.entry(event).or_insert_with(|| json!([]));
        let list = list.as_array_mut()?;
        // Written over a file that already has ours, it stays one copy.
        if !list.contains(&ours) {
            list.push(ours);
        }
    }
    // A system file whose `mcpServers` is not a map keeps it as it is.
    if let Some(server) = server {
        let servers = settings.entry("mcpServers").or_insert_with(|| json!({}));
        if let Some(servers) = servers.as_object_mut() {
            servers.insert("devpit".to_owned(), server.clone());
        }
    }
    Some(Value::Object(settings).to_string())
}

#[cfg(test)]
#[path = "gemini_tests.rs"]
mod tests;
