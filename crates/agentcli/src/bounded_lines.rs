//! The CLI's stdout, line by line, with a ceiling on one line.
//!
//! `BufRead::lines` grows its `String` until it finds a newline, so a single
//! line without one can use all the memory of the process before anything
//! looks at it. This reader stops at the ceiling instead: it holds at most
//! that many bytes, ends the stream there, and says so through a flag the
//! caller checks to end the process and report the turn as failed.

use std::io::{BufRead, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The most one stdout line of the CLI may carry. A tool result with an image
/// in it runs to a few megabytes, so this leaves room well past any real line.
pub(crate) const MOST_LINE_BYTES: usize = 32 * 1024 * 1024;

/// Set when a line passed the ceiling; the stream ends there.
#[derive(Clone, Default)]
pub(crate) struct Overflow(Arc<AtomicBool>);

impl Overflow {
    pub(crate) fn happened(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// What the turn says when the stream was cut for this.
pub(crate) fn complaint() -> String {
    format!(
        "\n\nThe agent stopped: it wrote a line longer than {} MiB.",
        MOST_LINE_BYTES / (1024 * 1024)
    )
}

pub(crate) struct BoundedLines<R: Read> {
    reader: std::io::BufReader<R>,
    most: usize,
    overflow: Overflow,
}

/// Reads `source` a line at a time, holding at most `MOST_LINE_BYTES` of one.
pub(crate) fn bounded_lines(source: impl Read) -> (BoundedLines<impl Read>, Overflow) {
    with_ceiling(source, MOST_LINE_BYTES)
}

pub(crate) fn with_ceiling<R: Read>(source: R, most: usize) -> (BoundedLines<R>, Overflow) {
    let overflow = Overflow::default();
    (
        BoundedLines {
            reader: std::io::BufReader::new(source),
            most,
            overflow: overflow.clone(),
        },
        overflow,
    )
}

impl<R: Read> Iterator for BoundedLines<R> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let mut line: Vec<u8> = Vec::new();
        loop {
            let chunk = match self.reader.fill_buf() {
                Ok(chunk) => chunk,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            };
            if chunk.is_empty() {
                return (!line.is_empty()).then(|| text(line));
            }
            let (take, done) = match chunk.iter().position(|&byte| byte == b'\n') {
                Some(at) => (at, true),
                None => (chunk.len(), false),
            };
            // Checked before copying: the line never grows past the ceiling.
            if line.len() + take > self.most {
                self.overflow.0.store(true, Ordering::SeqCst);
                return None;
            }
            line.extend_from_slice(&chunk[..take]);
            self.reader.consume(take + usize::from(done));
            if done {
                return Some(text(line));
            }
        }
    }
}

fn text(mut line: Vec<u8>) -> String {
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8(line)
        .unwrap_or_else(|bad| String::from_utf8_lossy(bad.as_bytes()).into_owned())
}

#[cfg(test)]
#[path = "bounded_lines_tests.rs"]
mod tests;
