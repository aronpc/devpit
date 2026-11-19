use super::*;

/// A pane the daemon no longer holds — a session listed on the pane it ran
/// in before devpit-ptyd restarted — is refused, and the attach hears why.
/// Heard as a plain end, the terminal stayed blank with nothing said.
#[test]
fn an_attach_to_a_pane_the_daemon_does_not_hold_hears_why() {
    let daemon = crate::ptyd_link::tests::serving();
    let mut stream = crate::ptyd_link::connect(daemon.home.path()).expect("connect to the daemon");
    let pane = crate::ptyd_link::pane_number("leaf_gone");
    frame::write(&mut stream, &Frame::new(pane, Kind::Attach, Vec::new())).expect("attach");

    let (heard_tx, mut heard) = tokio::sync::mpsc::channel::<Heard>(8);
    std::thread::spawn(move || drain(stream, pane, heard_tx));

    match heard.blocking_recv() {
        Some(Heard::Refused(why)) => assert_eq!(why, "no such pane"),
        Some(Heard::Ended) => panic!("heard a plain end, not why"),
        Some(_) => panic!("heard something other than a refusal"),
        None => panic!("heard nothing"),
    }
}
