//! MCP Apps in the chat: a tool that comes with a page shows it under its
//! call, and the page may call its own server's tools back.
//!
//! The CLI does the MCP side (see `devpit_agentcli::apps_host`): a process of
//! it per account and folder, kept a while, answers which tools have a page,
//! hands over a page, and runs a page's calls. What devpit adds is where a page
//! runs: its own scheme, `mcpapp:`, so it has an origin of its own and none of
//! the window's; the policy its server declared, and nothing wider; and an
//! iframe sandbox that keeps it from the window's scripts and devpit's IPC.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use devpit_agentcli::apps_host::{app_tools, AppTool, AppsHost, HostOf};
use devpit_rpc::{ErrorCode, RpcError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use specta::Type;
use tauri::{AppHandle, Manager};

/// A host nobody has asked anything of for this long is let go.
const IDLE: Duration = Duration::from_secs(600);
/// How long the tool list is trusted before it is asked again.
const FRESH: Duration = Duration::from_secs(300);
/// Starting a host connects every server of the account; that is slow.
const FIRST_ANSWER: Duration = Duration::from_secs(60);
const ANSWER: Duration = Duration::from_secs(45);
/// Pages kept to be served. One per tool, account and folder, so a chat
/// drawing a page again reuses it; the one least recently asked for goes.
const PAGES: usize = 40;

struct Held {
    host: Arc<AppsHost>,
    tools: Option<(Instant, Vec<AppTool>)>,
    used: Instant,
}

/// The account, the folder and the tool a page came with.
type PageOf = (String, String, String);

struct Page {
    id: String,
    of: PageOf,
    html: String,
    policy: String,
    bordered: bool,
}

#[derive(Default)]
pub(crate) struct McpApps {
    hosts: Mutex<HashMap<(String, String), Held>>,
    pages: Mutex<Vec<Page>>,
    /// Whether the thread that lets idle hosts go is running.
    reaping: std::sync::atomic::AtomicBool,
}

/// Lets go of every host nobody has asked anything of for a while, and of any
/// that already stopped. Each one is a whole CLI with every server of its
/// account connected: left, they stayed until devpit quit.
fn reap(hosts: &mut HashMap<(String, String), Held>) {
    hosts.retain(|_, held| {
        let keep = held.host.alive() && held.used.elapsed() < IDLE;
        if !keep {
            held.host.close();
        }
        keep
    });
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpAppTool {
    /// The tool as the model calls it: `mcp__<server>__<tool>`.
    pub called: String,
    pub server: String,
    pub bordered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpAppPage {
    /// Its id on the `mcpapp:` scheme.
    pub id: String,
    pub bordered: bool,
}

/// `mcp.appTools` — the tools of this account, in this folder, that come
/// with a page.
#[tauri::command]
#[specta::specta]
pub async fn mcp_app_tools(
    app: AppHandle,
    profile_id: String,
    cwd: String,
) -> Result<Vec<McpAppTool>, RpcError> {
    crate::off_main::blocking(move || {
        Ok(tools(&app, &profile_id, &cwd)?
            .into_iter()
            .map(|one| McpAppTool {
                called: one.called,
                server: one.server,
                bordered: one.bordered,
            })
            .collect())
    })
    .await
}

/// `mcp.appOpen` — the page that comes with a tool, ready to be served.
#[tauri::command]
#[specta::specta]
pub async fn mcp_app_open(
    app: AppHandle,
    profile_id: String,
    cwd: String,
    called: String,
) -> Result<McpAppPage, RpcError> {
    crate::off_main::blocking(move || {
        let of = (profile_id.clone(), cwd.clone(), called.clone());
        // A chat drawn again — a remount, a scroll back — asks for the page it
        // already had; reading it again could start a CLI process.
        if let Some(kept) = app.state::<McpApps>().kept(&of) {
            return Ok(kept);
        }
        let tool = tools(&app, &profile_id, &cwd)?
            .into_iter()
            .find(|one| one.called == called)
            .ok_or_else(|| RpcError::new(ErrorCode::NotFound, format!("{called} has no page")))?;
        let host = host(&app, &profile_id, &cwd)?;
        let read = host
            .ask(
                json!({ "subtype": "mcp_read_resource", "serverName": tool.server, "uri": tool.uri }),
                ANSWER,
            )
            .map_err(|why| RpcError::new(ErrorCode::Busy, why))?;
        let content = &read["contents"][0];
        let html = content["text"]
            .as_str()
            .ok_or_else(|| RpcError::new(ErrorCode::Busy, "the server sent no page"))?
            .to_owned();
        // The page's own policy, else the one its tool declared.
        let declared = match &content["_meta"]["ui"]["csp"] {
            Value::Null => &tool.csp,
            said => said,
        };
        let id = ulid::Ulid::generate().to_string().to_lowercase();
        let state = app.state::<McpApps>();
        let mut pages = state.pages.lock().map_err(|_| RpcError::internal("pages"))?;
        Ok(keep(
            &mut pages,
            Page {
                id,
                of,
                html,
                policy: policy(declared),
                bordered: tool.bordered,
            },
        ))
    })
    .await
}

/// `mcp.appCall` — a page calling a tool of its own server, once the person
/// has let it. The answer is the CLI's, as JSON.
#[tauri::command]
#[specta::specta]
pub async fn mcp_app_call(
    app: AppHandle,
    profile_id: String,
    cwd: String,
    called: String,
    tool: String,
    input: String,
) -> Result<String, RpcError> {
    crate::off_main::blocking(move || {
        let from = tools(&app, &profile_id, &cwd)?
            .into_iter()
            .find(|one| one.called == called)
            .ok_or_else(|| RpcError::new(ErrorCode::NotFound, format!("{called} has no page")))?;
        // A page reaches its own server, and no other.
        let full = format!(
            "mcp__{}__{}",
            devpit_agentcli::apps_host::plain(&from.server),
            tool
        );
        let arguments: Value = serde_json::from_str(&input)
            .map_err(|_| RpcError::new(ErrorCode::Invalid, "the arguments are not JSON"))?;
        let said = host(&app, &profile_id, &cwd)?
            .ask(
                json!({ "subtype": "mcp_call", "tool": full, "arguments": arguments }),
                ANSWER,
            )
            .map_err(|why| RpcError::new(ErrorCode::Busy, why))?;
        Ok(said.to_string())
    })
    .await
}

impl McpApps {
    /// The page already kept for this tool, moved to the newest place.
    fn kept(&self, of: &PageOf) -> Option<McpAppPage> {
        let mut pages = self.pages.lock().ok()?;
        let at = pages.iter().position(|page| &page.of == of)?;
        let page = pages.remove(at);
        let shown = McpAppPage {
            id: page.id.clone(),
            bordered: page.bordered,
        };
        pages.push(page);
        Some(shown)
    }
}

/// Keeps `page` as the newest, in place of one for the same tool, and lets
/// the least recently asked for go past [`PAGES`].
fn keep(pages: &mut Vec<Page>, page: Page) -> McpAppPage {
    pages.retain(|one| one.of != page.of);
    let shown = McpAppPage {
        id: page.id.clone(),
        bordered: page.bordered,
    };
    pages.push(page);
    let over = pages.len().saturating_sub(PAGES);
    pages.drain(..over);
    shown
}

/// A page, as the `mcpapp:` scheme serves it: with its policy as a header.
pub(crate) fn serve(app: &AppHandle, path: &str) -> tauri::http::Response<Vec<u8>> {
    let id = path.trim_start_matches('/');
    let found = app.try_state::<McpApps>().and_then(|state| {
        state.pages.lock().ok().and_then(|mut pages| {
            // Served is shown: the newest place, so it is not the next to go.
            let at = pages.iter().position(|page| page.id == id)?;
            let page = pages.remove(at);
            let found = (page.html.clone(), page.policy.clone());
            pages.push(page);
            Some(found)
        })
    });
    let builder = tauri::http::Response::builder()
        .header("X-Content-Type-Options", "nosniff")
        .header("Referrer-Policy", "no-referrer");
    match found {
        Some((html, policy)) => builder
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Content-Security-Policy", policy)
            .body(html.into_bytes()),
        None => builder.status(404).body(b"gone".to_vec()),
    }
    .unwrap_or_default()
}

fn tools(app: &AppHandle, profile_id: &str, cwd: &str) -> Result<Vec<AppTool>, RpcError> {
    let key = (profile_id.to_owned(), cwd.to_owned());
    let state = app.state::<McpApps>();
    if let Some(fresh) = state.hosts.lock().ok().and_then(|hosts| {
        hosts
            .get(&key)
            .and_then(|held| held.tools.as_ref())
            .filter(|(at, _)| at.elapsed() < FRESH)
            .map(|(_, tools)| tools.clone())
    }) {
        return Ok(fresh);
    }
    let started = host(app, profile_id, cwd)?;
    let status = started
        .ask(json!({ "subtype": "mcp_status" }), FIRST_ANSWER)
        .map_err(|why| RpcError::new(ErrorCode::Busy, why))?;
    let found = app_tools(&status);
    if let Ok(mut hosts) = state.hosts.lock() {
        if let Some(held) = hosts.get_mut(&key) {
            held.tools = Some((Instant::now(), found.clone()));
        }
    }
    Ok(found)
}

/// The host for this account and folder, started when there is none.
fn host(app: &AppHandle, profile_id: &str, cwd: &str) -> Result<Arc<AppsHost>, RpcError> {
    // Only a folder the window may open: a project's, or devpit's own.
    let folder = crate::reveal::allowed(cwd)?;
    let state = app.state::<McpApps>();
    let key = (profile_id.to_owned(), cwd.to_owned());
    {
        let mut hosts = state
            .hosts
            .lock()
            .map_err(|_| RpcError::internal("hosts"))?;
        reap(&mut hosts);
        if !state
            .reaping
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            let app = app.clone();
            // Once a minute, for as long as any host is kept.
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(60));
                let state = app.state::<McpApps>();
                let Ok(mut hosts) = state.hosts.lock() else {
                    return;
                };
                reap(&mut hosts);
                if hosts.is_empty() {
                    state
                        .reaping
                        .store(false, std::sync::atomic::Ordering::SeqCst);
                    return;
                }
            });
        }
        if let Some(held) = hosts.get_mut(&key) {
            held.used = Instant::now();
            return Ok(held.host.clone());
        }
    }
    // Started without the lock: starting one connects every server of the
    // account, and every other page's call used to wait on it.
    let (profile, path) = crate::chat_turn::spawnable(profile_id)?;
    let env = devpit_agentcli::running::runner(&profile).env;
    let mcp = crate::agent_reach::chat_mcp(&profile);
    let started = AppsHost::start(&HostOf {
        command: &path,
        env: &env,
        cwd: &folder,
        mcp_config: mcp.as_deref(),
    })
    .map_err(|why| RpcError::new(ErrorCode::Busy, why))?;
    let started = Arc::new(started);
    let mut hosts = state
        .hosts
        .lock()
        .map_err(|_| RpcError::internal("hosts"))?;
    // Another call started one meanwhile: the one kept is the one used.
    if let Some(held) = hosts.get_mut(&key) {
        started.close();
        held.used = Instant::now();
        return Ok(held.host.clone());
    }
    hosts.insert(
        key,
        Held {
            host: started.clone(),
            tools: None,
            used: Instant::now(),
        },
    );
    Ok(started)
}

/// The page's Content-Security-Policy: nothing but inline code, and the
/// hosts its server named — each checked to be a plain origin.
pub(crate) fn policy(declared: &Value) -> String {
    let listed = |key: &str| -> String {
        declared[key]
            .as_array()
            .map(|all| {
                all.iter()
                    .filter_map(Value::as_str)
                    .filter(|one| origin(one))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default()
    };
    let resources = listed("resourceDomains");
    let connect = listed("connectDomains");
    let frames = listed("frameDomains");
    let or_none = |list: &str| {
        if list.is_empty() {
            "'none'".to_owned()
        } else {
            list.to_owned()
        }
    };
    format!(
        "default-src 'none'; script-src 'unsafe-inline' {resources}; style-src 'unsafe-inline' {resources}; \
         img-src data: blob: {resources}; font-src data: {resources}; media-src {media}; connect-src {connect}; \
         frame-src {frames}; base-uri 'none'; form-action 'none'",
        media = or_none(&resources),
        connect = or_none(&connect),
        frames = or_none(&frames),
    )
}

/// `https://host`, `https://*.host`, or `wss://host`, with a port at most:
/// nothing that could close the directive or widen it.
fn origin(said: &str) -> bool {
    let Some(rest) = said
        .strip_prefix("https://")
        .or_else(|| said.strip_prefix("wss://"))
    else {
        return false;
    };
    let host = rest.strip_prefix("*.").unwrap_or(rest);
    !host.is_empty()
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | ':'))
}

#[cfg(test)]
#[path = "mcp_apps_tests.rs"]
mod tests;
