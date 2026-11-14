use super::*;

#[test]
fn claude_is_given_a_config_file_it_reads_for_this_session_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("mcp.json");
    let flags = mcp_flags("claude", Path::new("/opt/devpit"), &config).expect("flags");
    assert_eq!(flags, format!("--mcp-config='{}'", config.display()));
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config).expect("read")).expect("json");
    assert_eq!(written["mcpServers"]["devpit"]["command"], "/opt/devpit");
    assert_eq!(written["mcpServers"]["devpit"]["args"][0], "mcp");
    assert_eq!(
        written["mcpServers"]["devpit"]["env"]["DEVPIT_AGENT_ID"],
        "claude"
    );
}

#[test]
fn codex_is_given_the_server_as_overrides_on_its_own_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let flags = mcp_flags(
        "codex",
        Path::new("/opt/devpit"),
        &dir.path().join("mcp.json"),
    )
    .expect("flags");
    assert!(
        flags.contains(r#"-c 'mcp_servers.devpit.command="/opt/devpit"'"#),
        "{flags}"
    );
    assert!(
        flags.contains(r#"mcp_servers.devpit.args=["mcp"]"#),
        "{flags}"
    );
    assert!(flags.contains(r#"DEVPIT_AGENT_ID="codex""#), "{flags}");
    assert!(
        flags.contains("-c 'mcp_servers.devpit.tool_timeout_sec=1260'"),
        "{flags}"
    );
}

/// Codex hands its servers only the variables it is told to: without the
/// pane, a Codex session's message could not say whose it is.
#[test]
fn codex_hands_devpits_server_the_pane_it_runs_in() {
    let dir = tempfile::tempdir().expect("tempdir");
    let flags = mcp_flags(
        "codex",
        Path::new("/opt/devpit"),
        &dir.path().join("mcp.json"),
    )
    .expect("flags");
    assert!(
        flags.contains(r#"-c 'mcp_servers.devpit.env_vars=["DEVPIT_PANE"]'"#),
        "{flags}"
    );
}

/// Codex's own sixty seconds cut off a card's checks and review while they
/// ran on, and the agent ran them again.
#[test]
fn codex_waits_on_a_tool_longer_than_devpit_does() {
    for method in crate::agent_api::METHODS {
        assert!(
            codex_tool_timeout() > devpit_agentapi::client::wait_for(method),
            "{method}"
        );
    }
}

#[test]
fn an_agent_with_no_way_to_be_given_it_gets_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        mcp_flags(
            "gemini",
            Path::new("/opt/devpit"),
            &dir.path().join("m.json")
        ),
        None
    );
}

/// A quote in the path would end the shell word early and run the rest.
#[test]
fn a_path_it_cannot_quote_is_not_carried() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        mcp_flags(
            "codex",
            Path::new("/opt/it's/devpit"),
            &dir.path().join("m.json")
        ),
        None
    );
}

#[test]
fn the_shim_runs_this_binary_as_the_agent_cli() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bin = cli_shim(dir.path(), Path::new("/opt/devpit")).expect("shim");
    let script = std::fs::read_to_string(bin.join("devpit-agent")).expect("read");
    assert_eq!(script, "#!/bin/sh\nexec '/opt/devpit' agent \"$@\"\n");
}

/// `devpit hook` stays quiet beside the settings by the variable the settings
/// set; two spellings of one name would report every event twice.
#[test]
fn the_hook_door_reads_the_variable_the_settings_set() {
    assert_eq!(devpit_agentapi::hook::HOOKED_ENV, devpit_agentcli::HOOKED);
}

/// On Windows the shim is a batch file PATHEXT finds as `devpit-agent`.
#[cfg(windows)]
#[test]
fn on_windows_the_shim_is_a_batch_file_that_passes_everything_on() {
    let (name, said) = shim(r"C:\Users\jo\AppData\Local\devpit\devpit.exe").expect("shim");
    assert_eq!(name, "devpit-agent.cmd");
    assert_eq!(
        said,
        "@echo off\r\n\"C:\\Users\\jo\\AppData\\Local\\devpit\\devpit.exe\" agent %*\r\n"
    );
    assert!(shim(r"C:\100%\devpit.exe").is_none());

    let dir = tempfile::tempdir().expect("tempdir");
    let bin = cli_shim(dir.path(), Path::new(r"C:\devpit\devpit.exe")).expect("written");
    assert!(bin.join("devpit-agent.cmd").is_file());
}

/// A Windows path is carried with forward slashes, which TOML does not read
/// as escapes and Windows reads as a path.
#[cfg(windows)]
#[test]
fn on_windows_the_tools_are_reached_through_forward_slashes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("mcp.json");
    let flags = mcp_flags("codex", Path::new(r"C:\devpit\devpit.exe"), &config).expect("flags");
    assert!(flags.contains(r#"mcp_servers.devpit.command="C:/devpit/devpit.exe""#));
    assert!(mcp_flags("claude", Path::new(r"C:\devpit\devpit.exe"), &config).is_some());
}
