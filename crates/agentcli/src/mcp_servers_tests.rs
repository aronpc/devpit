use serde_json::json;

use super::{acp, codex_overrides, parse};

const FILE: &str = r#"{"mcpServers":{"devpit":{"type":"stdio","command":"/opt/dev pit/devpit","args":["mcp"],"env":{"DEVPIT_AGENT_ID":"codex"},"timeout":1260000}}}"#;

#[test]
fn codex_takes_the_server_as_overrides_with_its_tool_timeout_in_seconds() {
    assert_eq!(
        codex_overrides(&parse(FILE)),
        [
            "-c",
            r#"mcp_servers.devpit.command="/opt/dev pit/devpit""#,
            "-c",
            r#"mcp_servers.devpit.args=["mcp"]"#,
            "-c",
            r#"mcp_servers.devpit.env={DEVPIT_AGENT_ID="codex"}"#,
            "-c",
            "mcp_servers.devpit.tool_timeout_sec=1260",
        ]
    );
}

/// A quote or a backslash in the path stays inside the TOML string.
#[test]
fn a_path_with_quotes_is_still_one_toml_string() {
    let file = json!({ "mcpServers": { "devpit": { "command": r#"C:\a "b"\devpit.exe"#, "args": ["mcp"] } } });
    let argv = codex_overrides(&parse(&file.to_string()));
    assert_eq!(
        argv[1],
        r#"mcp_servers.devpit.command="C:\\a \"b\"\\devpit.exe""#
    );
}

#[test]
fn an_acp_agent_takes_the_server_with_its_environment_as_a_list() {
    assert_eq!(
        acp(&parse(FILE)),
        json!([{
            "name": "devpit",
            "command": "/opt/dev pit/devpit",
            "args": ["mcp"],
            "env": [{ "name": "DEVPIT_AGENT_ID", "value": "codex" }],
        }])
    );
}

/// A name that is not a bare key would reach into another part of Codex's
/// configuration once it is put on its command line.
#[test]
fn a_name_that_is_not_a_bare_key_is_left_out() {
    let file = json!({ "mcpServers": {
        "a.b": { "command": "x" },
        "no-command": { "args": ["mcp"] },
        "ok_1": { "command": "y" },
    } });
    let names: Vec<String> = parse(&file.to_string())
        .into_iter()
        .map(|one| one.name)
        .collect();
    assert_eq!(names, ["ok_1"]);
}

#[test]
fn a_file_that_is_not_an_mcp_file_has_no_servers() {
    assert!(parse("{ not json").is_empty());
    assert!(parse("[]").is_empty());
    assert!(super::read(std::path::Path::new("/nonexistent/mcp.json")).is_empty());
}
