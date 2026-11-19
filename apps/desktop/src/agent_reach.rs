//! How an agent devpit starts finds devpit: the MCP server on its command
//! line, and `devpit-agent` on its PATH.
//!
//! On the command line and nowhere else, for the reason the hooks are
//! (`shell_launch::launch_line`): a flag reaches the agents this app started,
//! and editing a tool's own configuration would reach every agent the person
//! ever starts, which is not a menu item's to decide.
//!
//! The server is this binary (`devpit mcp`, `devpit-agentapi`). From an
//! AppImage that is the AppImage file, not the mount it runs from, which moves
//! on every start.
//!
//! Each session's language reaches the server with it: Claude Code hands a
//! stdio server its own process environment, so the variable on the session's
//! line is enough; Codex hands a server only what it is told, so the language
//! is baked into the env on that session's own line; a chat's file is of the
//! language too, since a file every session of the agent shared would hand
//! each the last writer's language.

use std::path::{Path, PathBuf};

use crate::session_locale::SessionLocale;

/// The binary an agent runs to reach devpit.
pub(crate) fn exe() -> Option<PathBuf> {
    match std::env::var_os("APPIMAGE") {
        Some(appimage) if !appimage.is_empty() => Some(PathBuf::from(appimage)),
        _ => std::env::current_exe().ok(),
    }
}

/// The MCP file a chat turn under `profile` is handed.
pub(crate) fn chat_mcp(locale: SessionLocale, profile: &devpit_rpc::Profile) -> Option<String> {
    let agent = crate::restoring::base_or(Some(&profile.base), &profile.id);
    chat_mcp_in(
        locale,
        &devpit_core::Store::root().ok()?,
        &exe()?,
        &profile.driver,
        agent,
    )
}

/// Claude's own file, written with the terminal's flags so both reach the
/// same server; for Codex and an ACP agent, a file of devpit's that their
/// wire reads back, signed with the agent and of the session's language.
/// Only ever under devpit's `root`.
pub(crate) fn chat_mcp_in(
    locale: SessionLocale,
    root: &Path,
    exe: &Path,
    driver: &str,
    agent: &str,
) -> Option<String> {
    let config = match driver {
        "claude" => {
            let config = root.join("mcp.json");
            mcp_flags(locale, driver, exe, &config)?;
            config
        }
        "codex" | "acp" if devpit_pty::agents::known(agent).is_some() => {
            // Named with the language: the language is a value of the
            // session, and one file per agent would hand every session the
            // last writer's.
            let config = root.join(format!("mcp-{agent}-{}.json", locale.tag()));
            let file = serde_json::json!({ "mcpServers": { "devpit": server(locale, exe.to_str()?, agent) } });
            kept(&config, &file.to_string())?;
            config
        }
        _ => return None,
    };
    Some(config.display().to_string())
}

/// devpit's server for an agent that waits on a tool only so long, speaking
/// the session's language.
pub(crate) fn server(locale: SessionLocale, exe: &str, agent: &str) -> serde_json::Value {
    let timeout = u64::try_from(tool_timeout().as_millis()).unwrap_or(u64::MAX);
    let mut env = serde_json::Map::new();
    env.insert("DEVPIT_AGENT_ID".to_owned(), serde_json::json!(agent));
    let (name, value) = locale.var();
    env.insert(name, serde_json::json!(value));
    serde_json::json!({
        "command": exe,
        "args": ["mcp"],
        "env": env,
        "timeout": timeout,
    })
}

/// [`server`] for Gemini, which expands the pane devpit's terminal set. Its
/// defaults file is every Gemini session's, so no session's language is baked
/// into it: the shell fills the variable in, from the environment the
/// session's own line set.
pub(crate) fn gemini_server(locale: SessionLocale, exe: &str) -> serde_json::Value {
    let mut server = server(locale, exe, "gemini");
    server["env"]["DEVPIT_PANE"] = serde_json::json!("${DEVPIT_PANE}");
    server["env"][crate::session_locale::ENV] = serde_json::json!("${DEVPIT_LOCALE}");
    server
}

/// `path` holding `wanted`, written privately and only when it differs.
fn kept(path: &Path, wanted: &str) -> Option<()> {
    if std::fs::read_to_string(path).ok().as_deref() != Some(wanted) {
        devpit_core::home::write_private(path, wanted.as_bytes()).ok()?;
    }
    Some(())
}

/// What to add to an agent's launch line so it has devpit's tools, or
/// nothing for an agent with no per-session way to be given them. The
/// session's language goes with them where the CLI leaves no other way.
///
/// `config` is where Claude's MCP file goes; it is written here, and only
/// when it differs. The language never goes in it: the file is every Claude
/// session's, and Claude hands a stdio server its own environment, which the
/// variable on the session's line already set.
pub(crate) fn mcp_flags(
    locale: SessionLocale,
    agent: &str,
    exe: &Path,
    config: &Path,
) -> Option<String> {
    let exe = exe.to_str()?;
    // Windows reads forward slashes as well, and TOML reads a backslash as
    // an escape.
    #[cfg(windows)]
    let exe: &str = &exe.replace('\\', "/");
    // Quoted into a shell line and, for Codex, into TOML inside it: a path
    // with either quote in it is not one this can carry safely.
    if exe.contains(['\'', '"', '\\']) {
        return None;
    }
    match agent {
        "claude" => {
            kept(config, &claude_config(exe, agent))?;
            let path = config.to_str()?;
            // `=`, because the flag takes several files: written as two words
            // it swallowed whatever came after it, a subcommand included.
            (!path.contains('\'')).then(|| format!("--mcp-config='{path}'"))
        }
        // Codex gives up on a tool at sixty seconds; a card's checks and
        // review run up to twenty minutes, and were run again on a retry.
        // It hands a server only the variables it is told to: the pane is
        // what signs a session's message, and the language — baked here, on
        // this session's own line — is what it speaks.
        "codex" => Some(format!(
            "-c 'mcp_servers.devpit.command=\"{exe}\"' -c 'mcp_servers.devpit.args=[\"mcp\"]' \
             -c 'mcp_servers.devpit.env={{DEVPIT_AGENT_ID=\"codex\", DEVPIT_LOCALE=\"{}\"}}' \
             -c 'mcp_servers.devpit.env_vars=[\"DEVPIT_PANE\"]' \
             -c 'mcp_servers.devpit.tool_timeout_sec={}'",
            locale.tag(),
            tool_timeout().as_secs()
        )),
        _ => None,
    }
}

/// How long an agent waits on a devpit tool: past the longest the server
/// itself waits, so the answer, or devpit's own "still working", always arrives.
pub(crate) fn tool_timeout() -> std::time::Duration {
    crate::agent_api::METHODS
        .iter()
        .map(|method| devpit_agentapi::client::wait_for(method))
        .max()
        .unwrap_or_default()
        + std::time::Duration::from_secs(60)
}

/// Claude's `--mcp-config` file: devpit as a stdio server, told which agent
/// it serves so a comment is signed with it.
pub(crate) fn claude_config(exe: &str, agent: &str) -> String {
    serde_json::json!({
        "mcpServers": {
            "devpit": {
                "type": "stdio",
                "command": exe,
                "args": ["mcp"],
                "env": { "DEVPIT_AGENT_ID": agent },
            }
        }
    })
    .to_string()
}

/// `<root>/bin/devpit-agent`, a two-line script that runs `devpit agent`.
///
/// Not `devpit`: that name is the installed app itself, on the person's own
/// PATH, and a terminal where `devpit` meant something else would be a trap.
pub(crate) fn cli_shim(root: &Path, exe: &Path) -> Option<PathBuf> {
    let (name, wanted) = shim(exe.to_str()?)?;
    let dir = root.join("bin");
    let path = dir.join(name);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(wanted.as_str()) {
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::write(&path, &wanted).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).ok()?;
        }
    }
    Some(dir)
}

/// The shim's file name and what it says.
#[cfg(not(windows))]
fn shim(exe: &str) -> Option<(&'static str, String)> {
    if exe.contains('\'') {
        return None;
    }
    let wanted = format!("#!/bin/sh\nexec '{exe}' agent \"$@\"\n");
    Some(("devpit-agent", wanted))
}

/// A batch file on Windows, which PATHEXT finds as `devpit-agent` from cmd
/// and PowerShell alike. A `%` in the path would be expanded, a `"` would end
/// the quoting: neither can be carried.
#[cfg(windows)]
fn shim(exe: &str) -> Option<(&'static str, String)> {
    if exe.contains(['"', '%']) {
        return None;
    }
    Some((
        "devpit-agent.cmd",
        format!("@echo off\r\n\"{exe}\" agent %*\r\n"),
    ))
}

#[cfg(test)]
#[path = "agent_reach_tests.rs"]
mod tests;
