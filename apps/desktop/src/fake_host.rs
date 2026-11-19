//! A host on this computer, for tests of what devpit runs over ssh.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// A host on this computer: a fake `ssh` that runs the line it is handed in
/// `sh`, a home of its own, and only the programs put in its `bin`.
pub(crate) struct FakeHost {
    pub(crate) dir: tempfile::TempDir,
}

impl FakeHost {
    pub(crate) fn new() -> Self {
        // Under /tmp, not $TMPDIR: macOS's is long enough that a socket under
        // it (ssh's, tmux's) outgrows a unix socket's 104 bytes.
        let host = Self {
            dir: tempfile::Builder::new()
                .prefix("fh")
                .tempdir_in("/tmp")
                .expect("tempdir"),
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

    pub(crate) fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }

    pub(crate) fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    pub(crate) fn ssh(&self) -> PathBuf {
        self.dir.path().join("ssh")
    }

    /// Writes a program, then runs it once to no effect until it runs: a
    /// child another test's thread forked while it was being written holds it
    /// open, and running it then fails until that child runs its own.
    pub(crate) fn program(&self, path: &Path, body: &str) {
        std::fs::write(
            path,
            format!("#!/bin/sh\n[ -n \"$FAKE_PROBE\" ] && exit 0\n{body}\n"),
        )
        .expect("write");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        for _ in 0..300 {
            let ran = std::process::Command::new(path)
                .env("FAKE_PROBE", "1")
                .status();
            if ran.is_ok_and(|status| status.success()) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("{} never ran", path.display());
    }

    pub(crate) fn installs(&self, name: &str, says: &str) {
        self.program(&self.bin().join(name), &format!("echo '{says}'"));
    }

    pub(crate) fn signs_in_claude(&self) {
        let dir = self.home().join(".claude");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join(".credentials.json"), "{}").expect("credentials");
    }

    /// The variables the host's commands see: its own PATH and home.
    pub(crate) fn envs(&self) -> Vec<(String, String)> {
        vec![
            ("PATH".to_owned(), self.bin().display().to_string()),
            ("HOME".to_owned(), self.home().display().to_string()),
        ]
    }

    /// A folder on the host, made in its home.
    pub(crate) fn folder(&self, name: &str) -> PathBuf {
        let dir = self.home().join(name);
        std::fs::create_dir_all(&dir).expect("folder");
        dir
    }
}

impl FakeHost {
    /// A host whose ssh runs tmux: its own home, PATH and tmux sockets, set
    /// by its ssh as the real one sets the host's.
    /// Answers whether this machine has a tmux to stand in for the host's:
    /// without one, a test of a host's tmux has nothing to run.
    pub(crate) fn with_tmux(&self) -> bool {
        let Some(found) = tmux_folder() else {
            eprintln!("no tmux on this machine: a host's tmux is not tried");
            return false;
        };
        std::fs::create_dir_all(self.dir.path().join("tmux")).expect("tmux dir");
        self.program(
            &self.ssh(),
            &format!(
                "export HOME='{home}' PATH='{bin}:{found}:/usr/bin:/bin' SHELL=/bin/sh TMUX_TMPDIR='{tmux}'\nwhile [ \"$1\" != -- ]; do shift; done\nshift 2\nexec /bin/sh -c \"$1\"",
                home = self.home().display(),
                bin = self.bin().display(),
                found = found.display(),
                tmux = self.dir.path().join("tmux").display(),
            ),
        );
        true
    }

    /// The tmux server of the host, as devpit drives it.
    pub(crate) fn tmux(&self) -> devpit_tmux::Server {
        devpit_tmux::Server::over(vec![
            self.ssh().display().to_string(),
            "--".to_owned(),
            "host".to_owned(),
        ])
    }
}

impl Drop for FakeHost {
    fn drop(&mut self) {
        // Every session ended, its server ends with them.
        if self.dir.path().join("tmux").exists() {
            let _ = std::process::Command::new(self.ssh())
                .args([
                    "--",
                    "host",
                    "tmux -L devpit list-sessions -F '#{session_name}' | while read -r s; do tmux -L devpit kill-session -t \"=$s\"; done",
                ])
                .output();
        }
    }
}

/// The folder this machine's tmux is in, on the PATH.
fn tmux_folder() -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).find(|dir| dir.join("tmux").is_file())
}
