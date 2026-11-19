use super::*;

/// What runs a line "on the host" here: a shell, with tmux's sockets kept in
/// a folder of the test's own, as ssh would run it with the host's.
#[cfg(unix)]
fn over(tmp: &std::path::Path) -> Server {
    Server::over(vec![
        "env".to_owned(),
        format!("TMUX_TMPDIR={}", tmp.display()),
        "sh".to_owned(),
        "-c".to_owned(),
    ])
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
        "=ls",
        "",
    ];

    let said = std::process::Command::new("sh")
        .arg("-c")
        .arg(shell_line(&words))
        .current_dir(dir.path())
        .output()
        .expect("sh");

    assert_eq!(
        String::from_utf8_lossy(&said.stdout)
            .split('\n')
            .collect::<Vec<_>>(),
        [
            "a b",
            "$(touch pwned)",
            "x;touch pwned",
            "it's",
            "`touch pwned`",
            "=ls",
            "",
            ""
        ]
    );
    assert!(
        !dir.path().join("pwned").exists(),
        "a word ran as a command"
    );
}

/// A server reached through a command is driven like a local one: tmux's
/// own verbs, on a server of devpit's own name, wherever the command runs.
#[cfg(unix)]
#[test]
fn a_server_over_a_command_is_driven_like_a_local_one() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let folder = tmp.path().join("a b $(touch pwned)");
    std::fs::create_dir_all(&folder).expect("folder");
    let server = over(tmp.path());
    let session = "devpit_p__leaf_1";

    server
        .require(&[
            "new-session",
            "-d",
            "-s",
            session,
            "-c",
            &folder.display().to_string(),
        ])
        .expect("started there");

    assert!(server.has_exactly(session).expect("asked"));
    assert!(
        !server.has_exactly("devpit_p").expect("asked"),
        "a prefix matched"
    );
    assert_eq!(
        server
            .pane_path(session, "")
            .map(|path| path.canonicalize().expect("path")),
        Some(folder.canonicalize().expect("folder"))
    );
    server
        .send_keys(&format!("{session}:"), "echo marker-$((40+2))")
        .expect("typed");
    let seen = (0..50).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        server
            .capture_pane(&format!("{session}:"))
            .is_ok_and(|screen| screen.contains("marker-42"))
    });
    assert!(seen, "the line never ran there");
    assert!(!tmp.path().join("pwned").exists());
    let _ = server.kill_server();
}
