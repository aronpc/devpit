//! Settings → Providers: per profile, which of its models are switched off
//! and the advisor model its sessions are started with, kept beside the
//! profiles so the declared profile and what syncs stay as they were; and
//! how often sign-ins and MCP servers are checked.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use devpit_core::{preference, Store};
use devpit_rpc::{ErrorCode, RpcError};
use serde::{Deserialize, Serialize};
use specta::Type;

/// One profile's models, as Settings shows them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfileModels {
    /// Left out of the model picker.
    pub models_off: Vec<String>,
    /// Written as `advisorModel` into the settings devpit starts it with.
    pub advisor_model: Option<String>,
    /// What a chat on this profile runs when none is picked.
    pub model: Option<String>,
    /// The effort its chats start with, over the driver's own.
    pub effort: Option<String>,
}

fn kept(store: &Store) -> BTreeMap<String, ProfileModels> {
    store
        .preference(preference::AGENTS_PROFILE_MODELS)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// A profile's models; every one on and no advisor when nothing was set.
pub(crate) fn of(store: &Store, profile_id: &str) -> ProfileModels {
    kept(store).remove(profile_id).unwrap_or_default()
}

/// The profile's own default model, when it set one.
pub(crate) fn default_model(store: &Store, profile_id: &str) -> Option<String> {
    of(store, profile_id).model
}

/// The model a session runs without one chosen, which stays on.
pub(crate) const DEFAULT_MODEL: &str = "default";

/// What `write` accepts: model names only, as a profile's own models are;
/// an empty advisor model is none.
pub(crate) fn checked(models: ProfileModels) -> Result<ProfileModels, String> {
    let set = |one: Option<String>| {
        one.map(|one| one.trim().to_owned())
            .filter(|one| !one.is_empty())
    };
    let models = ProfileModels {
        advisor_model: set(models.advisor_model),
        model: set(models.model),
        effort: set(models.effort),
        models_off: models.models_off,
    };
    if let Some(effort) = models
        .effort
        .as_deref()
        .filter(|one| !devpit_agentcli::declaring::is_one_word(one))
    {
        return Err(format!("{effort:?} is not an effort"));
    }
    if models.models_off.iter().any(|one| one == DEFAULT_MODEL) {
        return Err("the default model stays on: it is what a session runs without one".to_owned());
    }
    match models
        .models_off
        .iter()
        .chain(models.advisor_model.iter())
        .chain(models.model.iter())
        .find(|one| !devpit_agentcli::declaring::is_a_model(one))
    {
        Some(bad) => Err(format!("{bad:?} is not a model name")),
        None => Ok(models),
    }
}

/// The hook settings file: `hooks.json`, or one per profile that needs its own.
pub(crate) fn hooks_file(root: &Path, profile_id: Option<&str>) -> PathBuf {
    match profile_id {
        Some(id) => {
            let safe: String = id
                .chars()
                .map(|one| {
                    if one.is_ascii_alphanumeric() || one == '-' || one == '_' {
                        one
                    } else {
                        '_'
                    }
                })
                .collect();
            root.join(format!("hooks-{safe}.json"))
        }
        None => root.join("hooks.json"),
    }
}

/// `profile.models_read`
#[tauri::command]
#[specta::specta]
pub async fn profile_models_read(profile_id: String) -> Result<ProfileModels, RpcError> {
    crate::off_main::blocking(move || Ok(of(&Store::open_default()?, &profile_id))).await
}

/// `profile.models_write` — the whole of one profile's models, replaced.
#[tauri::command]
#[specta::specta]
pub async fn profile_models_write(
    profile_id: String,
    models: ProfileModels,
) -> Result<ProfileModels, RpcError> {
    crate::off_main::blocking(move || {
        let models = checked(models).map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
        let store = Store::open_default()?;
        let mut all = kept(&store);
        if models == ProfileModels::default() {
            all.remove(&profile_id);
        } else {
            all.insert(profile_id.clone(), models);
        }
        let json =
            serde_json::to_string(&all).map_err(|err| RpcError::internal(err.to_string()))?;
        store.set_preference(preference::AGENTS_PROFILE_MODELS, &json)?;
        Ok(of(&store, &profile_id))
    })
    .await
}

/// How often sign-ins and MCP servers are checked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderChecks {
    pub minutes: u32,
}

/// The minutes Settings offers between checks.
pub(crate) const CHECK_MINUTES: [u32; 4] = [1, 5, 15, 30];
/// What the checks always waited.
pub(crate) const CHECK_DEFAULT: u32 = 5;

pub(crate) fn check_minutes(store: &Store) -> u32 {
    store
        .preference(preference::PROVIDERS_CHECK_MINUTES)
        .ok()
        .flatten()
        .and_then(|minutes| minutes.parse().ok())
        .filter(|minutes| CHECK_MINUTES.contains(minutes))
        .unwrap_or(CHECK_DEFAULT)
}

/// `providers.check_read`
#[tauri::command]
#[specta::specta]
pub async fn providers_check_read() -> Result<ProviderChecks, RpcError> {
    crate::off_main::blocking(|| {
        Ok(ProviderChecks {
            minutes: check_minutes(&Store::open_default()?),
        })
    })
    .await
}

/// `providers.check_write`
#[tauri::command]
#[specta::specta]
pub async fn providers_check_write(minutes: u32) -> Result<ProviderChecks, RpcError> {
    crate::off_main::blocking(move || {
        if !CHECK_MINUTES.contains(&minutes) {
            return Err(RpcError::new(
                ErrorCode::Invalid,
                format!("check every {CHECK_MINUTES:?} minutes"),
            ));
        }
        let store = Store::open_default()?;
        store.set_preference(preference::PROVIDERS_CHECK_MINUTES, &minutes.to_string())?;
        Ok(ProviderChecks {
            minutes: check_minutes(&store),
        })
    })
    .await
}

#[cfg(test)]
#[path = "profile_models_tests.rs"]
mod tests;
