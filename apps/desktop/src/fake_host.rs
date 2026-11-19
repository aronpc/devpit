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

    pub(crate) fn bin(&self) -> PathBuf {
        self.dir.path().join("bin")
    }

    pub(crate) fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    pub(crate) fn ssh(&self) -> PathBuf {
        self.dir.path().join("ssh")
    }

    pub(crate) fn program(&self, path: &Path, body: &str) {
        std::fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("write");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
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
