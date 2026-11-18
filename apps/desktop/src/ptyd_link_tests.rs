use super::*;
use devpit_rpc::{LayoutNode, SessionLayout, SplitDirection};

fn layout() -> SessionLayout {
    let tree = LayoutNode::leaf("leaf_a", "devpit-p:leaf_a")
        .split_leaf(
            "leaf_a",
            SplitDirection::Horizontal,
            LayoutNode::leaf("leaf_b", target("leaf_b")),
        )
        .expect("split");
    SessionLayout {
        project_id: "p".into(),
        focused_id: "leaf_a".into(),
        tree,
    }
}

#[test]
fn a_ptyd_leaf_is_told_apart_by_its_target() {
    let layout = layout();
    assert!(
        !leaf_is_ptyd(&layout, "leaf_a"),
        "a tmux window stays tmux's"
    );
    assert!(leaf_is_ptyd(&layout, "leaf_b"));
    assert!(!leaf_is_ptyd(&layout, "leaf_c"), "not in the layout");
    assert_eq!(ptyd_leaves(&layout), ["leaf_b"]);
}

#[test]
fn a_leaf_has_the_same_pane_number_on_every_run_and_never_zero() {
    assert_eq!(pane_number("leaf_01ABC"), pane_number("leaf_01ABC"));
    assert_ne!(pane_number("leaf_01ABC"), pane_number("leaf_01ABD"));
    assert_ne!(pane_number(""), 0);
}

#[test]
fn it_is_on_unless_the_preference_says_tmux() {
    let dir = tempfile::tempdir().expect("dir");
    let store = Store::open(&dir.path().join("state.db")).expect("store");
    assert_eq!(wanted(&store), cfg!(unix), "unset means ptyd");
    for ptyd in ["", "ptyd", " ptyd "] {
        store
            .set_preference(preference::TERMINAL_BACKEND, ptyd)
            .expect("set");
        assert_eq!(wanted(&store), cfg!(unix), "{ptyd:?}");
    }
    store
        .set_preference(preference::TERMINAL_BACKEND, TMUX)
        .expect("set");
    assert!(!wanted(&store), "tmux means tmux");
}

/// With ptyd wanted and no daemon to be had, a new terminal is made
/// in tmux as it always was: `in_ptyd_at` answers nothing, and that is the
/// fallback's whole contract with its callers.
#[cfg(unix)]
#[test]
fn with_no_daemon_a_new_terminal_falls_back_to_tmux() {
    let home = tempfile::tempdir().expect("home");
    let err = devpit_ptyd::launch::connect_or_start(
        None,
        &[],
        home.path(),
        &home.path().join("devpit-ptyd.sock"),
    )
    .expect_err("no daemon, nothing to start it from");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    let store = Store::open(&home.path().join("state.db")).expect("store");
    let mut tried = false;
    let no_daemon = |_: &Path, _: &str, _: &Path, _: Vec<(String, String)>| {
        tried = true;
        Err(err.to_string())
    };
    let found = in_ptyd_with(
        &store,
        Some(home.path()),
        "p",
        "leaf_x",
        home.path(),
        no_daemon,
    );
    assert!(tried, "ptyd was not asked first");
    assert_eq!(found, None);
}

/// A daemon of this process's own, serving a home of its own, for as long
/// as it is held.
#[cfg(unix)]
pub(crate) struct Serving {
    pub(crate) home: tempfile::TempDir,
    socket: PathBuf,
}

#[cfg(unix)]
impl Drop for Serving {
    fn drop(&mut self) {
        // Under XDG_RUNTIME_DIR, not the home: it outlives the temp dir.
        let _ = std::fs::remove_file(&self.socket);
    }
}

#[cfg(unix)]
pub(crate) fn serving() -> Serving {
    let home = tempfile::tempdir().expect("home");
    let socket = socket_path(home.path());
    let listener = devpit_ptyd::socket::listen(&socket).expect("listen");
    let daemon = devpit_ptyd::daemon::Daemon::new();
    std::thread::spawn(move || daemon.serve(listener));
    Serving { home, socket }
}

/// The split's question, and every new pane's: nothing about the pane it
/// splits from or what it is for is asked, only whether ptyd is wanted.
#[cfg(unix)]
#[test]
fn a_new_pane_goes_to_ptyd_while_wanted_and_to_tmux_otherwise() {
    let daemon = serving();
    let home = daemon.home.path();
    let store = Store::open(&home.join("state.db")).expect("store");
    store
        .set_preference(preference::TERMINAL_BACKEND, TMUX)
        .expect("set");
    let in_tmux = || Ok("devpit_p:leaf_t".to_owned());
    assert_eq!(
        new_pane(&store, Some(home), "p", "leaf_t", home, in_tmux).expect("made"),
        "devpit_p:leaf_t",
        "tmux: tmux makes it"
    );

    store
        .set_preference(preference::TERMINAL_BACKEND, "ptyd")
        .expect("set");
    let made = new_pane(&store, Some(home), "p", "leaf_s", home, || {
        panic!("tmux was asked while the daemon answered")
    })
    .expect("made");
    assert_eq!(made, target("leaf_s"));
    assert!(running(home, pane_number("leaf_s")));
    kill(home, pane_number("leaf_s"));
}

/// What says whether the agent is typed in again: a pane the daemon still
/// holds is left as it is, and only one it lost is started afresh.
#[cfg(unix)]
#[test]
fn a_pane_is_started_again_only_when_the_daemon_lost_it() {
    let daemon = serving();
    let home = daemon.home.path();
    let leaf = "leaf_revived";
    assert!(revive(home, leaf, home, Vec::new()), "lost: started again");
    assert!(running(home, pane_number(leaf)));
    assert!(
        !revive(home, leaf, home, Vec::new()),
        "running: left as it is"
    );
    kill(home, pane_number(leaf));
}

/// A daemon that answers, but late: every frame refused after 300 ms.
#[cfg(unix)]
fn slow_daemon_refusing_all(home: &Path) {
    use devpit_ptyd::frame::{self, Frame, Kind};

    let listener = devpit_ptyd::socket::listen(&socket_path(home)).expect("listen");
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let _ = devpit_ptyd::socket::greet(&mut stream);
                while let Ok(Some(one)) = frame::read(&mut stream) {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    let no = Frame::new(one.pane, Kind::Refused, "no such pane".to_owned());
                    let _ = frame::write(&mut stream, &no);
                }
            });
        }
    });
}

/// Silence was taken as a yes: a loaded machine made a lost pane look
/// running, and a refused spawn look made.
#[cfg(unix)]
#[test]
fn a_slow_daemon_is_waited_for_not_taken_at_its_silence() {
    let home = tempfile::tempdir().expect("home");
    slow_daemon_refusing_all(home.path());
    assert!(!running(home.path(), 7), "a refused pane read as running");
    let asked = devpit_ptyd::pane::Asked {
        rows: 24,
        cols: 80,
        cwd: home.path().to_string_lossy().into_owned(),
        argv: vec!["/bin/sh".into()],
        env: Vec::new(),
    };
    assert!(
        spawn(home.path(), 7, &asked).is_err(),
        "a refused spawn read as made"
    );
}

#[cfg(unix)]
fn shell_in(home: &Path) -> devpit_ptyd::pane::Asked {
    devpit_ptyd::pane::Asked {
        rows: 24,
        cols: 80,
        cwd: home.to_string_lossy().into_owned(),
        argv: vec!["/bin/sh".into()],
        env: Vec::new(),
    }
}

/// The daemon's first word to `pane` on `stream` after an attach: `None`
/// when it holds the pane, its refusal when it does not.
#[cfg(unix)]
fn refusal_of_attach(stream: &mut std::os::unix::net::UnixStream, pane: u32) -> Option<String> {
    use devpit_ptyd::frame::{self, Frame, Kind};

    frame::write(stream, &Frame::new(pane, Kind::Attach, Vec::new())).expect("attach");
    let none = u64::MAX.to_le_bytes().to_vec();
    frame::write(stream, &Frame::new(pane, Kind::History, none)).expect("history");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("timeout");
    loop {
        let one = frame::read(stream).expect("read").expect("a frame");
        match one.kind() {
            Some(Kind::Refused) if one.pane == pane => {
                return Some(String::from_utf8_lossy(&one.bytes).into_owned())
            }
            Some(Kind::Older) if one.pane == pane => return None,
            _ => {}
        }
    }
}

/// A spawn answers once the daemon holds the pane, so an attach right after
/// it, on a connection of its own, finds it.
///
/// The panes are killed only after the last spawn: macOS can refuse a new pty
/// while another is being freed, and a kill right before each spawn made that
/// race on every round.
#[cfg(unix)]
#[test]
fn an_attach_right_after_a_spawn_is_never_refused() {
    let daemon = serving();
    let home = daemon.home.path();
    let mut answers = Vec::new();
    for pane in 1..=20 {
        spawn(home, pane, &shell_in(home)).expect("spawn");
        let mut other = connect(home).expect("connect");
        answers.push((pane, refusal_of_attach(&mut other, pane)));
    }
    for (pane, _) in &answers {
        kill(home, *pane);
    }
    for (pane, refused) in answers {
        assert_eq!(refused, None, "pane {pane}");
    }
}

/// A daemon that takes a spawn without a word and answers `History` in
/// order, as every protocol version has.
#[cfg(unix)]
fn daemon_silent_on_spawn(home: &Path) {
    use devpit_ptyd::frame::{self, Frame, Kind};

    let listener = devpit_ptyd::socket::listen(&socket_path(home)).expect("listen");
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let _ = devpit_ptyd::socket::greet(&mut stream);
                while let Ok(Some(one)) = frame::read(&mut stream) {
                    if one.kind() == Some(Kind::History) {
                        let older = Frame::new(one.pane, Kind::Older, 0u64.to_le_bytes().to_vec());
                        let _ = frame::write(&mut stream, &older);
                    }
                }
            });
        }
    });
}

/// No ack of its own: the spawn is taken on the answer to the `History`
/// that follows it.
#[cfg(unix)]
#[test]
fn a_daemon_that_says_nothing_to_a_spawn_still_has_it_taken() {
    let home = tempfile::tempdir().expect("home");
    daemon_silent_on_spawn(home.path());
    spawn(home.path(), 7, &shell_in(home.path())).expect("taken");
}

/// A pane the daemon lost comes back whenever its tab is drawn, entering its
/// project included: as a shell, with nothing typed into it, even where devpit
/// had started an agent. A session starts only when the person asks for one.
#[cfg(unix)]
#[test]
fn a_lost_pane_comes_back_as_a_shell_and_starts_no_agent() {
    use devpit_ptyd::frame::{self, Frame, Kind};

    let daemon = serving();
    let home = daemon.home.path();
    let store = Store::open(&home.join("state.db")).expect("store");
    let root = home.join("project");
    std::fs::create_dir_all(&root).expect("root");
    let project = store.add_project(&root, None).expect("project");
    let leaf = "leaf_lost_with_an_agent";
    // What the agent's hooks kept: devpit launched it there, on a conversation with lines.
    let transcript = home.join("94e3457c.jsonl");
    std::fs::write(&transcript, "{}\n").expect("transcript");
    store
        .remember_pane_launch(&project, leaf, "claude")
        .expect("launch");
    store
        .remember_pane_session(leaf, "94e3457c", Some(&transcript.display().to_string()))
        .expect("session");

    assert!(revive_at(&store, Some(home), &project, leaf, None, &root));
    let pane = pane_number(leaf);
    // A shell in front at once: whatever waited for the prompt is typed now.
    let told = Frame::new(pane, Kind::Program, b"sh".to_vec());
    let _ = crate::ptyd_tap::tap()
        .fronts
        .lock()
        .expect("fronts")
        .heard(&told);

    let mut watching = connect(home).expect("connect");
    frame::write(&mut watching, &Frame::new(pane, Kind::Attach, Vec::new())).expect("attach");
    watching
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .expect("timeout");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let mut seen = Vec::new();
    while std::time::Instant::now() < until {
        match frame::read(&mut watching) {
            Ok(Some(one)) if matches!(one.kind(), Some(Kind::Output | Kind::Replay)) => {
                seen.extend(one.bytes)
            }
            Ok(Some(_)) => {}
            _ => break,
        }
    }
    kill(home, pane);
    let screen = String::from_utf8_lossy(&seen);
    assert!(!screen.contains("--resume"), "{screen}");
}
