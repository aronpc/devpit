use super::*;
use std::ffi::OsStr;

#[test]
fn the_path_is_read_past_what_the_shell_config_prints() {
    let said = "welcome back\nsome banner __devpit_path__ no\n__devpit_path__/home/me/.tool/bin:/usr/bin\n";
    assert_eq!(marked(said).as_deref(), Some("/home/me/.tool/bin:/usr/bin"));
    assert_eq!(marked("nothing marked"), None);
}

#[test]
fn the_shell_entries_lead_and_none_is_lost_or_doubled() {
    let path = joined(
        OsStr::new("/home/me/.tool/bin:/usr/bin"),
        OsStr::new("/usr/bin:/snap/bin"),
    );
    assert_eq!(path, OsStr::new("/home/me/.tool/bin:/usr/bin:/snap/bin"));
}

/// The installers' folders: Claude's own, npm, bun, Go, brew on Apple silicon.
#[cfg(unix)]
#[test]
fn the_usual_install_folders_are_known() {
    let home = Path::new("/Users/me");
    let dirs = known_dirs(home);
    for wanted in [
        "/Users/me/.local/bin",
        "/Users/me/.claude/local",
        "/Users/me/.npm-global/bin",
        "/Users/me/.bun/bin",
        "/Users/me/.local/go/bin",
        "/Users/me/go/bin",
        "/Users/me/.opencode/bin",
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/local/go/bin",
    ] {
        assert!(
            dirs.contains(&PathBuf::from(wanted)),
            "{wanted} is not searched"
        );
    }
}

/// nvm keeps one folder per Node; the newest by version is the one searched.
#[cfg(unix)]
#[test]
fn the_newest_nvm_node_is_searched() {
    let home = tempfile::tempdir().expect("tempdir");
    for version in ["v18.20.1", "v22.3.0", "v9.11.2"] {
        std::fs::create_dir_all(
            home.path()
                .join(".nvm/versions/node")
                .join(version)
                .join("bin"),
        )
        .expect("dir");
    }
    let dirs = known_dirs(home.path());
    assert!(dirs.contains(&home.path().join(".nvm/versions/node/v22.3.0/bin")));
    assert!(!dirs.contains(&home.path().join(".nvm/versions/node/v9.11.2/bin")));
}

#[test]
fn folders_are_added_after_the_path_once() {
    let path = with_dirs(
        OsStr::new("/usr/bin:/opt/homebrew/bin"),
        &[
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/Users/me/.local/bin"),
        ],
    );
    assert_eq!(
        path,
        OsStr::new("/usr/bin:/opt/homebrew/bin:/Users/me/.local/bin")
    );
}

/// A shell whose config never returns is ended, with what it started.
#[cfg(unix)]
#[test]
fn a_shell_that_hangs_is_given_up_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    let shell = dir.path().join("slow-shell");
    std::fs::write(&shell, "#!/bin/sh\nsleep 30\n").expect("write");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let started = Instant::now();
    let (outcome, path) = ask(shell.to_str().unwrap());
    assert_eq!((outcome, path), ("timed out", None));
    assert!(started.elapsed() < WAITS + Duration::from_secs(2));
}

/// GNOME starts apps with `SHLVL=0`: that is no shell, and its `PATH` is not
/// the person's, so the shell is asked.
#[test]
fn only_a_shell_level_above_zero_means_started_from_a_shell() {
    for (level, from_a_shell) in [
        (None, false),
        (Some("0"), false),
        (Some(""), false),
        (Some("not a number"), false),
        (Some("1"), true),
        (Some("3"), true),
    ] {
        assert_eq!(
            super::from_a_shell(level.map(OsStr::new)),
            from_a_shell,
            "SHLVL={level:?}"
        );
    }
}

/// An AppImage's mount reaches the shell's answer through the `PATH` it was
/// asked with; it moves on every start, so it never stays on the search path.
#[cfg(unix)]
#[test]
fn the_search_path_keeps_the_known_folders_and_drops_an_appimage_mount() {
    let home = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(home.path().join("go/bin")).expect("dir");
    let base = OsStr::new("/tmp/.mount_devpitAb12/usr/bin:/usr/bin:/tmp/.mount_old/bin");
    let path = searched(base, Some(home.path()), "/tmp/.mount_");
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    assert_eq!(dirs[0], PathBuf::from("/usr/bin"));
    assert!(dirs.contains(&home.path().join("go/bin")), "{path:?}");
    assert!(
        !dirs.iter().any(|dir| dir.starts_with("/tmp/.mount_old")),
        "{path:?}"
    );
    assert!(
        !dirs
            .iter()
            .any(|dir| dir.starts_with("/tmp/.mount_devpitAb12")),
        "{path:?}"
    );
    // Away from an AppImage nothing is taken out.
    let plain = searched(OsStr::new("/usr/bin:/bin"), None, "/tmp/.mount_");
    assert_eq!(plain, OsStr::new("/usr/bin:/bin"));
}
