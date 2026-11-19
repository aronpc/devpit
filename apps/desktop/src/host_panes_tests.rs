use std::io::Read;
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
    fake.with_tmux();
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
    fake.with_tmux();
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
    fake.with_tmux();
    let session = session_of("prj_1", "leaf_3");

    let refused = started(&reach(&fake), &profile(), "~/nowhere", &session).expect_err("started");

    assert_eq!(refused.code, ErrorCode::NotFound);
    assert!(!fake.tmux().has_exactly(&session).unwrap_or(false));
}

/// A folder with shell in its name is one argument there: the terminal
/// starts in it, and nothing in its name runs.
#[test]
fn a_folder_with_shell_in_its_name_stays_one_argument() {
    let fake = FakeHost::new();
    fake.with_tmux();
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
    fake.with_tmux();
    fake.folder("demo");
    let session = session_of("prj_1", "leaf_5");
    started(&reach(&fake), &profile(), "~/demo", &session).expect("started");
    fake.tmux()
        .send_keys(&format!("{session}:"), "echo before-$((40+4))")
        .expect("typed");
    let argv = attach_argv_after(&reach(&fake), &profile(), "~/demo", &session, "0");

    let first = Attached::spawn(&argv, 120);
    assert!(first.shows("before-44"), "{}", first.said());
    first.end();
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
    second.end();
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

    fn said(&self) -> String {
        self.screen.lock().expect("screen").clone()
    }

    fn shows(&self, text: &str) -> bool {
        eventually(|| self.said().contains(text))
    }

    fn end(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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
