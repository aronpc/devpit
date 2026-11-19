use std::path::Path;

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

/// The host's shell gets each word back as it was: no expansion, no second
/// command, whatever the word holds.
#[cfg(unix)]
#[test]
fn every_word_reaches_the_host_s_shell_as_one_argument() {
    let dir = tempfile::tempdir().expect("tempdir");
    let words = [
        "printf",
        "%s\\n",
        "a b",
        "$(touch pwned)",
        "x;touch pwned",
        "it's",
        "`touch pwned`",
        "",
    ];

    let said = std::process::Command::new("sh")
        .arg("-c")
        .arg(line(&words))
        .current_dir(dir.path())
        .output()
        .expect("sh");

    let printed = String::from_utf8_lossy(&said.stdout);
    assert_eq!(
        printed.split('\n').collect::<Vec<_>>(),
        [
            "a b",
            "$(touch pwned)",
            "x;touch pwned",
            "it's",
            "`touch pwned`",
            "",
            ""
        ]
    );
    assert!(
        !dir.path().join("pwned").exists(),
        "a word ran as a command"
    );
}
