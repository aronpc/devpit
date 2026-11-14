//! The app's preferences that follow the person to another machine.
//!
//! An allowlist, never a denylist: a preference added tomorrow stays on
//! this machine until someone decides here that it travels, and the test
//! beside this file fails until every key is on one list or the other.

use devpit_core::{preference as key, Store};

use crate::space::{Here, Namespace, Seen};

/// The one record preferences are kept under.
pub const RECORD: &str = "app";

/// The most a synced preference may hold; anything longer stays where it is.
pub const LONGEST: usize = 64 * 1024;

/// What syncs: how devpit looks and behaves, the same on every machine.
pub const SYNCED: &[&str] = &[
    key::THEME,
    key::CONFIRM_STOP,
    key::TERMINAL_CONTRAST,
    key::SHORTCUT_KEYS,
    key::FOCUS_MODE,
    key::AGENT_HOOKS,
    key::AGENT_TRUST,
    key::AGENTS_DISABLED,
    key::TRANSCRIBE_LANGUAGE,
    key::DECISIONS_PROVIDER,
    key::DECISIONS_MODEL,
    key::DECISIONS_URL,
    key::DECISIONS_DAILY_CAP,
    key::DECISIONS_MODES,
];

/// What stays on this machine, and why.
pub const LOCAL: &[(&str, &str)] = &[
    (key::ONBOARDED_AT, "this machine's first run"),
    (key::AUTO_UPDATE, "how this install updates itself"),
    (key::WORKTREE_BASE, "a path on this machine"),
    (key::WORKTREE_BRANCH, "this machine's worktrees"),
    (key::TERMINAL_BACKEND, "a daemon running on this machine"),
    (key::SESSION_NEXT_CARD, "sessions this machine starts"),
    (key::SESSION_HANDOFF_PERCENT, "sessions this machine starts"),
    (key::OPEN_IN_APPS, "the apps installed here"),
    (key::WORKTREES_HIDDEN, "this machine's worktrees"),
    (key::WORKTREES_DELETE_MERGED, "this machine's worktrees"),
    (key::WORKTREES_CLEANUP_LAST, "this machine's worktrees"),
    (key::MAINTENANCE_POWER_SAVER, "this machine's power"),
    (key::MAINTENANCE_KEEP_DAYS, "this machine's disk"),
    (key::MAINTENANCE_CLEAN_LAST, "this machine's disk"),
    (key::LABS_PROJECT_AGENT, "a beta tried on this machine"),
    (key::LABS_PROJECT_AGENT_OFF, "a beta tried on this machine"),
    (key::LABS_ACCEPT_FILE_FACTS, "a beta tried on this machine"),
    (key::LABS_CANVAS, "a beta tried on this machine"),
    (key::AGENTS_IDLE_COMPACT, "conversations this machine runs"),
    (
        key::AGENT_PROFILES,
        "config folders on this machine, and tokens",
    ),
    (
        key::AGENT_PROFILES_SYNCED,
        "what sync keeps of this machine's profiles: answers and removals made here",
    ),
    (key::AGENT_DEFAULT, "names a profile of this machine"),
    (key::SIDEBAR_WIDTH, "window state"),
    (key::FILES_WIDTH, "window state"),
    (
        key::HEADS_DOWN,
        "a focus on this machine, by its project id",
    ),
    (key::PAUSED_UNTIL, "a pause on this machine"),
    (
        key::SHORTCUT,
        "a shortcut this machine's system grants or refuses",
    ),
    (key::REMINDERS, "two machines reminding would remind twice"),
    (key::KEEP_IN_TRAY, "the tray"),
    (key::TRAY_TOLD, "the tray"),
    (key::ISLAND, "a window of this machine"),
    (key::ISLAND_SCREEN, "a screen of this machine"),
    (key::ISLAND_ANYWAY, "a window of this machine's display"),
    (key::DISPLAY_BACKEND, "this machine's display server"),
    (key::ERROR_REPORTS, "consent given on this install"),
    (key::TRANSCRIBE, "the engine installed here"),
    (key::TRANSCRIBE_MODEL, "a file on this machine"),
    (key::TRANSCRIBE_URL, "goes with the engine chosen here"),
    (key::REMOTE, "remote access to this machine"),
    (key::REMOTE_PORT, "remote access to this machine"),
    (key::REMOTE_VIA, "remote access to this machine"),
    (key::REMOTE_MCP, "remote access to this machine"),
    (
        key::DECISIONS_OPTED_OUT,
        "by project id of this machine; travels per project instead",
    ),
    (
        key::DEVFLOW,
        "its switch also tells open sessions, which a value from elsewhere would not",
    ),
    (
        key::DEVFLOW_PRESET,
        "new, so it stays here until someone decides it travels",
    ),
    (key::DEVFLOW_ON, "a click on this machine"),
    (
        key::DEVFLOW_COMMANDS,
        "a yes to run code, given on this machine",
    ),
    (key::DEVFLOW_ENFORCED, "a click on this machine"),
    (
        key::DEVFLOW_PROFILES,
        "by project and profile id of this machine",
    ),
    (key::CHANNEL_RULES, "the channels set up on this machine"),
    (
        key::CHANNEL_HUB,
        "two machines would send each notice twice",
    ),
    (key::CHANNEL_HUB_TITLES, "goes with the hub on this machine"),
    (key::CALENDAR_LINK, "holds the calendar's secret"),
    (
        key::CALENDAR_TITLES,
        "goes with the calendar on this machine",
    ),
    (key::SYNC, "sync is turned on per machine"),
    (
        key::AGENTS_PROFILE_MODELS,
        "beside the profiles, whose own sync is apart",
    ),
    (key::PROVIDERS_CHECK_MINUTES, "how often this machine asks"),
    (key::CHAT_MODEL, "the models this machine reaches"),
    (key::CHAT_PERMISSION, "how this machine's chats start"),
    (key::PROJECT_SETTINGS, "the projects on this machine"),
];

/// Whether `name` is a preference that follows the person.
pub fn syncs(name: &str) -> bool {
    SYNCED.contains(&name)
}

/// The preferences namespace, over this machine's store.
pub struct Prefs<'a> {
    pub store: &'a Store,
}

impl Namespace for Prefs<'_> {
    fn name(&self) -> &'static str {
        "prefs"
    }

    fn here(&self) -> Result<Here, String> {
        let mut fields = std::collections::BTreeMap::new();
        for name in SYNCED {
            let Some((value, at)) = self
                .store
                .preference_stamped(name)
                .map_err(|err| err.to_string())?
            else {
                continue;
            };
            if value.len() <= LONGEST {
                let at_ms = u64::try_from(at).ok().map(|seconds| seconds * 1000);
                fields.insert((*name).to_owned(), Seen::of(Some(value), at_ms));
            }
        }
        Ok(Here::from([(RECORD.to_owned(), fields)]))
    }

    fn write(&self, record: &str, field: &str, value: Option<&str>) -> Result<(), String> {
        // A preference is never removed, only set: a cleared one is left.
        let Some(value) = value else { return Ok(()) };
        if record != RECORD || !syncs(field) || value.len() > LONGEST {
            return Ok(());
        }
        self.store
            .set_preference(field, value)
            .map_err(|err| err.to_string())
    }
}

#[cfg(test)]
#[path = "prefs_tests.rs"]
mod tests;
