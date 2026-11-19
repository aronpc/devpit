use std::path::{Path, PathBuf};
use std::time::Duration;

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

fn row<'a>(check: &'a HostCheck, what: &str) -> &'a HostCheckRow {
    check
        .rows
        .iter()
        .find(|one| one.what == what)
        .unwrap_or_else(|| panic!("no {what} in {check:?}"))
}

const PATIENCE: Duration = Duration::from_secs(10);

fn checked(host: &FakeHost, patience: Duration) -> HostCheck {
    let envs = host.envs();
    let envs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    checked_with(&host.ssh(), &envs, patience, &profile(), host.dir.path())
}

/// A host with ssh, tmux, git and a signed-in agent passes each check, and
/// says what it found.
#[test]
fn a_host_with_everything_passes_each_check() {
    let host = FakeHost::new();
    host.installs("tmux", "tmux 3.4");
    host.installs("git", "git version 2.43.0");
    host.installs("claude", "2.1.0 (Claude Code)");
    host.signs_in_claude();

    let check = checked(&host, PATIENCE);

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

    let check = checked(&host, PATIENCE);

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

    let check = checked(&host, PATIENCE);

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

    let check = checked(&host, PATIENCE);

    assert_eq!(check.rows.len(), 1, "{check:?}");
    assert!(check.rows[0].said.contains("the host's shell"), "{check:?}");
    assert!(check.rows[0].said.contains("Unmatched"), "{check:?}");
}

/// A host that never answers is given up on, and said so.
#[test]
fn a_host_that_does_not_answer_is_given_up_on() {
    let host = FakeHost::new();
    host.program(&host.ssh(), "exec /bin/sleep 30");

    let check = checked(&host, Duration::from_millis(500));

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

    checked(&host, PATIENCE);

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

/// A tmux older than devpit can drive fails its line, and says which.
#[test]
fn a_tmux_too_old_to_drive_fails_its_line() {
    let host = FakeHost::new();
    host.installs("tmux", "tmux 2.5");
    host.installs("git", "git version 2.43.0");

    let check = checked(&host, PATIENCE);

    assert!(!row(&check, "tmux").ok, "{check:?}");
    assert!(row(&check, "tmux").said.contains("2.5"), "{check:?}");
    assert!(row(&check, "tmux").said.contains("2.6"), "{check:?}");
}

/// The tmux devpit can drive, from a version line: 2.6 and later.
#[test]
fn which_tmux_devpit_can_drive() {
    assert!(drivable("tmux 3.4"));
    assert!(drivable("tmux 2.9a"));
    assert!(drivable("tmux 2.6"));
    assert!(drivable("tmux next-3.5"));
    assert!(!drivable("tmux 2.5"));
    assert!(!drivable("tmux 1.8"));
}
