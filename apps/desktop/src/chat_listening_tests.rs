use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use devpit_agentcli::resident::Heard;
use devpit_agentcli::talk::Said;
use devpit_rpc::{Frame, Part, TurnEnd};
use tauri::ipc::{Channel, InvokeResponseBody};

use super::{taken, Claim, Displaced, Ear, Listening, Woken};
use crate::chat_relay::Relay;

fn text(said: &str) -> Part {
    Part::Text {
        text: said.to_owned(),
        parent: None,
    }
}

/// A woken turn, and the frames it sends.
fn woken(relay: &Relay) -> (Woken, Arc<Mutex<Vec<Frame>>>) {
    let got = Arc::new(Mutex::new(Vec::new()));
    let into = got.clone();
    let (frames, relaying) = relay.open(
        "conv",
        crate::chat_relay::Sink::whole(Channel::new(move |body: InvokeResponseBody| {
            if let Ok(frame) = body.deserialize::<Frame>() {
                into.lock().unwrap().push(frame);
            }
            Ok(())
        })),
    );
    let turn = Woken {
        turn_id: "turn_woken".to_owned(),
        answer_id: "msg_woken".to_owned(),
        parts: Vec::new(),
        began: 100.0,
        frames,
        _relaying: relaying,
    };
    (turn, got)
}

fn ended() -> Said {
    Said {
        end: TurnEnd {
            turn_id: String::new(),
            cost_usd: Some(0.5),
            duration_ms: None,
            stop_reason: None,
            is_error: false,
            context: None,
            ..Default::default()
        },
        session_id: None,
        init: None,
        anchor: None,
    }
}

fn ear_on(turn: Woken) -> Arc<Ear> {
    let ear = Arc::new(Ear::default());
    ear.now.lock().unwrap().listening = Listening::Woken(turn);
    ear
}

/// Until the claim on its own thread is inside its wait.
fn until_waiting(ear: &Ear) {
    while !ear.now.lock().unwrap().waiting {
        std::thread::yield_now();
    }
}

fn is_woken(ear: &Ear) -> bool {
    matches!(ear.now.lock().unwrap().listening, Listening::Woken(_))
}

#[test]
fn a_person_never_takes_a_process_a_woken_turn_is_on() {
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let (tell, _) = mpsc::channel();
    let mut listening = Listening::Woken(turn);
    assert!(!taken(&mut listening, &tell));
    assert!(matches!(listening, Listening::Woken(_)));

    let mut quiet = Listening::Nobody;
    assert!(taken(&mut quiet, &tell));
    assert!(matches!(quiet, Listening::Person(_)));
}

#[test]
fn the_person_waits_for_the_woken_turn_then_hears_only_their_own() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("conv.jsonl");
    let head = dir.path().join("conv.json");
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let ear = ear_on(turn);
    let (tell, heard) = mpsc::channel();
    let claiming = {
        let ear = ear.clone();
        std::thread::spawn(move || ear.claim(&tell))
    };
    until_waiting(&ear);
    assert!(is_woken(&ear), "the woken turn is not taken from under it");

    ear.hear(
        Heard::Ended(Box::new(ended())),
        || unreachable!(),
        &transcript,
        &head,
    );
    assert_eq!(claiming.join().unwrap(), Claim::Claimed);
    ear.hear(
        Heard::Part(text("mine")),
        || panic!("the person's words started a woken turn"),
        &transcript,
        &head,
    );
    assert!(matches!(heard.try_recv(), Ok(Heard::Part(_))));
}

#[test]
fn stopping_a_waiting_turn_leaves_the_woken_one_running() {
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let ear = ear_on(turn);
    assert!(!ear.stop_waiting(), "nothing waits yet");
    let (tell, _) = mpsc::channel();
    let claiming = {
        let ear = ear.clone();
        std::thread::spawn(move || ear.claim(&tell))
    };
    until_waiting(&ear);

    assert!(ear.stop_waiting());
    assert_eq!(claiming.join().unwrap(), Claim::Stopped);
    assert!(is_woken(&ear));
    assert!(!ear.now.lock().unwrap().waiting);
}

#[test]
fn a_woken_turn_that_dies_keeps_what_it_said_and_names_itself() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("conv.jsonl");
    let relay = Relay::default();
    let (turn, frames) = woken(&relay);
    let ear = ear_on(turn);
    let head = dir.path().join("conv.json");
    // Two deltas of one sentence are one part, not two paragraphs.
    for piece in ["half ", "an "] {
        ear.hear(
            Heard::Part(text(piece)),
            || unreachable!(),
            &transcript,
            &head,
        );
    }
    ear.hear(Heard::Gone, || unreachable!(), &transcript, &head);

    let (kept, _) = devpit_agentcli::store::read(&transcript);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].turn_id.as_deref(), Some("turn_woken"));
    assert!(matches!(&kept[0].parts[..], [Part::Text { text, .. }] if text == "half an "));
    // It began when it opened, and it says it was stopped, so a reopened chat does too.
    assert_eq!(kept[0].created_at, 100.0);
    let ended = kept[0]
        .ended
        .as_ref()
        .expect("the answer keeps how it ended");
    assert_eq!(ended.stop_reason.as_deref(), Some("interrupted"));
    assert!(ended.is_error && ended.at > 100.0);
    let last = frames.lock().unwrap().pop();
    assert!(
        matches!(&last, Some(Frame::Ended { end }) if end.turn_id == "turn_woken"),
        "the end names the turn it ends"
    );
    assert!(matches!(
        ear.now.lock().unwrap().listening,
        Listening::Nobody
    ));
}

/// A busy conversation is closed after its turn, not under it — so whatever
/// is on the process has to read as busy.
#[test]
fn busy_names_every_way_a_turn_can_be_on() {
    assert!(!Ear::default().busy(), "a quiet ear is not busy");

    // The wait alone, with nobody listening yet, is a turn on too.
    let waiting_only = Ear::default();
    waiting_only.now.lock().unwrap().waiting = true;
    assert!(waiting_only.busy(), "a waiting person alone reads as busy");

    let (tell, _) = mpsc::channel();
    let person = Ear::default();
    person.now.lock().unwrap().listening = Listening::Person(tell);
    assert!(person.busy(), "the person's turn is on");

    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    assert!(ear_on(turn).busy(), "a woken turn is on");

    // The person waiting for the process is on a turn of their own.
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let waiting = ear_on(turn);
    let (tell, _) = mpsc::channel();
    let claiming = {
        let ear = waiting.clone();
        std::thread::spawn(move || ear.claim(&tell))
    };
    until_waiting(&waiting);
    assert!(waiting.busy(), "a waiting person is on a turn too");
    let dir = tempfile::tempdir().unwrap();
    let (transcript, head) = (dir.path().join("c.jsonl"), dir.path().join("c.json"));
    waiting.hear(
        Heard::Ended(Box::new(ended())),
        || unreachable!(),
        &transcript,
        &head,
    );
    assert_eq!(claiming.join().unwrap(), Claim::Claimed);
}

#[test]
fn a_displaced_conversation_closes_when_its_turn_ends() {
    let dir = tempfile::tempdir().unwrap();
    let (transcript, head) = (dir.path().join("c.jsonl"), dir.path().join("c.json"));
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let ear = ear_on(turn);
    let ran = Arc::new(AtomicUsize::new(0));
    let outside = Arc::new(AtomicBool::new(false));
    let counting = ran.clone();
    let unlocked = outside.clone();
    let of = ear.clone();
    ear.displace(move || {
        counting.fetch_add(1, Ordering::SeqCst);
        // The closer takes locks of its own: it cannot run under the ear's.
        unlocked.store(of.now.try_lock().is_ok(), Ordering::SeqCst);
    });

    // The turn on it goes on to its end.
    ear.hear(
        Heard::Part(text("still on it")),
        || panic!("the turn on a displaced conversation was cut short"),
        &transcript,
        &head,
    );
    assert!(is_woken(&ear));
    assert_eq!(
        ran.load(Ordering::SeqCst),
        0,
        "the closer waits for the turn"
    );

    ear.hear(
        Heard::Ended(Box::new(ended())),
        || unreachable!(),
        &transcript,
        &head,
    );
    assert_eq!(
        ran.load(Ordering::SeqCst),
        1,
        "the closer ran once, when the ear went quiet"
    );
    assert!(
        outside.load(Ordering::SeqCst),
        "the closer runs outside the ear's lock"
    );

    // Quiet again: once is once.
    ear.hear(
        Heard::Ended(Box::new(ended())),
        || unreachable!(),
        &transcript,
        &head,
    );
    assert_eq!(ran.load(Ordering::SeqCst), 1);
}

#[test]
fn a_displaced_conversation_takes_no_new_woken_turn() {
    let dir = tempfile::tempdir().unwrap();
    let (transcript, head) = (dir.path().join("c.jsonl"), dir.path().join("c.json"));
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let ear = ear_on(turn);
    let ran = Arc::new(AtomicUsize::new(0));
    let counting = ran.clone();
    ear.displace(move || {
        counting.fetch_add(1, Ordering::SeqCst);
    });

    ear.hear(
        Heard::Ended(Box::new(ended())),
        || unreachable!(),
        &transcript,
        &head,
    );
    assert_eq!(
        ran.load(Ordering::SeqCst),
        1,
        "the closer ran at the turn's end"
    );

    // A message that reaches the process after does not wake another turn,
    // and the person's next one does not wait for this process either.
    let (tell, _) = mpsc::channel();
    assert_eq!(
        ear.claim(&tell),
        Claim::Closed,
        "a closed process is not taken again"
    );
    ear.hear(
        Heard::Part(text("too late")),
        || panic!("a displaced conversation took a new woken turn"),
        &transcript,
        &head,
    );
    assert!(matches!(
        ear.now.lock().unwrap().listening,
        Listening::Nobody
    ));
    assert_eq!(ran.load(Ordering::SeqCst), 1);
}

/// A person speaking in the conversation again undoes the displacement: it
/// listens on, and nothing closes when the turn ends.
#[test]
fn a_person_turn_after_the_displacement_takes_the_listening_back() {
    let dir = tempfile::tempdir().unwrap();
    let (transcript, head) = (dir.path().join("c.jsonl"), dir.path().join("c.json"));
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let ear = ear_on(turn);
    let ran = Arc::new(AtomicUsize::new(0));
    let counting = ran.clone();
    ear.displace(move || {
        counting.fetch_add(1, Ordering::SeqCst);
    });

    assert!(ear.unmark(), "the displacement was undone");
    assert!(!ear.unmark(), "once undone, there is nothing to undo");
    ear.hear(
        Heard::Ended(Box::new(ended())),
        || unreachable!(),
        &transcript,
        &head,
    );

    // The person's turn runs and ends, and the conversation still listens.
    let (tell, from_person) = mpsc::channel();
    assert_eq!(ear.claim(&tell), Claim::Claimed);
    ear.hear(
        Heard::Part(text("the person's")),
        || unreachable!(),
        &transcript,
        &head,
    );
    ear.hear(
        Heard::Ended(Box::new(ended())),
        || unreachable!(),
        &transcript,
        &head,
    );
    assert!(matches!(from_person.try_recv(), Ok(Heard::Part(_))));
    assert_eq!(
        ran.load(Ordering::SeqCst),
        0,
        "nothing closed: the conversation listens again"
    );

    // And a session can still wake it: it is the listener.
    let relay = Relay::default();
    let (waking, _) = woken(&relay);
    ear.hear(
        Heard::Part(text("from a session")),
        || waking,
        &transcript,
        &head,
    );
    assert!(is_woken(&ear));
}

/// The turn ends between "is it busy" and "arm the closer": with the two in
/// one step there is no such gap, so the quiet ear closes on the spot and the
/// closer is never stored to wait for an event that will not come.
#[test]
fn displacing_a_quiet_ear_commits_the_closing_and_stores_no_closer() {
    let dir = tempfile::tempdir().unwrap();
    let (transcript, head) = (dir.path().join("c.jsonl"), dir.path().join("c.json"));
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let ear = ear_on(turn);
    assert!(ear.busy());
    // The turn ends before the displacement is made.
    ear.hear(
        Heard::Ended(Box::new(ended())),
        || unreachable!(),
        &transcript,
        &head,
    );
    let ran = Arc::new(AtomicUsize::new(0));
    let counting = ran.clone();
    let displaced = ear.displace(move || {
        counting.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(displaced, Displaced::Now);
    assert!(ear.closed(), "the closing is committed with the decision");
    assert!(
        ear.now.lock().unwrap().closing.is_none(),
        "no closer is left waiting for an event that will not come"
    );
    assert_eq!(
        ran.load(Ordering::SeqCst),
        0,
        "the caller closes, not the ear"
    );

    // A busy ear is the other answer.
    let (turn, _) = woken(&relay);
    assert_eq!(ear_on(turn).displace(|| {}), Displaced::Later);
}

/// Stop and the closing arrive together: Stop wins, and the turn ends
/// cancelled instead of starting on a new process.
#[test]
fn a_stop_beside_the_closing_still_stops_the_turn() {
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let ear = ear_on(turn);
    ear.displace(|| {});
    let (tell, _) = mpsc::channel();
    let claiming = {
        let ear = ear.clone();
        std::thread::spawn(move || ear.claim(&tell))
    };
    until_waiting(&ear);

    // Both land before the claim wakes: the ear lock is held across them.
    {
        let mut now = ear.now.lock().unwrap();
        now.stopped = true;
        now.closed = true;
    }
    ear.quiet.notify_all();
    assert_eq!(
        claiming.join().unwrap(),
        Claim::Stopped,
        "the stop was read as a closing"
    );
}

/// A turn on its way to a process reads as busy before it has claimed it, so
/// a displacement in that gap arms the closer instead of closing under it.
#[test]
fn an_expected_turn_is_busy_before_it_claims() {
    let ear = Ear::default();
    assert!(!ear.busy());
    ear.expect();
    assert!(ear.busy());
    assert_eq!(ear.displace(|| {}), Displaced::Later);
    assert!(!ear.closed());
    // A Stop asked in the gap is kept for the claim.
    assert!(ear.stop_waiting());
    let (tell, _) = mpsc::channel();
    assert_eq!(ear.claim(&tell), Claim::Stopped);
}

/// A Stop before the claim leaves no turn to end and run the closer the
/// displacement armed: the claim runs it, once, and the process is closed.
#[test]
fn a_stop_before_the_claim_closes_the_displaced_process() {
    let ear = Ear::default();
    ear.expect();
    let ran = Arc::new(AtomicUsize::new(0));
    let counter = ran.clone();
    let displaced = ear.displace(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(displaced, Displaced::Later);
    assert!(ear.stop_waiting());
    let (tell, _) = mpsc::channel();
    assert_eq!(ear.claim(&tell), Claim::Stopped);
    assert!(ear.closed(), "the displaced process is closed");
    assert_eq!(ran.load(Ordering::SeqCst), 1, "the closer ran once");
    assert!(ear.now.lock().unwrap().closing.is_none());
}

/// With a woken turn still on, the closer stays for that turn's end.
#[test]
fn a_stop_before_the_claim_leaves_a_woken_turns_closer_armed() {
    let relay = Relay::default();
    let (turn, _) = woken(&relay);
    let ear = ear_on(turn);
    ear.expect();
    let ran = Arc::new(AtomicUsize::new(0));
    let counter = ran.clone();
    ear.displace(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    assert!(ear.stop_waiting());
    let (tell, _) = mpsc::channel();
    assert_eq!(ear.claim(&tell), Claim::Stopped);
    assert!(!ear.closed());
    assert_eq!(ran.load(Ordering::SeqCst), 0);
    assert!(ear.now.lock().unwrap().closing.is_some());
}
