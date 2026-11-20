//! How an agent reaches the devpit that is running: a CLI and an MCP server,
//! both in the app's own binary.
//!
//! `devpit agent …` and `devpit mcp` are answered here before any window is
//! made. Both post to the listener's `/agent` route through the same door the
//! hooks use — the endpoint and the secret read off disk on every call — so a
//! restarted app is found at its new port, and nothing is written into any
//! tool's own configuration to make this work. What a client on another
//! machine is offered, through the relay, is `remote_mcp`.
//!
//! No dependency on the rest of the app: the caller hands over where devpit
//! keeps its state, and everything else is a socket and some JSON.

#[cfg(test)]
mod agent_rules_tests;
pub mod apps;
mod assistant;
pub mod cli;
pub mod client;
mod consult;
pub mod devflow;
mod devflow_pt_br;
pub mod events;
pub mod guide;
pub mod hook;
mod lean;
mod listing;
pub mod mcp;
mod mcp_pt_br;
pub mod remote_mcp;
mod rest_pt_br;
pub mod skills;
mod slim;

pub use client::ask;
pub use slim::{slimmed, SLIM_ABOVE};

/// Where the agent is standing.
///
/// Not the process's own directory when this runs from an AppImage: the
/// AppImage's launcher changes into its mount before it starts the binary, and
/// leaves the directory it was started from in `OWD`. Read as it was, every
/// question came from `/tmp/.mount_…/usr`, which is no project.
pub fn standing() -> std::path::PathBuf {
    match std::env::var_os("OWD") {
        Some(owd) if !owd.is_empty() => std::path::PathBuf::from(owd),
        _ => std::env::current_dir().unwrap_or_default(),
    }
}
