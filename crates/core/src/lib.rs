//! Core of devpit.
//!
//! All product logic lives here and in the other `crates/`. The desktop shell,
//! the HTTP server and the CLI are thin adapters over it.
//!
//! The rule that holds the design together — checked by `cargo xtask check` —
//! is that nothing under `crates/` imports `tauri`. That is what keeps the
//! shell choice reversible: if WebKitGTK on Linux rules Tauri out, swapping
//! shells costs days instead of months.

pub mod bus;
pub mod data_files;
pub mod home;
pub mod i18n;
pub mod locale;
#[cfg(test)]
mod locale_tests;
pub mod origin;
pub mod paths;
#[cfg(test)]
mod paths_guard_tests;
#[cfg(test)]
mod paths_tests;
pub mod reports;
pub mod store;
pub mod tree;
mod tree_error;
#[cfg(test)]
mod tree_tests;
pub mod words;

pub use bus::{Bus, Event, Severity};
pub use store::{
    limits, preference, AttachmentRow, CardEdges, CardRow, ColumnRow, CommentRow, HostRow, NewCard,
    NoticeRow, PaneLayoutWrite, PluginRow, ProjectRow, ReminderRow, RunRow, SessionLink, StepRow,
    Store, StoreError, SyncedProject, DEFAULT_COLUMNS, DEV_ROOT, RELEASE_ROOT, ROOT_NAME,
};
pub use tree_error::TreeError;

/// Held by whatever reads, changes and writes back the agent profiles or
/// what sync keeps about them: each is one preference written whole, and two
/// writers at once would each drop the other's change.
pub static PROFILES_WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());
