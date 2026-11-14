//! Finding the profiles this machine can run.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::driver::driver;
pub use devpit_rpc::{Declared, EnvVar, Profile, Reach};

/// What a base agent lends to the profiles built on it.
///
/// Resolved by the caller rather than looked up here: the agent catalogue
/// lives in `devpit_pty`, which this crate does not depend on and should not
/// start depending on to answer two questions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    /// The program a profile runs when it names none of its own.
    pub program: String,
    /// Which driver reads its output, empty when nothing can.
    pub driver: String,
}

/// Where a command resolves to, if anywhere. On Windows `claude` is
/// `claude.exe` or `claude.cmd`, and the name alone is no file.
#[cfg(windows)]
pub fn found(command: &str) -> Option<String> {
    devpit_pty::process::found_on_path(command).map(|found| found.to_string_lossy().into_owned())
}

/// Where a command resolves to, if anywhere: on the path devpit's children
/// get, not the desktop's. `~/` names the home folder.
#[cfg(not(windows))]
pub fn found(command: &str) -> Option<String> {
    let command = expanded(command, std::env::var_os("HOME").as_deref());
    std::env::split_paths(devpit_pty::login_path::search_path())
        .map(|dir| dir.join(&command))
        .find(is_runnable)
        .map(|found| found.to_string_lossy().into_owned())
}

/// `~/x` under `home`; anything else as written.
pub(crate) fn expanded(command: &str, home: Option<&std::ffi::OsStr>) -> PathBuf {
    match (command.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(command),
    }
}

#[cfg(unix)]
fn is_runnable(path: &PathBuf) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// How far this machine gets with a command.
///
/// `shell_knows` is whether the person's own shell can run the name — which is
/// a different question from whether a file exists, and the only one that is
/// right about a function defined in their `.zshrc`. Asking the shell costs an
/// interactive shell, so it is asked once by the caller and passed in here
/// rather than asked per command.
///
/// A file wins over shell knowledge rather than being weighed against it: when
/// both are true the shell would run the file anyway, and `Runnable` is the
/// answer that lets devpit spawn it.
pub fn reach(command: &str, shell_knows: bool) -> (Reach, Option<String>) {
    match found(command) {
        Some(path) => (Reach::Runnable, Some(path)),
        None if shell_knows => (Reach::ShellOnly, None),
        None => (Reach::Missing, None),
    }
}

/// What the driver offers, or nothing when no driver answers to that name.
fn models_of(name: &str) -> Vec<String> {
    driver(name)
        .map(|found| found.models().iter().map(|&m| m.to_owned()).collect())
        .unwrap_or_default()
}

/// A profile's own models, led by `default` — what the account runs with no
/// `--model` at all is always a choice — or the driver's when it named none.
pub fn offered(own: &[String], driver: &str) -> Vec<String> {
    if own.is_empty() {
        return models_of(driver);
    }
    let mut all = Vec::with_capacity(own.len() + 1);
    if !own.iter().any(|one| one == DEFAULT_MODEL) {
        all.push(DEFAULT_MODEL.to_owned());
    }
    all.extend(own.iter().cloned());
    all
}

/// What the CLI is passed when nobody picked a model.
const DEFAULT_MODEL: &str = "default";

/// Whether the driver reports cost and context; a profile with none reports nothing either way.
fn metered(name: &str) -> bool {
    driver(name).is_none_or(|found| found.metered())
}

fn efforts_of(name: &str) -> (Vec<String>, Option<String>) {
    driver(name)
        .map(|found| {
            (
                found.efforts().iter().map(|&e| e.to_owned()).collect(),
                found.effort_default().map(ToOwned::to_owned),
            )
        })
        .unwrap_or_default()
}

/// The commands worth looking for when nothing has been declared, as
/// `(command, label, driver)`.
///
/// Named for what it is rather than `KNOWN`, which is already the agent
/// catalogue in `devpit_pty::agents` and answers a different question.
pub const DISCOVERED: &[(&str, &str, &str)] = &[("claude", "Claude Code", "claude")];

/// Every profile: the declared ones first, then the discovered commands this
/// machine can reach and that are not already named.
///
/// `base` resolves a base agent id into what it lends; `shell_knows` holds the
/// command names the person's own shell can run, asked once by the caller. A
/// name missing from both is the only thing that counts as absent.
pub fn profiles(
    declared: &[Declared],
    base: impl Fn(&str) -> Option<Base>,
    shell_knows: &HashSet<String>,
) -> Vec<Profile> {
    let knows = |command: &str| shell_knows.contains(command);

    let mut all: Vec<Profile> = declared
        .iter()
        .map(|one| {
            // A base that vanished from the build leaves the profile listed
            // and driverless rather than gone: somebody chose it, and dropping
            // their row explains nothing.
            let lent = base(&one.base).unwrap_or_else(|| Base {
                program: one.command.clone(),
                driver: String::new(),
            });
            let command = if one.command.is_empty() {
                lent.program
            } else {
                one.command.clone()
            };
            let (efforts, effort_default) = efforts_of(&lent.driver);
            let (how, path) = reach(&command, knows(&command));
            Profile {
                id: one.id.clone(),
                label: one.label.clone(),
                command,
                driver: lent.driver.clone(),
                path,
                reach: how,
                base: one.base.clone(),
                args: one.args.clone(),
                env: one.env.clone(),
                mine: true,
                models: offered(&one.models, &lent.driver),
                own_models: one.models.clone(),
                efforts,
                effort_default,
                metered: metered(&lent.driver),
                enabled: true,
            }
        })
        .collect();

    for (command, label, driver) in DISCOVERED {
        // Named already only by the same thing: its own id (an override of
        // it), or the same command started exactly the same way. A `glm` or a
        // second sign-in runs `claude` too, with variables of its own — it is
        // another account, not this one, and hiding the plain one behind it
        // left nobody able to pick their default sign-in.
        let named = all.iter().any(|profile| {
            profile.id == *command
                || (profile.command == *command
                    && profile.env.is_empty()
                    && profile.args.is_empty())
        });
        if named {
            continue;
        }
        // Discovery, so something absent is left out entirely — a declared
        // profile is a choice somebody made and stays listed, a discovered one
        // that is nowhere is just noise.
        let (how, path) = reach(command, knows(command));
        if how == Reach::Missing {
            continue;
        }
        let (efforts, effort_default) = efforts_of(driver);
        all.push(Profile {
            id: (*command).to_owned(),
            label: (*label).to_owned(),
            command: (*command).to_owned(),
            driver: (*driver).to_owned(),
            path,
            reach: how,
            base: String::new(),
            args: Vec::new(),
            env: Vec::new(),
            mine: false,
            models: models_of(driver),
            own_models: Vec::new(),
            efforts,
            effort_default,
            metered: metered(driver),
            enabled: true,
        });
    }
    all
}
