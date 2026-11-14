//! devpit's MCP file, read back for a CLI that takes its servers some other
//! way than a file: Codex as config overrides on its command line, an ACP
//! agent in the request that opens its session. Nothing is written to the
//! CLI's own configuration either way.

use std::io::Read;
use std::path::Path;

use serde_json::{json, Value};

/// More than a file of a few servers needs: anything bigger is not read.
const LARGEST: u64 = 64 * 1024;

/// One stdio server, as the file's `mcpServers` names it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Server {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// How long the CLI waits on one of its tools, in milliseconds.
    pub timeout_ms: Option<u64>,
}

/// The servers in the file at `path`; none when it cannot be read.
pub(crate) fn read(path: &Path) -> Vec<Server> {
    let mut text = String::new();
    let read = std::fs::File::open(path)
        .and_then(|file| file.take(LARGEST).read_to_string(&mut text))
        .is_ok();
    if read {
        parse(&text)
    } else {
        Vec::new()
    }
}

/// The stdio servers of an `mcpServers` object. A name that could not be a
/// TOML key, or a server without a command, is left out.
pub(crate) fn parse(text: &str) -> Vec<Server> {
    let Ok(file) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let Some(servers) = file["mcpServers"].as_object() else {
        return Vec::new();
    };
    servers
        .iter()
        .filter(|(name, _)| bare(name))
        .filter_map(|(name, server)| {
            let strings = |value: &Value| -> Vec<String> {
                value
                    .as_array()
                    .map(|all| {
                        all.iter()
                            .filter_map(|one| one.as_str())
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let env = server["env"]
                .as_object()
                .map(|all| {
                    all.iter()
                        .filter(|(key, _)| bare(key))
                        .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default();
            Some(Server {
                name: name.clone(),
                command: server["command"]
                    .as_str()
                    .filter(|one| !one.is_empty())?
                    .to_owned(),
                args: strings(&server["args"]),
                env,
                timeout_ms: server["timeout"].as_u64(),
            })
        })
        .collect()
}

/// Letters, digits, `_` and `-`: a TOML bare key, and an environment name.
fn bare(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The servers as `codex -c key=value` pairs, each value TOML. A JSON string
/// is a TOML basic string, which is what carries a path with any character.
pub(crate) fn codex_overrides(servers: &[Server]) -> Vec<String> {
    let mut argv = Vec::new();
    for server in servers {
        let key = format!("mcp_servers.{}", server.name);
        let env: Vec<String> = server
            .env
            .iter()
            .map(|(name, value)| format!("{name}={}", json!(value)))
            .collect();
        let mut set = vec![
            format!("{key}.command={}", json!(server.command)),
            format!("{key}.args={}", json!(server.args)),
            format!("{key}.env={{{}}}", env.join(", ")),
        ];
        if let Some(ms) = server.timeout_ms {
            set.push(format!("{key}.tool_timeout_sec={}", ms.div_ceil(1000)));
        }
        for one in set {
            argv.push("-c".to_owned());
            argv.push(one);
        }
    }
    argv
}

/// The servers as an ACP session request's `mcpServers`.
pub(crate) fn acp(servers: &[Server]) -> Value {
    servers
        .iter()
        .map(|server| {
            let env: Vec<Value> = server
                .env
                .iter()
                .map(|(name, value)| json!({ "name": name, "value": value }))
                .collect();
            json!({ "name": server.name, "command": server.command, "args": server.args, "env": env })
        })
        .collect()
}

#[cfg(test)]
#[path = "mcp_servers_tests.rs"]
mod tests;
