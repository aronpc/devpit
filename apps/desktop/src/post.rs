//! Reading one POST off a socket, by hand.
//!
//! One route, one method, one caller on loopback. An HTTP crate here would be
//! several thousand lines to parse a request this already refuses to
//! over-read.

use std::io::{BufRead, BufReader};
use std::net::TcpStream;

/// A payload bigger than this is not one of ours.
///
/// A `Stop` carries the last thing the agent said, which can be long, so the
/// ceiling is generous — but a ceiling there is, because this reads from a
/// socket and an unbounded read is a way to spend all the memory on the box.
pub(crate) const MOST_BYTES: usize = 256 * 1024;

/// What a post that showed the right secret may carry. A tool's report can be
/// a whole file — a Read, a command's output — and refusing it lost the report
/// with it: the step stayed running. It is read, cut down to what devpit uses
/// (`devpit_agentapi::slimmed`), and the rest let go.
pub(crate) const MOST_HOOK_BYTES: usize = 16 * 1024 * 1024;

/// A header line longer than this is not one of ours either.
///
/// `read_line` grows its buffer until it meets a newline, so a sender that
/// never sends one chooses how much memory this process spends. The body had a
/// ceiling from the start; the headers above it did not.
pub(crate) const MOST_HEADER_BYTES: usize = 8 * 1024;

/// And there are not a hundred of them.
pub(crate) const MOST_HEADERS: usize = 64;

/// One POST: what it carried, which pane it came from, and the secret it
/// showed at the door.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posted {
    pub body: String,
    /// The pane named in the query, when the hook was fired inside one of our
    /// terminals. Absent for a headless turn, which belongs to a card rather
    /// than to a pane.
    pub pane: Option<String>,
    /// What the `x-devpit-hook` header carried, if anything. Read here and
    /// judged in `listener::authorized`: parsing a request and deciding
    /// whether to trust it are two jobs.
    pub secret: Option<String>,
    /// Posted to `/agent` — a question from an agent (`agent_api`) — rather
    /// than to `/hook`, a report.
    pub agent: bool,
    /// The conversation whose chat process posted, from `x-devpit-chat`;
    /// none for a session started anywhere else.
    pub chat: Option<String>,
}

/// The body of a POST, or nothing.
///
/// Written by hand rather than with an HTTP crate: one route, one method, one
/// caller on loopback. A dependency here would be several thousand lines to
/// parse a request this already refuses to over-read.
pub fn read_request(stream: &mut TcpStream, door: Option<&str>) -> Option<Posted> {
    read_post(BufReader::new(stream.try_clone().ok()?), door)
}

/// The same, from anything that yields bytes.
///
/// Split off the socket so a test can hand it a request instead of opening a
/// port. The ceiling below is the only thing between a `content-length`
/// somebody else wrote and an allocation of exactly that size, and a rule a
/// test cannot call is a rule the test cannot guard.
pub(crate) fn read_post(mut reader: impl BufRead, door: Option<&str>) -> Option<Posted> {
    let mut length = 0usize;
    let mut pane = None;
    let mut secret = None;
    let mut agent = false;
    let mut chat = None;
    let mut gemini = false;
    let mut first = true;
    let mut seen = 0usize;

    loop {
        seen += 1;
        if seen > MOST_HEADERS {
            return None;
        }
        let mut line = String::new();
        // Bounded like the body: `take` caps what one line may cost, and a
        // line that came back without its newline hit that cap rather than
        // ended — which is a request to stop reading, not to grow.
        if std::io::Read::take(reader.by_ref(), MOST_HEADER_BYTES as u64)
            .read_line(&mut line)
            .ok()?
            == 0
        {
            return None;
        }
        if !line.ends_with('\n') {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if first {
            // `POST /hook?pane=leaf_01... HTTP/1.1`. The only line that is
            // not a header, and the only place the pane could travel without
            // touching the JSON the agent itself wrote.
            pane = pane_in(line);
            gemini = query_says(line, "from") == Some("gemini");
            agent = line
                .split_whitespace()
                .nth(1)
                .is_some_and(|target| target == "/agent" || target.starts_with("/agent?"));
            first = false;
            continue;
        }
        // Names are case-insensitive and may be padded either side of the
        // colon; `Content-Length` is how most senders spell it.
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().ok()?,
                devpit_agentcli::HOOK_HEADER => secret = Some(value.trim().to_owned()),
                devpit_agentapi::hook::CHAT_HEADER => chat = ours(value.trim()),
                _ => {}
            }
        }
    }

    // The bigger ceiling only for a post that showed the door's secret, which
    // is known before a byte of the body is read: anyone else gets the small
    // one, and no say in how much this process allocates.
    let ceiling = match door {
        Some(door) if crate::listener::authorized(secret.as_deref(), door) => MOST_HOOK_BYTES,
        _ => MOST_BYTES,
    };
    if length == 0 || length > ceiling {
        return None;
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    let mut body = String::from_utf8(body).ok()?;
    // Gemini CLI's report, put into the words devpit reads — before it is
    // cut, which would take the error a failed tool reports. One devpit does
    // not listen to is left with nothing to say.
    if gemini && !agent {
        body = devpit_agentcli::gemini::claude_shaped(&body).unwrap_or_else(|| "{}".to_owned());
    }
    if !agent && body.len() > devpit_agentapi::SLIM_ABOVE {
        // What is not JSON is not cut, and passes only under the small
        // ceiling, as it always did; the reader after this refuses it.
        match devpit_agentapi::slimmed(&body) {
            Some(slim) => body = slim,
            None if body.len() <= MOST_BYTES => {}
            None => return None,
        }
    }
    Some(Posted {
        body,
        pane,
        secret,
        agent,
        chat,
    })
}

/// The pane named in a request line, if one is.
///
/// Refused unless it looks like one of ours. This arrives from a shell we
/// wrote but through a process we did not, and it goes on to key a map and
/// reach the screen — a value nobody checked is a value somebody else can
/// choose.
fn pane_in(request_line: &str) -> Option<String> {
    ours(query_says(request_line, "pane")?.trim())
}

/// A name one of our own would have — a pane, a conversation — or nothing.
fn ours(named: &str) -> Option<String> {
    let ours = !named.is_empty()
        && named.len() <= 64
        && named
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    ours.then(|| named.to_owned())
}

/// What a request line's query gives `key`, if it gives it anything.
fn query_says<'a>(request_line: &'a str, key: &str) -> Option<&'a str> {
    let target = request_line.split_whitespace().nth(1)?;
    let query = target.split_once('?')?.1;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
}

#[cfg(test)]
#[path = "post_tests.rs"]
mod tests;
