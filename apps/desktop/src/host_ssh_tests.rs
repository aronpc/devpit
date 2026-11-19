use std::path::Path;
use std::time::Duration;

use super::*;

fn host() -> HostRow {
    HostRow {
        id: "host_1".to_owned(),
        name: "staging".to_owned(),
        target: "staging-01".to_owned(),
        user: Some("deploy".to_owned()),
        port: Some(2222),
        identity_file: Some("~/.ssh/id_ed25519".to_owned()),
        work_root: "/srv".to_owned(),
        environment: "staging".to_owned(),
    }
}

/// The argv comes from the profile's fields alone: its port, user and key
/// path as ssh's own options, then `--`, so the target is never read as one.
#[test]
fn the_argv_is_the_profile_s_fields_and_the_target_after_the_options() {
    let argv = argv(&host(), Path::new("/home/me/.devpit/ssh"), &["tmux", "-V"]);

    let at = |flag: &str| {
        argv.iter()
            .position(|one| one == flag)
            .map(|at| argv[at + 1].as_str())
    };
    assert_eq!(argv[0], "ssh");
    assert_eq!(at("-p"), Some("2222"));
    assert_eq!(at("-l"), Some("deploy"));
    assert_eq!(at("-i"), Some("~/.ssh/id_ed25519"));
    assert!(argv.contains(&"BatchMode=yes".to_owned()), "{argv:?}");
    assert!(
        argv.iter().any(|one| one.starts_with("ConnectTimeout=")),
        "{argv:?}"
    );
    let target = argv
        .iter()
        .position(|one| one == "staging-01")
        .expect("target");
    assert_eq!(argv[target - 1], "--");
    assert_eq!(argv[target + 1], "tmux -V");
    assert_eq!(argv.len(), target + 2, "one line for the host's shell");
}

/// An alias alone adds nothing ssh's config would not say itself.
#[test]
fn an_alias_alone_is_only_the_target() {
    let alias = HostRow {
        user: None,
        port: None,
        identity_file: None,
        ..host()
    };

    let argv = argv(&alias, Path::new("/s"), &["true"]);

    for flag in ["-p", "-l", "-i"] {
        assert!(!argv.contains(&flag.to_owned()), "{flag} in {argv:?}");
    }
}

/// Every command to a host goes over the same connection: one control
/// socket per host, under devpit's home, kept a while after the last use.
#[test]
fn every_command_to_a_host_shares_its_control_socket() {
    let sockets = Path::new("/home/me/.devpit/ssh");

    let first = argv(&host(), sockets, &["tmux", "-V"]);
    let second = argv(&host(), sockets, &["git", "--version"]);

    let control = |argv: &[String]| {
        argv.iter()
            .filter(|one| one.starts_with("Control"))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(control(&first), control(&second));
    assert!(control(&first).contains(&"ControlMaster=auto".to_owned()));
    assert!(control(&first).contains(&"ControlPath=/home/me/.devpit/ssh/%C".to_owned()));
    assert!(control(&first)
        .iter()
        .any(|one| one.starts_with("ControlPersist=")));
}

/// A dead connection is found out, and nothing of the person's agent or
/// display goes along with a command.
#[test]
fn the_connection_is_kept_alive_and_forwards_nothing() {
    let argv = argv(&host(), Path::new("/home/me/.devpit/ssh"), &["true"]);

    for option in [
        "ServerAliveInterval=15",
        "ForwardAgent=no",
        "ForwardX11=no",
        "ClearAllForwardings=yes",
        "RemoteCommand=none",
    ] {
        assert!(
            argv.contains(&option.to_owned()),
            "{option} not in {argv:?}"
        );
    }
}

/// A home whose path ssh cannot take as a control socket gets no shared
/// connection, rather than a broken one.
#[test]
fn a_socket_path_ssh_cannot_take_shares_no_connection() {
    for sockets in [
        "/home/a b/.devpit/ssh",
        "/home/100%/.devpit/ssh",
        &format!("/{}", "x".repeat(60)),
    ] {
        let argv = argv(&host(), Path::new(sockets), &["true"]);

        assert!(
            argv.contains(&"ControlPath=none".to_owned()),
            "{sockets}: {argv:?}"
        );
        assert!(
            !argv.contains(&"ControlMaster=auto".to_owned()),
            "{sockets}: {argv:?}"
        );
    }
}

/// What a host prints is read up to a ceiling, and the rest is drained.
#[cfg(unix)]
#[test]
fn a_host_s_output_is_read_up_to_a_ceiling() {
    let said = run(
        &[
            "sh".to_owned(),
            "-c".to_owned(),
            "head -c 9000000 /dev/zero".to_owned(),
        ],
        &[],
        Duration::from_secs(20),
    )
    .expect("ran");

    assert!(said.status.success());
    assert_eq!(said.stdout.len(), devpit_tmux::remote::OUTPUT_MOST);
}

/// A process left behind holding the output open, as ssh's shared
/// connection can be, does not keep the answer waiting.
#[cfg(unix)]
#[test]
fn a_process_left_holding_the_output_does_not_hold_the_answer() {
    let started = std::time::Instant::now();

    let said = run(
        &[
            "sh".to_owned(),
            "-c".to_owned(),
            "sleep 8 & echo done".to_owned(),
        ],
        &[],
        Duration::from_secs(20),
    )
    .expect("ran");

    assert_eq!(String::from_utf8_lossy(&said.stdout).trim(), "done");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

/// Given up on, the command goes with whatever it started.
#[cfg(target_os = "linux")]
#[test]
fn a_command_given_up_on_takes_what_it_started_with_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pidfile = dir.path().join("pid");
    let script = format!("sleep 30 & echo $! > {}; wait", pidfile.display());

    let said = run(
        &["sh".to_owned(), "-c".to_owned(), script],
        &[],
        Duration::from_millis(500),
    );

    assert!(said.expect_err("answered").contains("no answer"));
    let pid: i32 = std::fs::read_to_string(&pidfile)
        .expect("pid")
        .trim()
        .parse()
        .expect("a pid");
    let gone = (0..60).any(|_| {
        std::thread::sleep(Duration::from_millis(50));
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        stat.is_empty()
            || stat
                .rsplit_once(')')
                .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'))
    });
    assert!(gone, "what it started still runs");
}

/// A window attaches with a terminal on the host; a command never asks one.
#[test]
fn only_an_attach_asks_the_host_for_a_terminal() {
    let sockets = Path::new("/s");

    assert!(argv_tty(&host(), sockets, &["tmux"]).contains(&"-t".to_owned()));
    assert!(!argv(&host(), sockets, &["tmux"]).contains(&"-t".to_owned()));
}
