use std::io::Read;

use super::{bounded_lines, with_ceiling, MOST_LINE_BYTES};

#[test]
fn lines_within_the_ceiling_come_out_whole() {
    let (lines, overflow) = with_ceiling(&b"one\ntwo\r\nlast"[..], 16);
    assert_eq!(lines.collect::<Vec<_>>(), ["one", "two", "last"]);
    assert!(!overflow.happened());
}

#[test]
fn a_line_exactly_at_the_ceiling_passes() {
    let (lines, overflow) = with_ceiling(&b"12345678\nnext\n"[..], 8);
    assert_eq!(lines.collect::<Vec<_>>(), ["12345678", "next"]);
    assert!(!overflow.happened());
}

#[test]
fn a_line_past_the_ceiling_ends_the_stream_and_says_so() {
    let (mut lines, overflow) = with_ceiling(&b"ok\n123456789\nnever\n"[..], 8);
    assert_eq!(lines.next().as_deref(), Some("ok"));
    assert_eq!(lines.next(), None);
    assert!(overflow.happened());
    assert_eq!(lines.next(), None);
}

/// A source that never ends and never has a newline: what a hostile or broken
/// CLI looks like. The reader must stop at the ceiling, not read it all, and
/// must not hold more than the ceiling while it does.
struct Endless {
    served: usize,
}

impl Read for Endless {
    fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
        into.fill(b'x');
        self.served += into.len();
        Ok(into.len())
    }
}

#[test]
fn an_endless_line_is_cut_at_the_ceiling() {
    let ceiling = 1024 * 1024;
    let mut source = Endless { served: 0 };
    let (mut lines, overflow) = with_ceiling(&mut source, ceiling);
    assert_eq!(lines.next(), None);
    assert!(overflow.happened());
    drop(lines);
    // Read no further than the ceiling plus one buffer.
    assert!(source.served <= ceiling + 64 * 1024, "{}", source.served);
}

#[test]
fn the_ceiling_leaves_room_for_a_large_real_line() {
    const { assert!(MOST_LINE_BYTES >= 16 * 1024 * 1024) };
    let (mut lines, _) = bounded_lines(&b"{}\n"[..]);
    assert_eq!(lines.next().as_deref(), Some("{}"));
}
