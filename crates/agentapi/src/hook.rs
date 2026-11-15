//! `devpit hook`: an agent's hook, posted to the running app.
//!
//! The Windows form of the line `devpit_agentcli`'s hook settings write
//! elsewhere — `cat`, `curl` and POSIX `${VAR:+…}`, none of which Windows can
//! be counted on to have. It keeps that line's promises: the endpoint and the
//! secret are read from disk on every call, a devpit that has gone away costs
//! the agent a moment rather than a hang, and it always exits 0, because a
//! hook that fails must not fail the turn it reports on.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use crate::client;

/// The variable a devpit terminal names its pane in, as `devpit_tmux` sets it.
pub const PANE_ENV: &str = "DEVPIT_PANE";

/// The conversation a chat's process is started in, as
/// `devpit_agentcli::CHAT_ENV` names it, and the header it travels in.
pub const CHAT_ENV: &str = "DEVPIT_CHAT";
pub const CHAT_HEADER: &str = "x-devpit-chat";

/// Set in a session launched with devpit's own hook settings
/// (`devpit_agentcli::HOOKED`), where the plugin's copy stays quiet.
pub const HOOKED_ENV: &str = "DEVPIT_HOOKED";

/// The most a hook's report is read up to: as the listener takes from a hook
/// that shows its secret. It is cut down before it is sent (`slimmed`), so
/// what crosses is what devpit reads.
const MOST_BYTES: usize = 16 * 1024 * 1024;

/// A reply is a decision or nothing; anything longer is not one of ours.
const MOST_REPLY: u64 = 64 * 1024;

/// How long reaching the app may take, as `curl --connect-timeout`.
const CONNECT: Duration = Duration::from_millis(500);

/// What one hook was asked to do.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Asked {
    /// The whole budget, as `curl --max-time`.
    pub wait: Duration,
    /// Print the reply: only the hook that can be answered does.
    pub echo: bool,
    /// The plugin's copy, which speaks only inside a devpit pane and not
    /// beside the settings' own.
    pub plugin: bool,
    /// The agent the report is from, when it is not Claude Code: the listener
    /// puts it into Claude Code's words.
    pub from: Option<String>,
}

/// Runs one hook and answers with its exit code, which is always 0.
pub fn run(root: &Path, args: &[String]) -> i32 {
    let asked = parsed(args);
    let pane = std::env::var(PANE_ENV).unwrap_or_default();
    let hooked = std::env::var(HOOKED_ENV).unwrap_or_default();
    if !speaks(&asked, &pane, &hooked) {
        return 0;
    }
    let mut body = Vec::new();
    let read = std::io::stdin()
        .lock()
        .take(MOST_BYTES as u64 + 1)
        .read_to_end(&mut body);
    if read.is_err() || body.len() > MOST_BYTES {
        return 0;
    }
    let body = slim(body);
    if let Some(reply) = post(root, &body, &pane, asked.from.as_deref(), asked.wait) {
        if asked.echo {
            let _ = std::io::stdout().write_all(&reply);
        }
    }
    0
}

/// A report cut down to what devpit reads, when it is big enough to matter:
/// a Read's whole file and a command's whole output stay on this side.
pub(crate) fn slim(body: Vec<u8>) -> Vec<u8> {
    if body.len() <= crate::SLIM_ABOVE {
        return body;
    }
    std::str::from_utf8(&body)
        .ok()
        .and_then(crate::slimmed)
        .map(String::into_bytes)
        .unwrap_or(body)
}

/// `--wait <seconds>`, `--echo`, `--plugin`, `--from <agent>`; anything else is ignored, so a
/// newer settings file never makes an older binary fail a hook.
pub(crate) fn parsed(args: &[String]) -> Asked {
    let mut asked = Asked {
        wait: Duration::from_millis(1500),
        echo: false,
        plugin: false,
        from: None,
    };
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--echo" => asked.echo = true,
            "--plugin" => asked.plugin = true,
            // A word, as the query takes it: nothing else rides into the URL.
            "--from" => {
                asked.from = words
                    .next()
                    .filter(|agent| {
                        !agent.is_empty() && agent.chars().all(|c| c.is_ascii_alphanumeric())
                    })
                    .cloned();
            }
            "--wait" => {
                if let Some(seconds) = words.next().and_then(|one| one.parse::<f64>().ok()) {
                    asked.wait = Duration::from_secs_f64(seconds.clamp(0.1, 600.0));
                }
            }
            _ => {}
        }
    }
    asked
}

/// Whether this hook posts at all: the plugin's copy only inside a devpit
/// pane, and only where the launch line did not bring the same hooks.
pub(crate) fn speaks(asked: &Asked, pane: &str, hooked: &str) -> bool {
    !asked.plugin || (!pane.is_empty() && hooked.is_empty())
}

/// Posts `body` to the app's hook route, naming `pane` when there is one, and
/// answers with the reply's body — or nothing, when the app could not be
/// reached in time.
pub(crate) fn post(
    root: &Path,
    body: &[u8],
    pane: &str,
    from: Option<&str>,
    wait: Duration,
) -> Option<Vec<u8>> {
    let (address, secret) = client::door(root).ok()?;
    let endpoint = std::fs::read_to_string(root.join("hook-endpoint")).ok()?;
    let route = route_of(endpoint.trim())?;
    let query = query_of(pane, from);
    let to: SocketAddr = address.parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&to, CONNECT).ok()?;
    stream.set_read_timeout(Some(wait)).ok()?;
    stream.set_write_timeout(Some(wait)).ok()?;
    let chat = chat_line(&std::env::var(CHAT_ENV).unwrap_or_default());
    let head = format!(
        "POST {route}{query} HTTP/1.1\r\nhost: {address}\r\ncontent-type: application/json\r\n{secret}\r\n{chat}content-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(body).ok()?;
    let mut reply = Vec::new();
    stream.take(MOST_REPLY).read_to_end(&mut reply).ok()?;
    let at = reply.windows(4).position(|four| four == b"\r\n\r\n")?;
    Some(reply.split_off(at + 4))
}

/// `?from=<agent>&pane=<pane>`, each when there is one.
pub(crate) fn query_of(pane: &str, from: Option<&str>) -> String {
    let pairs: Vec<String> = from
        .map(|agent| format!("from={agent}"))
        .into_iter()
        .chain((!pane.is_empty()).then(|| format!("pane={pane}")))
        .collect();
    if pairs.is_empty() {
        String::new()
    } else {
        format!("?{}", pairs.join("&"))
    }
}

/// The header naming the chat whose process this hook runs in, or nothing
/// outside one. Only a name of ours goes into the request.
pub(crate) fn chat_line(chat: &str) -> String {
    let ours = !chat.is_empty()
        && chat.len() <= 64
        && chat
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    match ours {
        true => format!("{CHAT_HEADER}: {chat}\r\n"),
        false => String::new(),
    }
}

/// `/hook` out of `http://host:port/hook`.
pub(crate) fn route_of(endpoint: &str) -> Option<String> {
    let rest = endpoint.strip_prefix("http://")?;
    let (_, route) = rest.split_once('/')?;
    Some(format!("/{route}"))
}

#[cfg(test)]
#[path = "hook_tests.rs"]
mod tests;
