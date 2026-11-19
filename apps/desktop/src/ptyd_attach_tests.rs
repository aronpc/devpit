use std::time::Duration;

use super::*;

/// What `drain` hands on first from `stream`, for `pane`.
fn first_heard(stream: UnixStream, pane: u32) -> Option<Heard> {
    // A daemon that says nothing ends as a plain end, never a hung test.
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let (heard_tx, mut heard) = tokio::sync::mpsc::channel::<Heard>(8);
    std::thread::spawn(move || drain(stream, pane, heard_tx));
    let mut first = heard.blocking_recv();
    // What came before the end, when the end is what is asked about.
    while matches!(first, Some(Heard::Replay(_) | Heard::Folder(_))) {
        first = heard.blocking_recv();
    }
    first
}

/// A pane the daemon no longer holds — a session listed on the pane it ran
/// in before devpit-ptyd restarted — is refused, and the attach hears why.
/// Heard as a plain end, the terminal stayed blank with nothing said.
#[test]
fn an_attach_to_a_pane_the_daemon_does_not_hold_hears_why() {
    let daemon = crate::ptyd_link::tests::serving();
    let mut stream = crate::ptyd_link::connect(daemon.home.path()).expect("connect to the daemon");
    let pane = crate::ptyd_link::pane_number("leaf_gone");
    frame::write(&mut stream, &Frame::new(pane, Kind::Attach, Vec::new())).expect("attach");

    match first_heard(stream, pane) {
        Some(Heard::Refused(why)) => assert_eq!(why, "no such pane"),
        Some(Heard::Ended) => panic!("heard a plain end, not why"),
        Some(_) => panic!("heard something other than a refusal"),
        None => panic!("heard nothing"),
    }
}

/// A refusal of an input or a resize is not of the attach: it ends the
/// attach as it always did, and is not said to be a terminal that is gone —
/// even from a pane that has printed nothing yet.
#[test]
fn a_refusal_of_an_input_is_a_plain_end() {
    let (mut daemon, window) = UnixStream::pair().expect("pair");
    let pane = 7;
    for one in [
        Frame::new(
            pane,
            Kind::Refused,
            b"the pane did not take the input".to_vec(),
        ),
        // Heard, were the refusal let through: not an end.
        Frame::new(pane, Kind::Output, b"$ ".to_vec()),
    ] {
        frame::write(&mut daemon, &one).expect("write");
    }

    assert!(matches!(first_heard(window, pane), Some(Heard::Ended)));
}

/// What the terminal is told: the reason a refused attach ended, and
/// nothing for one that ended on its own.
#[test]
fn a_refused_attach_ends_with_its_reason_and_any_other_ends_quietly() {
    let refused = ended(Some("no such pane".to_owned())).expect_err("refused");
    assert_eq!(refused.code, ErrorCode::NotFound);
    assert!(
        refused.message.contains("no such pane"),
        "{}",
        refused.message
    );

    assert!(ended(None).is_ok());
}
