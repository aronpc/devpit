//! The slash commands a profile has, as its CLI last reported them.
//!
//! Read off the `init` line a turn prints and kept per profile, so a new
//! conversation can offer them before its first turn — the list differs per
//! installation and per plugin, and a table kept here would be wrong for
//! somebody the day it was written.

use std::path::{Path, PathBuf};

use devpit_rpc::{ErrorCode, RpcError, SessionInit};

fn plain(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\']) && !id.starts_with('.')
}

fn file(home: &Path, profile_id: &str) -> PathBuf {
    home.join("cli-init").join(format!("{profile_id}.json"))
}

/// The commands a chat may offer: the CLI's list, less the ones it says only
/// work in a terminal.
pub(crate) fn offered(init: &SessionInit) -> Vec<String> {
    init.slash_commands
        .iter()
        .filter(|one| !init.terminal_slash_commands.contains(one))
        .cloned()
        .collect()
}

/// Keeps what a turn's CLI reported, for the next conversation of this profile.
pub(crate) fn remember(home: &Path, profile_id: &str, init: Option<&SessionInit>) {
    let Some(init) = init else {
        return;
    };
    if !plain(profile_id) {
        return;
    }
    let path = file(home, profile_id);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(init) {
        let _ = std::fs::write(path, text);
    }
}

pub(crate) fn recall(home: &Path, profile_id: &str) -> Option<SessionInit> {
    if !plain(profile_id) {
        return None;
    }
    let text = std::fs::read_to_string(file(home, profile_id)).ok()?;
    serde_json::from_str(&text).ok()
}

/// `chat.slash_commands` — what the composer offers after `/` for a profile.
///
/// Empty until a turn of that profile has run: nothing is offered that the CLI
/// did not say it has.
#[tauri::command]
#[specta::specta]
pub async fn chat_slash_commands(profile_id: String) -> Result<Vec<String>, RpcError> {
    crate::off_main::blocking(move || chat_slash_commands_now(profile_id)).await
}

/// [`chat_slash_commands`], on the calling thread.
pub(crate) fn chat_slash_commands_now(profile_id: String) -> Result<Vec<String>, RpcError> {
    if !plain(&profile_id) {
        return Err(RpcError::new(ErrorCode::Forbidden, "that is not a profile"));
    }
    Ok(recall(&crate::chat::home(), &profile_id)
        .map(|init| offered(&init))
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init() -> SessionInit {
        SessionInit {
            model: Some("haiku".to_owned()),
            models: vec![devpit_rpc::OfferedModel {
                id: "gpt-5".to_owned(),
                name: "gpt-5".to_owned(),
                unavailable: Some("Invalid model value: gpt-5".to_owned()),
            }],
            slash_commands: vec!["compact".into(), "clear".into(), "statusline".into()],
            terminal_slash_commands: vec!["statusline".into()],
            skills: Vec::new(),
            agents: Vec::new(),
        }
    }

    #[test]
    fn a_command_that_only_works_in_a_terminal_is_not_offered() {
        assert_eq!(offered(&init()), ["compact", "clear"]);
    }

    #[test]
    fn what_a_turn_reported_is_there_for_the_next_conversation() {
        let home = tempfile::tempdir().expect("tempdir");
        remember(home.path(), "claude", Some(&init()));
        assert_eq!(recall(home.path(), "claude"), Some(init()));
    }

    #[test]
    fn a_profile_that_never_ran_offers_nothing() {
        let home = tempfile::tempdir().expect("tempdir");
        assert_eq!(recall(home.path(), "never"), None);
    }

    #[test]
    fn a_profile_id_that_is_a_path_names_no_file() {
        let home = tempfile::tempdir().expect("tempdir");
        remember(home.path(), "../escape", Some(&init()));
        assert!(!home.path().join("escape.json").exists());
        assert!(chat_slash_commands_now("../escape".into()).is_err());
    }
}
