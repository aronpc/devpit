use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;

/// A host on this computer: a fake `ssh` that runs the line it is handed in
/// `sh`, a home of its own, and only the programs put in its `bin`.
struct FakeHost {
    dir: tempfile::TempDir,
}

impl FakeHost {
    fn new() -> Self {
        let host = Self {
            dir: tempfile::tempdir().expect("tempdir"),
        };
        std::fs::create_dir_all(host.bin()).expect("bin");
        std::fs::create_dir_all(host.home()).expect("home");
        // Every host has a shell; the rest is what each test installs.
        std::os::unix::fs::symlink("/bin/sh", host.bin().join("sh")).expect("sh");
        host.program(
            &host.dir.path().join("ssh"),
            "while [ \"$1\" != -- ]; do shift; done\nshift 2\nexec /bin/sh -c \"$1\"",
        );
        host
    }

    fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    fn ssh(&self) -> PathBuf {
        self.dir.path().join("ssh")
    }

    fn program(&self, path: &Path, body: &str) {
        std::fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("write");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    fn installs(&self, name: &str, says: &str) {
        self.program(&self.bin().join(name), &format!("echo '{says}'"));
    }

    fn signs_in_claude(&self) {
        let dir = self.home().join(".claude");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join(".credentials.json"), "{}").expect("credentials");
    }

    fn checked(&self, patience: Duration) -> HostCheck {
        let bin = self.bin().display().to_string();
        let home = self.home().display().to_string();
        checked_with(
            &self.ssh(),
            &[("PATH", bin.as_str()), ("HOME", home.as_str())],
            patience,
            &host(),
            self.dir.path(),
        )
    }
}

fn host() -> HostRow {
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

fn row<'a>(check: &'a HostCheck, what: &str) -> &'a HostCheckRow {
    check
        .rows
        .iter()
        .find(|one| one.what == what)
        .unwrap_or_else(|| panic!("no {what} in {check:?}"))
}

const PATIENCE: Duration = Duration::from_secs(10);

/// A host with ssh, tmux, git and a signed-in agent passes each check, and
/// says what it found.
#[test]
fn a_host_with_everything_passes_each_check() {
    let host = FakeHost::new();
    host.installs("tmux", "tmux 3.4");
    host.installs("git", "git version 2.43.0");
    host.installs("claude", "2.1.0 (Claude Code)");
    host.signs_in_claude();

    let check = host.checked(PATIENCE);

    assert!(check.rows.iter().all(|one| one.ok), "{check:?}");
    assert_eq!(row(&check, "tmux").said, "tmux 3.4");
    assert_eq!(row(&check, "git").said, "git version 2.43.0");
    assert!(
        row(&check, "agent CLI").said.contains("claude (signed in)"),
        "{check:?}"
    );
}

/// What is missing fails its own line, and says why it matters.
#[test]
fn a_host_without_tmux_or_a_signed_in_agent_fails_those_lines() {
    let host = FakeHost::new();
    host.installs("git", "git version 2.43.0");
    host.installs("claude", "2.1.0 (Claude Code)");

    let check = host.checked(PATIENCE);

    assert!(row(&check, "ssh").ok, "{check:?}");
    assert!(!row(&check, "tmux").ok);
    assert!(
        row(&check, "tmux").said.contains("not installed"),
        "{check:?}"
    );
    assert!(row(&check, "git").ok);
    assert!(!row(&check, "agent CLI").ok);
    assert!(
        row(&check, "agent CLI")
            .said
            .contains("claude (no sign-in found)"),
        "{check:?}"
    );
}

/// When ssh cannot reach the host, that is the answer: its own words, and
/// nothing else asked.
#[test]
fn a_host_ssh_cannot_reach_says_why_and_checks_nothing_else() {
    let host = FakeHost::new();
    host.program(
        &host.ssh(),
        "echo 'deploy@staging-01: Permission denied (publickey).' >&2\nexit 255",
    );

    let check = host.checked(PATIENCE);

    assert_eq!(check.rows.len(), 1, "{check:?}");
    assert!(!check.rows[0].ok);
    assert!(
        check.rows[0].said.contains("Permission denied (publickey)"),
        "{check:?}"
    );
}

/// ssh got in, but the host's shell could not run the check: that is said
/// apart from ssh's own failure.
#[test]
fn a_host_whose_shell_fails_the_check_says_so_apart_from_ssh() {
    let host = FakeHost::new();
    host.program(&host.ssh(), "echo \"Unmatched '.\" >&2\nexit 1");

    let check = host.checked(PATIENCE);

    assert_eq!(check.rows.len(), 1, "{check:?}");
    assert!(check.rows[0].said.contains("the host's shell"), "{check:?}");
    assert!(check.rows[0].said.contains("Unmatched"), "{check:?}");
}

/// A host that never answers is given up on, and said so.
#[test]
fn a_host_that_does_not_answer_is_given_up_on() {
    let host = FakeHost::new();
    host.program(&host.ssh(), "exec /bin/sleep 30");

    let check = host.checked(Duration::from_millis(500));

    assert_eq!(check.rows.len(), 1, "{check:?}");
    assert!(check.rows[0].said.contains("no answer"), "{check:?}");
}

/// A check reads; it writes nothing on the host.
#[test]
fn a_check_writes_nothing_on_the_host() {
    let host = FakeHost::new();
    host.installs("tmux", "tmux 3.4");
    host.installs("git", "git version 2.43.0");
    host.installs("claude", "2.1.0 (Claude Code)");
    host.signs_in_claude();
    let before = tree(&host.home());

    host.checked(PATIENCE);

    assert_eq!(tree(&host.home()), before);
}

fn tree(dir: &Path) -> Vec<(PathBuf, u64, Option<std::time::SystemTime>)> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).expect("dir").flatten() {
        let path = entry.path();
        let meta = std::fs::metadata(&path).expect("meta");
        found.push((path.clone(), meta.len(), meta.modified().ok()));
        if meta.is_dir() {
            found.extend(tree(&path));
        }
    }
    found.sort();
    found
}
