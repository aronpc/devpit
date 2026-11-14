//! The `PATH` devpit's children are started with.
//!
//! An app opened from the desktop (Finder, a launcher) inherits a minimal
//! `PATH`, not the one the person's login shell builds. So the shell is asked
//! once, and the usual install folders are added after it.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Long enough for a slow shell config; a shell that takes longer is left.
const WAITS: Duration = Duration::from_secs(4);
const MARK: &str = "__devpit_path__";

/// How the login shell's `PATH` was read, for the diagnostics.
#[derive(Debug, Clone)]
pub struct Asked {
    pub shell: Option<String>,
    /// `read`, `started from a shell`, `no SHELL`, `timed out`, `failed`.
    pub outcome: &'static str,
    pub took_ms: u128,
    pub path: Option<OsString>,
}

/// The login shell's answer, asked once.
pub fn asked() -> &'static Asked {
    static ASKED: OnceLock<Asked> = OnceLock::new();
    ASKED.get_or_init(|| {
        let shell = std::env::var("SHELL").ok().filter(|one| !one.is_empty());
        let started = Instant::now();
        let (outcome, path) = if from_a_shell(std::env::var_os("SHLVL").as_deref()) {
            ("started from a shell", None)
        } else if let Some(shell) = &shell {
            ask(shell)
        } else {
            ("no SHELL", None)
        };
        Asked {
            shell,
            outcome,
            took_ms: started.elapsed().as_millis(),
            path,
        }
    })
}

/// Whether this process was started from a shell, whose `PATH` it then has.
/// A shell counts itself in `SHLVL` from 1; GNOME starts apps with `SHLVL=0`,
/// and a value nobody can read is asked about too, which costs one slow start.
pub(crate) fn from_a_shell(shlvl: Option<&OsStr>) -> bool {
    shlvl
        .and_then(OsStr::to_str)
        .and_then(|level| level.trim().parse::<i64>().ok())
        .is_some_and(|level| level > 0)
}

/// The shell's `PATH` joined with this process's. `None` when it was not read.
pub fn login_path() -> Option<&'static OsString> {
    static FOUND: OnceLock<Option<OsString>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            let said = asked().path.as_ref()?;
            Some(joined(said, &std::env::var_os("PATH").unwrap_or_default()))
        })
        .as_ref()
}

/// What children are started with: the login `PATH` (or this process's), then
/// the usual install folders that exist and are not on it yet.
pub fn search_path() -> &'static OsString {
    static PATH: OnceLock<OsString> = OnceLock::new();
    PATH.get_or_init(|| {
        let base = login_path()
            .cloned()
            .or_else(|| std::env::var_os("PATH"))
            .unwrap_or_default();
        let home = std::env::var_os("HOME").map(PathBuf::from);
        searched(&base, home.as_deref(), &crate::host_env::mount_prefix())
    })
}

/// [`search_path`] read again, for a CLI installed into a folder that did not
/// exist when devpit started. The login shell's `PATH` is still the one asked once.
pub fn search_path_now() -> OsString {
    let base = login_path()
        .cloned()
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    searched(&base, home.as_deref(), &crate::host_env::mount_prefix())
}

/// `base`, then the known folders under `home` that exist, without an AppImage
/// mount's entries: the shell was asked from inside one, and a mount moves.
pub(crate) fn searched(base: &OsStr, home: Option<&Path>, mounts: &str) -> OsString {
    let extra: Vec<PathBuf> = home
        .map(known_dirs)
        .unwrap_or_default()
        .into_iter()
        .filter(|dir| dir.is_dir())
        .collect();
    let path = with_dirs(base, &extra);
    let Some(text) = path.to_str() else {
        return path;
    };
    match crate::host_env::cleaned([("PATH".to_owned(), text.to_owned())], mounts).pop() {
        Some((_, kept)) => OsString::from(kept.unwrap_or_default()),
        None => path,
    }
}

/// Where agent CLIs and their runtimes are commonly installed, in the order
/// they are tried after the shell's own `PATH`.
pub fn known_dirs(home: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        return Vec::new();
    }
    let mut dirs: Vec<PathBuf> = [
        ".local/bin",
        ".claude/local",
        ".npm-global/bin",
        ".bun/bin",
        ".volta/bin",
        ".asdf/shims",
        ".local/share/mise/shims",
        ".cargo/bin",
        // Go's tarball unpacked at home, and what `go install` puts in GOPATH.
        ".local/go/bin",
        "go/bin",
        ".opencode/bin",
    ]
    .iter()
    .map(|dir| home.join(dir))
    .collect();
    dirs.extend(newest_nvm_node(home));
    dirs.extend(
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/local/go/bin",
            "/home/linuxbrew/.linuxbrew/bin",
        ]
        .iter()
        .map(PathBuf::from),
    );
    dirs
}

/// The `bin` of the newest Node nvm installed, by version number.
fn newest_nvm_node(home: &Path) -> Option<PathBuf> {
    let versions = std::fs::read_dir(home.join(".nvm/versions/node")).ok()?;
    versions
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let numbers: Vec<u64> = name
                .trim_start_matches('v')
                .split('.')
                .map(|part| part.parse().ok())
                .collect::<Option<_>>()?;
            Some((numbers, entry.path().join("bin")))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, bin)| bin)
}

/// Runs the person's shell as a login, interactive shell and reads its `PATH`
/// off a marked line, past whatever its config prints.
fn ask(shell: &str) -> (&'static str, Option<OsString>) {
    let mut command = Command::new(shell);
    command
        .args(["-lic", &format!("printf '\\n{MARK}%s\\n' \"$PATH\"")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Not with an AppImage's libraries, which would load into the shell too.
    crate::host_env::scrub(&mut command);
    // A group of its own, so a timeout ends what the config started too.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let Ok(mut child) = command.spawn() else {
        return ("failed", None);
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > WAITS => {
                end_group(&mut child);
                return ("timed out", None);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return ("failed", None),
        }
    }
    let Ok(output) = child.wait_with_output() else {
        return ("failed", None);
    };
    match marked(&String::from_utf8_lossy(&output.stdout)) {
        Some(path) => ("read", Some(OsString::from(path))),
        None => ("failed", None),
    }
}

fn end_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    // SAFETY: signals the group this function's child leads; no memory is read.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// The value on the marked line.
pub(crate) fn marked(said: &str) -> Option<String> {
    said.lines()
        .find_map(|line| line.strip_prefix(MARK))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
}

/// The shell's entries first, then this process's that it lacks.
pub(crate) fn joined(shell: &OsStr, own: &OsStr) -> OsString {
    with_dirs(shell, &std::env::split_paths(own).collect::<Vec<_>>())
}

/// `path` with `dirs` appended, each once.
pub(crate) fn with_dirs(path: &OsStr, dirs: &[PathBuf]) -> OsString {
    let mut all: Vec<PathBuf> = std::env::split_paths(path).collect();
    for one in dirs {
        if !all.contains(one) {
            all.push(one.clone());
        }
    }
    std::env::join_paths(all).unwrap_or_else(|_| path.to_owned())
}

#[cfg(test)]
#[path = "login_path_tests.rs"]
mod tests;
