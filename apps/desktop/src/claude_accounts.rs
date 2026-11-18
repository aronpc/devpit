//! The Claude Code accounts a session can open in: a name and the
//! `CLAUDE_CONFIG_DIR` its sign-in lives in, kept in `accounts.json` under
//! devpit's home, with the account each project's new sessions use.
//!
//! An account is chosen when a session starts and never changed under one
//! that runs: the variable is on its launch line, and a live process keeps it.

use std::path::{Path, PathBuf};

use devpit_rpc::{Accounts, ClaudeAccount, ErrorCode, ProjectAccount, RpcError};

/// The variable that moves Claude Code's configuration, and with it the account.
pub(crate) const CONFIG_VAR: &str = "CLAUDE_CONFIG_DIR";
/// The driver that reads it.
pub(crate) const DRIVER: &str = "claude";
/// A name is a label, not a document.
const LONGEST_NAME: usize = 60;
/// The person's own few; past this it is not a list someone picks from.
const MOST: usize = 32;
/// The file is small and written only here.
const MOST_BYTES: u64 = 256 * 1024;

pub(crate) fn file(root: &Path) -> PathBuf {
    root.join("accounts.json")
}

/// What the file says, or none when there is no readable file.
pub(crate) fn read_at(root: &Path) -> Accounts {
    let path = file(root);
    if !std::fs::metadata(&path).is_ok_and(|meta| meta.len() <= MOST_BYTES) {
        return Accounts::default();
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub(crate) fn write_at(root: &Path, accounts: &Accounts) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(accounts).map_err(std::io::Error::other)?;
    devpit_core::home::write_private(&file(root), text.as_bytes())
}

/// The list as it may be kept: names trimmed and unique, every directory an
/// absolute path with `~` read as `home`, and project defaults only for
/// accounts that exist.
pub(crate) fn checked(given: Accounts, home: &str) -> Result<Accounts, String> {
    if given.accounts.len() > MOST {
        return Err(format!("at most {MOST} accounts"));
    }
    let mut accounts: Vec<ClaudeAccount> = Vec::new();
    for one in given.accounts {
        let name = one.name.trim().to_owned();
        if name.is_empty() || name.chars().count() > LONGEST_NAME {
            return Err(format!(
                "an account needs a name of up to {LONGEST_NAME} characters"
            ));
        }
        if name.chars().any(char::is_control) {
            return Err("an account name cannot hold control characters".to_owned());
        }
        if accounts.iter().any(|had| had.name == name) {
            return Err(format!("two accounts are called {name}"));
        }
        let dir = devpit_agentcli::declaring::at_home_value(one.config_dir.trim(), home);
        // Typed into a terminal inside quotes: a control character is a key.
        if !Path::new(&dir).is_absolute() || dir.chars().any(char::is_control) {
            return Err(format!("{name} needs its config directory as a full path"));
        }
        accounts.push(ClaudeAccount {
            name,
            config_dir: dir,
        });
    }
    let mut projects: Vec<ProjectAccount> = Vec::new();
    for one in given.projects {
        if accounts.iter().any(|known| known.name == one.account)
            && !projects.iter().any(|had| had.project_id == one.project_id)
        {
            projects.push(one);
        }
    }
    Ok(Accounts { accounts, projects })
}

/// The account `project_id`'s new sessions open in, if it has one.
pub(crate) fn of_project<'a>(
    accounts: &'a Accounts,
    project_id: &str,
) -> Option<&'a ClaudeAccount> {
    let name = &accounts
        .projects
        .iter()
        .find(|one| one.project_id == project_id)?
        .account;
    accounts.accounts.iter().find(|one| &one.name == name)
}

/// `line` started in `config_dir`.
///
/// In front, so a profile that names its own directory still wins: of two
/// assignments to one variable the shell keeps the last.
pub(crate) fn in_dir(line: &str, config_dir: &str) -> String {
    format!(
        "{CONFIG_VAR}={} {line}",
        devpit_agentcli::declaring::quoted(config_dir)
    )
}

fn root() -> Result<PathBuf, RpcError> {
    devpit_core::Store::root().map_err(|err| RpcError::internal(err.to_string()))
}

/// The registry on this machine now.
pub(crate) fn read() -> Accounts {
    root().map(|root| read_at(&root)).unwrap_or_default()
}

/// The directory `project_id`'s new sessions open in, if it has an account.
pub(crate) fn dir_of_project(project_id: &str) -> Option<String> {
    of_project(&read(), project_id).map(|one| one.config_dir.clone())
}

/// `accounts.read` — the accounts and each project's default.
#[tauri::command]
#[specta::specta]
pub async fn accounts_read() -> Result<Accounts, RpcError> {
    crate::off_main::blocking(|| Ok(read())).await
}

/// `accounts.save` — the whole list, as the screen holds it.
#[tauri::command]
#[specta::specta]
pub async fn accounts_save(accounts: Accounts) -> Result<Accounts, RpcError> {
    crate::off_main::blocking(move || {
        let home = crate::installations::home()?;
        let kept = checked(accounts, &home.to_string_lossy())
            .map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
        write_at(&root()?, &kept).map_err(|err| RpcError::internal(err.to_string()))?;
        Ok(kept)
    })
    .await
}

#[cfg(test)]
#[path = "claude_accounts_tests.rs"]
mod tests;
