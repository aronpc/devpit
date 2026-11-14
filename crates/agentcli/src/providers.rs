//! The CLIs devpit holds a chat with, or will: found on this machine, with
//! their version, whether they are signed in, and the lines that install and
//! sign them in. devpit never signs anyone in itself: the login runs in a
//! terminal, where the person finishes it.

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use devpit_rpc::{Credentials, ProviderFound};

use crate::running::{line, Runner};

/// Where a CLI keeps the mark of a sign-in, as far as devpit can read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keeps {
    /// `.credentials.json` under `CLAUDE_CONFIG_DIR`, else `~/.claude`.
    Claude,
    /// `auth.json` under `CODEX_HOME`, else `~/.codex`.
    Codex,
    /// `opencode/auth.json` under `XDG_DATA_HOME`, holding a provider.
    OpenCode,
    /// `~/.cursor/cli-config.json`, naming the account in `authInfo`.
    Cursor,
    /// Kept where devpit cannot read it, such as a browser session.
    Nowhere,
}

/// One supported CLI, by its id in the agent catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Supported {
    pub id: &'static str,
    pub install: &'static str,
    pub install_windows: &'static str,
    /// What follows the program to sign in. Empty when starting it signs in.
    pub login: &'static [&'static str],
    pub keeps: Keeps,
}

/// The CLIs Settings → Providers lists, in its order.
pub const SUPPORTED: &[Supported] = &[
    Supported {
        id: "claude",
        install: "curl -fsSL https://claude.ai/install.sh | bash",
        install_windows: "irm https://claude.ai/install.ps1 | iex",
        login: &["auth", "login"],
        keeps: Keeps::Claude,
    },
    Supported {
        id: "codex",
        install: "npm install -g @openai/codex",
        install_windows: "npm install -g @openai/codex",
        login: &["login"],
        keeps: Keeps::Codex,
    },
    Supported {
        id: "opencode",
        install: "curl -fsSL https://opencode.ai/install | bash",
        install_windows: "npm install -g opencode-ai",
        login: &["auth", "login"],
        keeps: Keeps::OpenCode,
    },
    Supported {
        id: "cursor",
        install: "curl https://cursor.com/install -fsS | bash",
        install_windows: "irm 'https://cursor.com/install?win32=true' | iex",
        login: &["login"],
        keeps: Keeps::Cursor,
    },
    Supported {
        id: "grok",
        install: "curl -fsSL https://x.ai/cli/install.sh | bash",
        install_windows: "irm https://x.ai/cli/install.ps1 | iex",
        login: &["login"],
        keeps: Keeps::Nowhere,
    },
    Supported {
        id: "agy",
        install: "curl -fsSL https://antigravity.google/cli/install.sh | bash",
        install_windows: "irm https://antigravity.google/cli/install.ps1 | iex",
        login: &[],
        keeps: Keeps::Nowhere,
    },
];

/// How long `--version` may take before the CLI is listed without one.
const VERSION_WAIT: Duration = Duration::from_secs(5);

/// The most of a sign-in file read to tell whether it holds one.
const READ_CEILING: u64 = 256 * 1024;

pub fn supported(id: &str) -> Option<&'static Supported> {
    SUPPORTED.iter().find(|one| one.id == id)
}

/// The first runnable file called `program` on the `PATH`-style `path`; on
/// Windows also its `.exe`, `.cmd` or `.bat`.
pub fn locate(program: &str, path: &OsStr) -> Option<PathBuf> {
    if let Some(found) = devpit_pty::host_env::found_on(path, OsStr::new(program)) {
        return Some(found);
    }
    if !cfg!(windows) {
        return None;
    }
    ["exe", "cmd", "bat"].iter().find_map(|ext| {
        devpit_pty::host_env::found_on(path, OsStr::new(&format!("{program}.{ext}")))
    })
}

/// The version in what `--version` printed: its first word that starts with
/// a digit, a leading `v` dropped.
pub fn version_in(said: &str) -> Option<String> {
    let line = said.lines().map(str::trim).find(|one| !one.is_empty())?;
    line.split_whitespace()
        .map(|word| word.strip_prefix('v').unwrap_or(word))
        .map(|word| word.trim_end_matches([',', ';', ')']))
        .find(|word| word.starts_with(|first: char| first.is_ascii_digit()))
        .map(|word| word.chars().take(40).collect())
}

/// Runs `program --version` with `path` as its `PATH`, waiting at most `wait`.
pub fn version_of(program: &Path, path: &OsStr, wait: Duration) -> Option<String> {
    let mut command = devpit_pty::host_env::command(program);
    command
        .arg("--version")
        .env("PATH", path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < wait => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut said = String::new();
    child
        .stdout
        .take()?
        .take(READ_CEILING)
        .read_to_string(&mut said)
        .ok()?;
    version_in(&said)
}

/// Whether the CLI is signed in, read where it keeps that. `var` answers the
/// variables the CLI would see: a profile's own, then this process's.
pub fn sign_in(keeps: Keeps, home: &Path, var: &dyn Fn(&str) -> Option<String>) -> Credentials {
    let under = |name: &str, otherwise: &str| {
        var(name)
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(otherwise))
    };
    match keeps {
        Keeps::Claude => crate::cli_config::sign_in_at(&under("CLAUDE_CONFIG_DIR", ".claude")),
        Keeps::Codex => {
            if under("CODEX_HOME", ".codex").join("auth.json").is_file() {
                Credentials::Saved
            } else {
                Credentials::Missing
            }
        }
        Keeps::OpenCode => {
            let file = under("XDG_DATA_HOME", ".local/share").join("opencode/auth.json");
            json_holds(&file, |value| {
                value.as_object().is_some_and(|all| !all.is_empty())
            })
        }
        Keeps::Cursor => json_holds(&home.join(".cursor/cli-config.json"), |value| {
            value.get("authInfo").is_some_and(|info| !info.is_null())
        }),
        Keeps::Nowhere => Credentials::Unknown,
    }
}

/// Saved when the JSON file says so, missing when it is absent or does not,
/// unknown when it cannot be read. Nothing read is kept.
fn json_holds(file: &Path, holds: impl Fn(&serde_json::Value) -> bool) -> Credentials {
    let Ok(opened) = std::fs::File::open(file) else {
        return Credentials::Missing;
    };
    let mut text = String::new();
    if opened.take(READ_CEILING).read_to_string(&mut text).is_err() {
        return Credentials::Unknown;
    }
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) if holds(&value) => Credentials::Saved,
        Ok(_) => Credentials::Missing,
        Err(_) => Credentials::Unknown,
    }
}

/// `path` as a person reads it: from `~` when it is under the home folder.
fn from_home(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => {
            format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display())
        }
        _ => path.display().to_string(),
    }
}

/// The line that signs `base` in, run as `runner` with its variables and
/// without its own arguments; none for a CLI that is not supported.
pub fn login_line(base: &str, runner: &Runner) -> Option<String> {
    let one = supported(base)?;
    Some(line(&Runner {
        program: runner.program.clone(),
        args: one.login.iter().map(|&arg| arg.to_owned()).collect(),
        env: runner.env.clone(),
    }))
}

/// Every supported CLI as this machine has it. Whether it can hold a chat is
/// the agent catalogue's: a driver named there.
pub fn detect(
    path: &OsStr,
    home: &Path,
    var: &(dyn Fn(&str) -> Option<String> + Sync),
) -> Vec<ProviderFound> {
    let found: Vec<Option<PathBuf>> = SUPPORTED
        .iter()
        .map(|one| {
            let program = devpit_pty::agents::known(one.id).map_or(one.id, |agent| agent.launch);
            locate(program, path)
        })
        .collect();
    // The versions in parallel: each is a process start, a Node one at that.
    let versions: Vec<Option<String>> = std::thread::scope(|scope| {
        let asked: Vec<_> = found
            .iter()
            .map(|at| {
                scope.spawn(move || {
                    at.as_deref()
                        .and_then(|at| version_of(at, path, VERSION_WAIT))
                })
            })
            .collect();
        asked
            .into_iter()
            .map(|one| one.join().ok().flatten())
            .collect()
    });
    SUPPORTED
        .iter()
        .zip(found)
        .zip(versions)
        .map(|((one, at), version)| {
            let agent = devpit_pty::agents::known(one.id);
            let program = agent.map_or(one.id, |agent| agent.launch);
            let plain = Runner {
                program: program.to_owned(),
                args: Vec::new(),
                env: Vec::new(),
            };
            ProviderFound {
                id: one.id.to_owned(),
                label: agent.map_or(one.id, |agent| agent.label).to_owned(),
                signed_in: if at.is_some() {
                    sign_in(one.keeps, home, var)
                } else {
                    Credentials::Unknown
                },
                path: at.map(|at| from_home(&at, home)),
                version,
                chat: agent.is_some_and(|agent| !agent.driver.is_empty()),
                install: if cfg!(windows) {
                    one.install_windows
                } else {
                    one.install
                }
                .to_owned(),
                login: login_line(one.id, &plain).unwrap_or_default(),
                homepage: agent.map_or("", |agent| agent.homepage).to_owned(),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "providers_tests.rs"]
mod tests;
