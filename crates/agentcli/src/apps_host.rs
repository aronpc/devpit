//! A process of the CLI kept to serve MCP Apps: the tools that come with a
//! page, the page itself, and the calls the page makes.
//!
//! Claude Code hosts MCP Apps behind `CLAUDE_CODE_MCP_APPS_HOST=1`
//! (measured on 2.1.282): it then tells servers it can show their pages, and
//! answers three control requests without a model turn — `mcp_status` (each
//! tool's `_meta.ui`), `mcp_read_resource` (a `ui://` page) and `mcp_call` (a
//! tool, called directly). The connections, and their sign-ins, are the
//! CLI's; devpit holds none of its own.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::control::Control;

/// The switch that makes the CLI a host of MCP Apps.
pub const APPS_HOST_ENV: (&str, &str) = ("CLAUDE_CODE_MCP_APPS_HOST", "1");

type Waiting = Arc<Mutex<HashMap<String, mpsc::Sender<Value>>>>;

/// The process, and the answers it owes.
pub struct AppsHost {
    control: Control,
    waiting: Waiting,
    alive: Arc<std::sync::atomic::AtomicBool>,
    next: std::sync::atomic::AtomicU64,
}

/// What it is started with: the account's program and environment, where,
/// and devpit's own MCP file so devpit's pages are among them.
pub struct HostOf<'a> {
    pub command: &'a str,
    pub env: &'a [(String, String)],
    pub cwd: &'a std::path::Path,
    pub mcp_config: Option<&'a str>,
}

impl AppsHost {
    pub fn start(of: &HostOf<'_>) -> Result<Self, String> {
        let mut argv = vec![
            "--print",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        if let Some(config) = of.mcp_config {
            argv.push(format!("--mcp-config={config}"));
        }
        let mut child = devpit_pty::host_env::command(of.command)
            .args(&argv)
            .current_dir(of.cwd)
            .envs(of.env.iter().map(|(key, value)| (key, value)))
            .env(APPS_HOST_ENV.0, APPS_HOST_ENV.1)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("{} did not start: {err}", of.command))?;
        let control = Control::new();
        control.attach(child.stdin.take().ok_or("no stdin")?);
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let waiting: Waiting = Arc::default();
        let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let (owed, living) = (waiting.clone(), alive.clone());
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if !line.contains("\"control_response\"") {
                    continue;
                }
                let Ok(said) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                let response = said["response"].clone();
                let id = response["request_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                if let Some(tell) = owed.lock().ok().and_then(|mut all| all.remove(&id)) {
                    let _ = tell.send(response);
                }
            }
            living.store(false, std::sync::atomic::Ordering::SeqCst);
            let _ = child.wait();
        });
        Ok(Self {
            control,
            waiting,
            alive,
            next: std::sync::atomic::AtomicU64::new(1),
        })
    }

    pub fn alive(&self) -> bool {
        self.alive.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// One control request, answered or refused within `wait`: the answer's
    /// `response`, or the CLI's error.
    pub fn ask(&self, request: Value, wait: Duration) -> Result<Value, String> {
        let id = format!(
            "app-{}",
            self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        let (tell, heard) = mpsc::channel();
        self.waiting
            .lock()
            .map_err(|_| "the host is not answering")?
            .insert(id.clone(), tell);
        let line = json!({ "type": "control_request", "request_id": id, "request": request });
        if !self.control.write(&line.to_string()) {
            return Err("the host has stopped".to_owned());
        }
        let said = heard
            .recv_timeout(wait)
            .map_err(|_| "the host did not answer in time".to_owned());
        if said.is_err() {
            if let Ok(mut all) = self.waiting.lock() {
                all.remove(&id);
            }
        }
        answered(said?)
    }

    /// Lets it exit.
    pub fn close(&self) {
        self.control.close();
    }
}

/// A control answer's payload, or the error it carries.
pub(crate) fn answered(said: Value) -> Result<Value, String> {
    match said["subtype"].as_str() {
        Some("error") => Err(said["error"]
            .as_str()
            .unwrap_or("the CLI refused")
            .to_owned()),
        _ => Ok(said["response"].clone()),
    }
}

/// The tools that come with a page, by the name the model calls them:
/// `mcp__<server>__<tool>`, as the CLI names MCP tools.
pub fn app_tools(status: &Value) -> Vec<AppTool> {
    let servers = status["mcpServers"]
        .as_array()
        .or_else(|| status.as_array())
        .cloned()
        .unwrap_or_default();
    servers
        .iter()
        .flat_map(|server| {
            let name = server["name"].as_str().unwrap_or_default().to_owned();
            server["tools"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(move |tool| {
                    let ui = &tool["_meta"]["ui"];
                    let uri = ui["resourceUri"]
                        .as_str()
                        .or_else(|| tool["_meta"]["ui/resourceUri"].as_str())?;
                    let tool_name = tool["name"].as_str()?;
                    Some(AppTool {
                        called: format!("mcp__{}__{}", plain(&name), tool_name),
                        server: name.clone(),
                        tool: tool_name.to_owned(),
                        uri: uri.to_owned(),
                        bordered: ui["prefersBorder"].as_bool().unwrap_or(true),
                        csp: ui["csp"].clone(),
                    })
                })
        })
        .collect()
}

/// A server's name as it appears inside a tool's: anything but a letter, a
/// digit, `_` or `-` becomes `_`.
pub fn plain(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AppTool {
    /// How the model calls it.
    pub called: String,
    pub server: String,
    pub tool: String,
    pub uri: String,
    pub bordered: bool,
    /// The page's `_meta.ui.csp`, as the server declared it.
    pub csp: Value,
}

#[cfg(test)]
#[path = "apps_host_tests.rs"]
mod tests;
