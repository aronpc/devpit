use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;
use crate::fake_host::FakeHost;

fn profile() -> HostRow {
    HostRow {
        id: "host_1".to_owned(),
        name: "staging".to_owned(),
        target: "staging-01".to_owned(),
        user: None,
        port: None,
        identity_file: None,
        work_root: "/srv".to_owned(),
        environment: "staging".to_owned(),
    }
}

fn reach(fake: &FakeHost) -> Reach {
    Reach {
        ssh: fake.ssh(),
        sockets: fake.dir.path().to_path_buf(),
    }
}

fn eventually(mut seen: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        if seen() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// A terminal opened on a host runs its shell in the link's folder there,
/// `~` read as the host's home.
#[test]
fn a_terminal_on_a_host_starts_in_the_link_s_folder() {
    let fake = FakeHost::new();
    if !fake.with_tmux() {
        return;
    }
    let folder = fake.folder("apps/demo");
    let session = session_of("prj_1", "leaf_1");

    started(&reach(&fake), &profile(), "~/apps/demo", &session).expect("started");

    let path = fake.tmux().pane_path(&session, "").expect("a pane");
    assert_eq!(
        path.canonicalize().expect("path"),
        folder.canonicalize().expect("folder")
    );
}

/// Typing and a paste reach the terminal on the host.
#[test]
fn typing_and_a_paste_reach_the_terminal_on_the_host() {
    let fake = FakeHost::new();
    if !fake.with_tmux() {
        return;
    }
    fake.folder("demo");
    let session = session_of("prj_1", "leaf_2");
    started(&reach(&fake), &profile(), "~/demo", &session).expect("started");
    let server = fake.tmux();
    let at = format!("{session}:");

    server
        .send_keys(&at, "echo typed-$((40+2))")
        .expect("typed");
    server.paste(&at, "echo pasted-$((40+3))").expect("pasted");
    server
        .press(&at, &[devpit_tmux::Key::Enter])
        .expect("entered");

    assert!(eventually(|| server.capture_pane(&at).is_ok_and(
        |screen| screen.contains("typed-42") && screen.contains("pasted-43")
    )));
}

/// A link whose folder is gone from the host is said, and nothing starts.
#[test]
fn a_folder_gone_from_the_host_is_said() {
    let fake = FakeHost::new();
    if !fake.with_tmux() {
        return;
    }
    let session = session_of("prj_1", "leaf_3");

    let refused = started(&reach(&fake), &profile(), "~/nowhere", &session).expect_err("started");

    assert_eq!(refused.code, ErrorCode::NotFound, "{}", refused.message);
    assert!(!fake.tmux().has_exactly(&session).unwrap_or(false));
}

/// A folder with shell in its name is one argument there: the terminal
/// starts in it, and nothing in its name runs.
#[test]
fn a_folder_with_shell_in_its_name_stays_one_argument() {
    let fake = FakeHost::new();
    if !fake.with_tmux() {
        return;
    }
    let folder = fake.folder("a b;touch pwned");
    let session = session_of("prj_1", "leaf_4");

    started(&reach(&fake), &profile(), "~/a b;touch pwned", &session).expect("started");

    let path = fake.tmux().pane_path(&session, "").expect("a pane");
    assert_eq!(
        path.canonicalize().expect("path"),
        folder.canonicalize().expect("folder")
    );
    assert!(!fake.home().join("pwned").exists(), "the name ran");
}

/// While ssh says the connection dropped, the window's attach runs again;
/// any other end is the terminal's own, and ends it.
#[test]
fn the_attach_runs_again_while_the_connection_drops() {
    let fake = FakeHost::new();
    let tries = fake.dir.path().join("tries");
    fake.program(
        &fake.ssh(),
        &format!(
            "echo x >> '{tries}'\n[ \"$(wc -l < '{tries}')\" -ge 3 ] && exit 0\nexit 255",
            tries = tries.display()
        ),
    );

    let argv = attach_argv_after(&reach(&fake), &profile(), "~", "devpit_s", "0");
    let ended = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .output()
        .expect("ran");

    assert!(ended.status.success(), "{ended:?}");
    assert_eq!(
        std::fs::read_to_string(&tries)
            .expect("tries")
            .lines()
            .count(),
        3
    );
    assert!(String::from_utf8_lossy(&ended.stdout).contains("reconnecting"));
}

/// A window that goes, as a dropped connection takes it, leaves the session
/// on the host; the next attach shows the same screen, at the window's size.
#[test]
fn the_session_outlives_its_window_and_the_next_attach_shows_it() {
    let fake = FakeHost::new();
    if !fake.with_tmux() {
        return;
    }
    fake.folder("demo");
    let session = session_of("prj_1", "leaf_5");
    started(&reach(&fake), &profile(), "~/demo", &session).expect("started");
    fake.tmux()
        .send_keys(&format!("{session}:"), "echo before-$((40+4))")
        .expect("typed");
    let argv = attach_argv_after(&reach(&fake), &profile(), "~/demo", &session, "0");

    let first = Attached::spawn(&argv, 120);
    assert!(first.shows("before-44"), "{}", first.said());
    // Its client ends; its pty stays open until the test does, so the next
    // one never reuses it.
    let _first = first.end();
    assert!(
        fake.tmux().has_exactly(&session).expect("asked"),
        "the session went with its window"
    );

    let second = Attached::spawn(&argv, 100);
    assert!(second.shows("before-44"), "{}", second.said());
    let width = || {
        fake.tmux()
            .format_of(&format!("{session}:"), "#{window_width}")
            .unwrap_or_default()
    };
    assert!(eventually(|| width() == "100"), "width {}", width());
    let _second = second.end();
}

/// A window attached in a pty, its screen read as it comes.
struct Attached {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    screen: std::sync::Arc<std::sync::Mutex<String>>,
    _master: Box<dyn portable_pty::MasterPty + Send>,
}

impl Attached {
    fn spawn(argv: &[String], cols: u16) -> Self {
        let pair = portable_pty::native_pty_system()
            .openpty(portable_pty::PtySize {
                rows: 30,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("pty");
        let mut command = portable_pty::CommandBuilder::new(&argv[0]);
        command.args(&argv[1..]);
        command.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(command).expect("spawn");
        let mut reader = pair.master.try_clone_reader().expect("reader");
        let screen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let heard = std::sync::Arc::clone(&screen);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(read) = reader.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                heard
                    .lock()
                    .expect("screen")
                    .push_str(&String::from_utf8_lossy(&chunk[..read]));
            }
        });
        Self {
            child,
            screen,
            _master: pair.master,
        }
    }

    fn types(&mut self, text: &str) {
        use std::io::Write;
        let mut writer = self._master.take_writer().expect("writer");
        writer.write_all(text.as_bytes()).expect("typed");
    }

    fn waited(&mut self, within: Duration) -> Option<portable_pty::ExitStatus> {
        let until = Instant::now() + within;
        while Instant::now() < until {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Some(status);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        None
    }

    fn said(&self) -> String {
        self.screen.lock().expect("screen").clone()
    }

    fn shows(&self, text: &str) -> bool {
        eventually(|| self.said().contains(text))
    }

    fn end(mut self) -> Self {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self
    }
}

/// Targets as they were keep their meaning; only `ssh:` ones reach a host.
#[test]
fn old_targets_keep_their_meaning_and_only_ssh_ones_reach_a_host() {
    assert_eq!(parsed("devpit_prj_1__leaf_1:leaf_1"), None);
    assert_eq!(parsed("ptyd:leaf_1"), None);
    let target = target("host_1", "devpit_prj_1__leaf_1");
    assert_eq!(parsed(&target), Some(("host_1", "devpit_prj_1__leaf_1")));
}

/// A host's session starts quieted like devpit's own: no status bar, a long
/// history.
#[test]
fn a_session_on_a_host_starts_quieted_like_devpit_s_own() {
    let fake = FakeHost::new();
    if !fake.with_tmux() {
        return;
    }
    fake.folder("demo");
    let session = session_of("prj_1", "leaf_6");

    started(&reach(&fake), &profile(), "~/demo", &session).expect("started");

    let at = format!("{session}:");
    assert_eq!(
        fake.tmux().format_of(&at, "#{status}").expect("asked"),
        "off"
    );
    assert_eq!(
        fake.tmux()
            .format_of(&at, "#{history_limit}")
            .expect("asked"),
        "10000"
    );
}

/// While it waits to reconnect, a Ctrl+C never ends the window's loop, and
/// what was typed meanwhile is dropped rather than typed on the host.
#[test]
fn waiting_to_reconnect_outlasts_a_ctrl_c_and_drops_what_was_typed() {
    let fake = FakeHost::new();
    let tries = fake.dir.path().join("tries");
    let got = fake.dir.path().join("got");
    fake.program(
        &fake.ssh(),
        &format!(
            "n=$(cat '{tries}' 2>/dev/null | wc -l); echo x >> '{tries}'\n[ \"$n\" -eq 0 ] && exit 255\nstty -icanon min 0 time 0; cat > '{got}'; exit 0",
            tries = tries.display(),
            got = got.display()
        ),
    );
    let argv = attach_argv_after(&reach(&fake), &profile(), "~", "devpit_s", "2");

    let mut waiting = Attached::spawn(&argv, 80);
    assert!(waiting.shows("reconnecting"), "{}", waiting.said());
    waiting.types("\x03echo replayed\r");

    let ended = waiting
        .waited(Duration::from_secs(15))
        .expect("the loop ended");
    assert!(ended.success(), "ended with {ended:?}: {}", waiting.said());
    assert_eq!(
        std::fs::read_to_string(&tries)
            .expect("tries")
            .lines()
            .count(),
        2
    );
    assert_eq!(
        std::fs::read_to_string(&got).unwrap_or_default(),
        "",
        "typed on the host"
    );
}

/// A stop ends the session on the host there and then, and says so when the
/// host does not answer: never "stopped" while it goes on there.
#[test]
fn a_stop_ends_the_session_on_the_host_or_says_it_could_not() {
    let fake = FakeHost::new();
    if !fake.with_tmux() {
        return;
    }
    fake.folder("demo");
    let session = session_of("prj_1", "leaf_9");
    started(&reach(&fake), &profile(), "~/demo", &session).expect("started");

    ended_on(&reach(&fake), &profile(), &session).expect("ended");
    assert!(
        !fake.tmux().has_exactly(&session).unwrap_or(true),
        "it goes on there"
    );

    started(&reach(&fake), &profile(), "~/demo", &session).expect("started again");
    let away = FakeHost::new();
    away.program(&away.ssh(), "exit 255");
    let refused = ended_on(&reach(&away), &profile(), &session).expect_err("said stopped");
    assert!(refused.message.contains("staging"), "{}", refused.message);
}

/// Whether devpit's connection to a host is up, as ssh says of the shared
/// connection: up, down, or unknown where ssh shares none.
#[test]
fn a_host_s_connection_is_up_down_or_unknown() {
    let fake = FakeHost::new();
    // Up only when asked of the shared connection: never a login of its own.
    fake.program(
        &fake.ssh(),
        "for one; do [ \"$prev\" = -O ] && [ \"$one\" = check ] && exit 0; prev=$one; done\nexit 2",
    );
    assert_eq!(connected(&reach(&fake), &profile()), Some(true));

    fake.program(&fake.ssh(), "exit 255");
    assert_eq!(connected(&reach(&fake), &profile()), Some(false));

    let unshared = Reach {
        ssh: fake.ssh(),
        sockets: PathBuf::from("/a path ssh cannot take"),
    };
    assert_eq!(connected(&unshared, &profile()), None);
}

/// A tab whose terminal runs on a host names it, with its environment; a
/// tab on this computer names none.
#[test]
fn a_tab_names_the_host_its_terminal_runs_on() {
    let fake = FakeHost::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let project = store.add_project(dir.path(), None).expect("project");
    store
        .save_host(&HostRow {
            environment: "production".to_owned(),
            ..profile()
        })
        .expect("host");
    let layout = |target: String| devpit_rpc::SessionLayout {
        project_id: project.clone(),
        focused_id: "leaf_1".to_owned(),
        tree: devpit_rpc::LayoutNode::leaf("leaf_1", target),
    };
    crate::sessions::persist(&store, "tab_host", &layout(target("host_1", "devpit_s")))
        .expect("kept");
    crate::sessions::persist(&store, "tab_here", &layout("devpit_p:leaf_1".to_owned()))
        .expect("kept");

    let on_host = tab_host_in(&store, &reach(&fake), &project, "tab_host")
        .expect("asked")
        .host
        .expect("a host");
    assert_eq!(on_host.name, "staging");
    assert_eq!(on_host.environment, devpit_rpc::HostEnvironment::Production);
    assert_eq!(on_host.connected, Some(true));
    assert_eq!(
        tab_host_in(&store, &reach(&fake), &project, "tab_here")
            .expect("asked")
            .host,
        None
    );
}

/// The first attach leaves other clients of the session be — the person's
/// own `tmux attach` there among them; only an attach after a dropped
/// connection takes the session from a client the drop left behind.
#[test]
fn only_an_attach_after_a_drop_takes_the_session_from_other_clients() {
    let fake = FakeHost::new();
    let lines = fake.dir.path().join("lines");
    fake.program(
        &fake.ssh(),
        &format!(
            "for last; do :; done; printf '%s\\n' \"$last\" >> '{lines}'\n[ \"$(wc -l < '{lines}')\" -ge 2 ] && exit 0\nexit 255",
            lines = lines.display()
        ),
    );

    let argv = attach_argv_after(&reach(&fake), &profile(), "~", "devpit_s", "0");
    let ended = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .output()
        .expect("ran");

    assert!(ended.status.success(), "{ended:?}");
    let said = std::fs::read_to_string(&lines).expect("lines");
    let said: Vec<&str> = said.lines().collect();
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said[0].contains("new-session -A -s devpit_s"),
        "{}",
        said[0]
    );
    assert!(
        said[1].contains("new-session -A -D -s devpit_s"),
        "{}",
        said[1]
    );
}

/// A tab with terminals on two hosts names production when one of them is:
/// whichever way its panes were joined, PROD is what it shows.
#[test]
fn a_tab_on_two_hosts_names_the_production_one() {
    let fake = FakeHost::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let project = store.add_project(dir.path(), None).expect("project");
    store.save_host(&profile()).expect("staging");
    store
        .save_host(&HostRow {
            id: "host_2".to_owned(),
            name: "web prod".to_owned(),
            environment: "production".to_owned(),
            ..profile()
        })
        .expect("production");
    let tree = devpit_rpc::LayoutNode::leaf("leaf_a", target("host_1", "devpit_a"))
        .split_leaf(
            "leaf_a",
            devpit_rpc::SplitDirection::Vertical,
            devpit_rpc::LayoutNode::leaf("leaf_b", target("host_2", "devpit_b")),
        )
        .expect("joined");
    let layout = devpit_rpc::SessionLayout {
        project_id: project.clone(),
        focused_id: "leaf_a".to_owned(),
        tree,
    };
    crate::sessions::persist(&store, "tab_both", &layout).expect("kept");

    let host = tab_host_in(&store, &reach(&fake), &project, "tab_both")
        .expect("asked")
        .host
        .expect("a host");

    assert_eq!(host.name, "web prod");
    assert_eq!(host.environment, devpit_rpc::HostEnvironment::Production);
}
