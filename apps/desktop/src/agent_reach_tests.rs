use super::*;

use devpit_core::locale::Locale;

use crate::session_locale::SessionLocale;

/// The session of most tests: born in English, the base.
fn en() -> SessionLocale {
    SessionLocale::of(Locale::En)
}

#[test]
fn claude_is_given_a_config_file_it_reads_for_this_session_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("mcp.json");
    let flags = mcp_flags(en(), "claude", Path::new("/opt/devpit"), &config).expect("flags");
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
        en(),
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
        en(),
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
            tool_timeout() > devpit_agentapi::client::wait_for(method),
            "{method}"
        );
    }
}

#[test]
fn an_agent_with_no_way_to_be_given_it_gets_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        mcp_flags(
            en(),
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
            en(),
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
    let flags =
        mcp_flags(en(), "codex", Path::new(r"C:\devpit\devpit.exe"), &config).expect("flags");
    assert!(flags.contains(r#"mcp_servers.devpit.command="C:/devpit/devpit.exe""#));
    assert!(flags.contains(r#"DEVPIT_LOCALE="en""#), "{flags}");
    assert!(mcp_flags(en(), "claude", Path::new(r"C:\devpit\devpit.exe"), &config).is_some());
}

/// Every file below `dir`, relative to it.
fn files_under(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(
                files_under(&path)
                    .into_iter()
                    .map(|inner| format!("{}/{inner}", entry.file_name().to_string_lossy())),
            );
        } else {
            found.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    found.sort();
    found
}

/// A Codex chat, an ACP chat and a Gemini terminal reach devpit's server
/// through files in devpit's own folder, each signed with its agent: nothing
/// else in the person's home is written.
#[test]
fn each_cli_is_handed_devpits_server_from_devpits_own_folder_only() {
    let home = tempfile::tempdir().expect("tempdir");
    let root = home.path().join(".devpit");
    std::fs::create_dir_all(home.path().join(".codex")).expect("codex home");
    std::fs::create_dir_all(&root).expect("root");
    let exe = Path::new("/opt/devpit");
    let codex = chat_mcp_in(en(), &root, exe, "codex", "codex").expect("codex");
    let opencode = chat_mcp_in(en(), &root, exe, "acp", "opencode").expect("opencode");
    chat_mcp_in(en(), &root, exe, "claude", "claude").expect("claude");
    crate::shell_launch::gemini_settings(en(), &root).expect("gemini");

    assert_eq!(
        files_under(home.path()),
        [
            ".devpit/gemini-hooks.json",
            ".devpit/mcp-codex-en.json",
            ".devpit/mcp-opencode-en.json",
            ".devpit/mcp.json",
        ]
    );
    let signed = |file: &str| -> serde_json::Value {
        let text = std::fs::read_to_string(file).expect("read");
        serde_json::from_str::<serde_json::Value>(&text).expect("json")["mcpServers"]["devpit"]
            ["env"]["DEVPIT_AGENT_ID"]
            .clone()
    };
    assert_eq!(signed(&codex), "codex");
    assert_eq!(signed(&opencode), "opencode");
    let gemini: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("gemini-hooks.json")).expect("read"),
    )
    .expect("json");
    let server = &gemini["mcpServers"]["devpit"];
    assert_eq!(server["args"][0], "mcp");
    assert_eq!(server["env"]["DEVPIT_AGENT_ID"], "gemini");
    assert_eq!(server["env"]["DEVPIT_PANE"], "${DEVPIT_PANE}");
    assert_eq!(server["env"]["DEVPIT_LOCALE"], "${DEVPIT_LOCALE}");
}

/// The agent names the file: one devpit does not know names none.
#[test]
fn a_chat_for_an_agent_devpit_does_not_know_gets_no_file() {
    let root = tempfile::tempdir().expect("tempdir");
    let exe = Path::new("/opt/devpit");
    assert_eq!(
        chat_mcp_in(en(), root.path(), exe, "acp", "../escape"),
        None
    );
    assert_eq!(chat_mcp_in(en(), root.path(), exe, "", "aider"), None);
    assert!(files_under(root.path()).is_empty());
}

/// Claude's config file is every Claude session's, so no session's language
/// can be written into it: one session's would be every session's. The
/// language rides the session's own environment instead.
#[test]
fn the_language_never_rides_claudes_shared_config_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("mcp.json");
    mcp_flags(en(), "claude", Path::new("/opt/devpit"), &config).expect("flags");
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config).expect("read")).expect("json");
    assert!(
        written["mcpServers"]["devpit"]["env"]["DEVPIT_LOCALE"].is_null(),
        "{written}"
    );
}

/// Codex hands a server only what its own line says, so the session's
/// language is baked into the env on that line — each session's line its own.
#[test]
fn codex_is_told_the_sessions_language_on_its_own_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let flags = mcp_flags(
        SessionLocale::of(Locale::PtBr),
        "codex",
        Path::new("/opt/devpit"),
        &dir.path().join("m.json"),
    )
    .expect("flags");
    assert!(
        flags
            .contains(r#"mcp_servers.devpit.env={DEVPIT_AGENT_ID="codex", DEVPIT_LOCALE="pt-BR"}"#),
        "{flags}"
    );
}

/// A chat's server is handed its session's language in a file of the
/// language's own, per agent: never the file every Claude session reads.
#[test]
fn a_chats_server_file_is_of_its_language_and_carries_it() {
    let root = tempfile::tempdir().expect("tempdir");
    let exe = Path::new("/opt/devpit");
    for (driver, agent) in [("codex", "codex"), ("acp", "opencode")] {
        let config = chat_mcp_in(
            SessionLocale::of(Locale::PtBr),
            root.path(),
            exe,
            driver,
            agent,
        )
        .expect("a chat");
        let name = std::path::Path::new(&config)
            .file_name()
            .expect("a file")
            .to_string_lossy()
            .into_owned();
        assert!(name.contains("pt-BR"), "{name}");
        assert_ne!(name, "mcp.json", "{name}");
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).expect("read")).expect("json");
        assert_eq!(
            written["mcpServers"]["devpit"]["env"]["DEVPIT_LOCALE"],
            "pt-BR"
        );
    }
}

/// Every server devpit hands out waits past devpit's own longest answer.
#[test]
fn the_server_waits_on_a_tool_as_long_as_devpit_may_take() {
    let timeout = server(en(), "/opt/devpit", "codex")["timeout"]
        .as_u64()
        .expect("ms");
    assert_eq!(u128::from(timeout), tool_timeout().as_millis());
}
