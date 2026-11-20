//! The profile a run was started under, as a snapshot.
//!
//! A profile can be edited or deleted after a run, and the run's page should
//! still say which account and gateway it spent. This is the pure half: it
//! reads a profile and a step and returns what to keep, touching no store and
//! starting no process.
//!
//! What it keeps is what the sync already considers safe to travel
//! ([`devpit_sync::profiles::shared`]), so a token can never reach a run's
//! record by a path the sync would have refused.

use devpit_rpc::Declared;
use serde::{Deserialize, Serialize};

/// What a run remembers about the profile it ran under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PinnedProfile {
    pub id: String,
    pub label: String,
    pub agent: String,
    /// The account's config folder, with the home directory written as `~`.
    pub config_dir: Option<String>,
    /// The gateway, without any credentials the URL carried.
    pub base_url: Option<String>,
    /// The model devpit asked the CLI for: the step's, else the profile's.
    pub requested_model: Option<String>,
}

/// The first variable the sync would share that `wants` by name.
fn shared_value(declared: &Declared, wants: impl Fn(&str) -> bool) -> Option<&str> {
    declared
        .env
        .iter()
        .find(|var| wants(&var.name) && devpit_sync::profiles::shared(&var.name))
        .map(|var| var.value.as_str())
        .filter(|value| !value.is_empty())
}

/// A path with the home directory written as `~`.
fn tilde(path: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    match path.strip_prefix(home) {
        Some("") if !home.is_empty() => "~".to_owned(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_owned(),
    }
}

/// A URL without the `user:password@` part of its authority.
fn without_credentials(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    format!("{scheme}://{host}{tail}")
}

/// The snapshot for one step: `step_model` wins over the profile's own.
pub(crate) fn pin(declared: &Declared, step_model: Option<&str>, home: &str) -> PinnedProfile {
    PinnedProfile {
        id: declared.id.clone(),
        label: declared.label.clone(),
        agent: declared.base.clone(),
        config_dir: shared_value(declared, |name| name == "CLAUDE_CONFIG_DIR")
            .map(|dir| tilde(dir, home)),
        base_url: shared_value(declared, |name| name.ends_with("_BASE_URL"))
            .map(without_credentials),
        requested_model: step_model
            .filter(|model| !model.is_empty())
            .or_else(|| shared_value(declared, |name| name == "ANTHROPIC_MODEL"))
            .map(str::to_owned),
    }
}

#[cfg(test)]
#[path = "run_pin_tests.rs"]
mod tests;
