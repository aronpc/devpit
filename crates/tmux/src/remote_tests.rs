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
    if !Server::available() {
        return;
    }
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
            // A plain shell, ready at once: a login's own may take its time.
            "sh",
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
    let seen = (0..100).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        server
            .capture_pane(&format!("{session}:"))
            .is_ok_and(|screen| screen.contains("marker-42"))
    });
    assert!(seen, "the line never ran there");
    assert!(!tmp.path().join("pwned").exists());
    let _ = server.kill_server();
}

/// Inside single quotes POSIX shells read nothing, but fish reads `\\` and
/// `\'`: a quoted word holds neither a quote nor a backslash inside its
/// single quotes, so every shell gives it back as it was.
#[test]
fn a_quoted_word_is_read_the_same_by_posix_shells_and_fish() {
    for word in [
        "\\' ; touch pwned #",
        "don\\'t",
        "ends in \\",
        "a\\\\b",
        "'",
        "it's",
    ] {
        let quoted = quoted(word);
        let mut inside = false;
        for ch in quoted.chars() {
            if ch == '\'' && !inside {
                inside = true;
            } else if ch == '\'' {
                inside = false;
            } else if inside {
                assert_ne!(ch, '\\', "a backslash inside single quotes: {quoted}");
            }
        }
    }
}

/// The same words, back through the POSIX shells there are here.
#[cfg(unix)]
#[test]
fn a_quoted_word_comes_back_whole_through_every_shell_here() {
    let words = [
        "\\' ; touch pwned #",
        "don\\'t",
        "ends in \\",
        "a\\\\b",
        "'",
        "",
    ];
    for shell in ["sh", "bash", "zsh", "dash"] {
        let Ok(said) = std::process::Command::new(shell)
            .arg("-c")
            .arg(shell_line(&[&["printf", "%s|"][..], &words[..]].concat()))
            .output()
        else {
            continue;
        };
        assert_eq!(
            String::from_utf8_lossy(&said.stdout),
            format!("{}|", words.join("|")),
            "{shell}"
        );
    }
}

/// tmux is found where a host's own login finds the programs it installed
/// for itself, which a non-interactive one often lacks.
#[test]
fn tmux_on_a_host_is_found_in_the_login_s_own_folders() {
    let argv = argv(
        &["ssh".to_owned(), "--".to_owned(), "host".to_owned()],
        &["list-sessions"],
    );

    let line = argv.last().expect("a line");
    assert!(line.contains("$HOME/.local/bin"), "{line}");
    assert!(line.ends_with("tmux -L devpit list-sessions"), "{line}");
}

/// What a host answers is read up to a ceiling; the rest is not kept.
#[cfg(unix)]
#[test]
fn a_host_s_answer_is_read_up_to_a_ceiling() {
    let child = std::process::Command::new("sh")
        .args(["-c", "head -c 9000000 /dev/zero"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("sh");

    let said = within_bounded(child, std::time::Duration::from_secs(20))
        .expect("answered")
        .expect("read");

    assert_eq!(said.stdout.len(), OUTPUT_MOST);
}

/// A host's session starts quieted like devpit's own: no status bar, a short
/// escape, a long history — set before its first pane is made.
#[cfg(unix)]
#[test]
fn a_host_s_session_starts_quieted_like_devpit_s_own() {
    if !Server::available() {
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let server = over(tmp.path());
    let mut args = quieting();
    args.extend([";", "new-session", "-d", "-s", "devpit_q", "-c", "/"]);

    server.require(&args).expect("started");

    assert_eq!(
        server.format_of("devpit_q:", "#{status}").expect("asked"),
        "off"
    );
    assert_eq!(
        server
            .format_of("devpit_q:", "#{history_limit}")
            .expect("asked"),
        "10000"
    );
    let _ = server.kill_session("devpit_q");
}

/// A process left holding a host's answer open, as an ssh connection's master
/// can, does not keep the answer waiting.
#[cfg(unix)]
#[test]
fn something_left_holding_the_answer_does_not_keep_it_waiting() {
    let started = std::time::Instant::now();
    let child = std::process::Command::new("sh")
        .args(["-c", "sleep 8 & echo done"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("sh");

    let said = within_bounded(child, std::time::Duration::from_secs(20))
        .expect("answered")
        .expect("read");

    assert_eq!(String::from_utf8_lossy(&said.stdout).trim(), "done");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

/// A host's tmux older than an option devpit sets still starts the session:
/// an option it does not know is passed over, never the end of the line.
#[cfg(unix)]
#[test]
fn an_option_a_host_s_tmux_lacks_never_keeps_its_session_from_starting() {
    if !Server::available() {
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let server = over(tmp.path());
    let mut args = quieted(&[["status", "off"], ["no-such-option", "on"]]);
    args.extend([";", "new-session", "-d", "-s", "devpit_old", "-c", "/"]);

    let _ = server.require(&args);

    assert!(
        server.has_exactly("devpit_old").expect("asked"),
        "the session never started"
    );
    let _ = server.kill_session("devpit_old");
}
