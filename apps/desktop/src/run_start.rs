//! Settling an agent run before its process starts.

use devpit_agentcli::running::Runner;
use devpit_core::store::{AttemptCarry, RunAsk, SwitchKind};
use devpit_core::Store;
use devpit_rpc::{Credentials, Profile, RpcError, Step};

use crate::step_check::OtherSignIn;

pub(crate) struct FixedAgent {
    pub(crate) profile: String,
    pub(crate) runner: Runner,
    pub(crate) model: Option<String>,
}

pub(crate) struct BegunAgent {
    pub(crate) run_id: String,
    pub(crate) fixed: Option<FixedAgent>,
    pub(crate) paused: bool,
}

pub(crate) struct AgentPreview {
    pub(crate) profile: String,
    pub(crate) model: Option<String>,
    pub(crate) check: String,
}

pub(crate) struct Sources {
    pub(crate) home: String,
    pub(crate) declared: Vec<devpit_rpc::Declared>,
    pub(crate) profiles: Vec<Profile>,
    pub(crate) credentials: Vec<devpit_rpc::ProfileSignIn>,
    pub(crate) waiting: std::collections::HashMap<String, String>,
    /// What devpit's own environment, which the CLI inherits, signs in with.
    pub(crate) inherited: OtherSignIn,
}

impl Sources {
    pub(crate) fn read(store: &Store) -> Result<Self, RpcError> {
        let home = crate::installations::home()?;
        let declared = crate::agent_profiles::declared(store);
        let profiles = crate::agent_profiles::all(store)?;
        let inherited = |name: &str| std::env::var(name).ok();
        let credentials = crate::provider_scan::profile_sign_ins(&declared, &home, &inherited);
        let waiting = profiles
            .iter()
            .filter_map(|profile| {
                crate::profile_waiting::blocked(store, &profile.id)
                    .map(|reason| (profile.id.clone(), reason))
            })
            .collect();
        let vars: Vec<(String, String)> = std::env::vars().collect();
        let inherited = OtherSignIn::of(
            vars.iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        );
        Ok(Self {
            home: home.display().to_string(),
            declared,
            profiles,
            credentials,
            waiting,
            inherited,
        })
    }

    pub(crate) fn current(&self, profile_id: &str, model: Option<&str>) -> AttemptCarry {
        let pinned = self
            .declared
            .iter()
            .find(|profile| profile.id == profile_id)
            .map(|profile| crate::run_pin::pin(profile, model, &self.home));
        AttemptCarry {
            requested_model: pinned
                .as_ref()
                .and_then(|profile| profile.requested_model.clone()),
            pinned: pinned.and_then(|profile| serde_json::to_string(&profile).ok()),
        }
    }

    pub(crate) fn preview(&self, profile_id: &str, model: Option<&str>) -> AgentPreview {
        let carry = self.current(profile_id, model);
        let pinned = carry
            .pinned
            .as_deref()
            .and_then(|json| serde_json::from_str::<crate::run_pin::PinnedProfile>(json).ok());
        let profile = pinned
            .as_ref()
            .map_or_else(|| profile_id.to_owned(), |profile| profile.label.clone());
        let check = self
            .fixed(&carry)
            .map(|_| "Ready".to_owned())
            .unwrap_or_else(|reason| reason);
        AgentPreview {
            profile,
            model: carry.requested_model,
            check,
        }
    }

    pub(crate) fn fixed(&self, carry: &AttemptCarry) -> Result<FixedAgent, String> {
        let pinned = carry
            .pinned
            .as_deref()
            .and_then(|json| serde_json::from_str::<crate::run_pin::PinnedProfile>(json).ok());
        let profile = pinned
            .as_ref()
            .and_then(|pinned| self.profiles.iter().find(|profile| profile.id == pinned.id));
        let credentials = pinned
            .as_ref()
            .and_then(|pinned| {
                self.credentials
                    .iter()
                    .find(|credentials| credentials.id == pinned.id)
            })
            // A CLI whose sign-in devpit cannot read is not known to be out.
            .map_or(Credentials::Unknown, |credentials| credentials.signed_in);
        let quota = profile.and_then(|profile| quota_for(profile, &self.home));
        let other = match (&pinned, profile) {
            (Some(pinned), Some(profile)) if pinned.agent == "claude" => {
                settings_sign_in(profile, &self.home).or(self.inherited)
            }
            _ => self.inherited,
        };
        match crate::step_check::check(pinned, profile, credentials, quota.as_ref(), other) {
            crate::step_check::StepCheck::Blocked(reason) => Err(reason),
            crate::step_check::StepCheck::Ready(pinned) => {
                let profile = profile.expect("a ready check has a profile");
                if let Some(why) = self.waiting.get(&pinned.id) {
                    return Err(why.clone());
                }
                if !profile.spawnable() {
                    return Err(format!(
                        "{} cannot be started outside a terminal",
                        profile.label
                    ));
                }
                Ok(FixedAgent {
                    profile: pinned.id,
                    runner: devpit_agentcli::running::runner(profile),
                    model: carry.requested_model.clone(),
                })
            }
        }
    }
}

pub(crate) fn switch_agent(
    store: &Store,
    from: &str,
    from_column: Option<&str>,
    carry: AttemptCarry,
    fixed: FixedAgent,
    how: SwitchKind,
) -> Result<BegunAgent, devpit_core::store::StoreError> {
    let (run_id, _, _) = store.begin_checked_attempt(
        RunAsk::Switch {
            from: from.to_owned(),
            how,
        },
        from_column,
        carry,
        |_| None,
    )?;
    Ok(BegunAgent {
        run_id,
        fixed: Some(fixed),
        paused: false,
    })
}

/// The Claude folder `profile` runs against.
fn config_dir(profile: &Profile, home: &str) -> std::path::PathBuf {
    let env: Vec<(String, String)> = profile
        .env
        .iter()
        .map(|one| (one.name.clone(), one.value.clone()))
        .collect();
    let inherited = std::env::var("CLAUDE_CONFIG_DIR").ok();
    devpit_agentcli::cli_config::config_dir_of(
        std::path::Path::new(home),
        &env,
        inherited.as_deref(),
    )
}

/// The most of a `settings.json` read.
const SETTINGS_CEILING: u64 = 1024 * 1024;

/// What the Claude settings in `profile`'s folder sign in with.
fn settings_sign_in(profile: &Profile, home: &str) -> OtherSignIn {
    let Ok(file) = std::fs::File::open(config_dir(profile, home).join("settings.json")) else {
        return OtherSignIn::default();
    };
    let mut text = String::new();
    match std::io::Read::read_to_string(&mut std::io::Read::take(file, SETTINGS_CEILING), &mut text)
    {
        Ok(_) => OtherSignIn::in_settings(&text),
        Err(_) => OtherSignIn::default(),
    }
}

fn quota_for(profile: &Profile, home: &str) -> Option<devpit_rpc::AccountQuota> {
    if profile.base != crate::claude_accounts::DRIVER {
        return None;
    }
    crate::account_quota::remembered(&config_dir(profile, home).display().to_string())
}

pub(crate) fn prepare(
    store: &Store,
    card_id: &str,
    step: &Step,
    from_column: Option<&str>,
) -> Result<BegunAgent, RpcError> {
    if step.kind != devpit_rpc::StepKind::Agent {
        return Ok(BegunAgent {
            run_id: store.start_run(card_id, &step.id, from_column)?,
            fixed: None,
            paused: false,
        });
    }
    let config = crate::steps::agent_config::readable(&step.config)
        .map_err(|why| RpcError::internal(why))?;
    let Some(profile_id) = config.profile.as_deref() else {
        return Ok(BegunAgent {
            run_id: store.start_run(card_id, &step.id, from_column)?,
            fixed: None,
            paused: false,
        });
    };
    let sources = Sources::read(store)?;
    begin_profiled(
        store,
        card_id,
        &step.id,
        from_column,
        profile_id,
        config.model.as_deref(),
        &sources,
    )
    .map_err(Into::into)
}

fn begin_profiled(
    store: &Store,
    card_id: &str,
    step_id: &str,
    from_column: Option<&str>,
    profile_id: &str,
    model: Option<&str>,
    sources: &Sources,
) -> Result<BegunAgent, devpit_core::store::StoreError> {
    let carry = sources.current(profile_id, model);
    let begun = begin_agent(store, card_id, step_id, from_column, carry, |carry| {
        sources.fixed(carry)
    })?;
    if !begun.paused || !crate::project_settings::automatic_switch(store, card_id)? {
        return Ok(begun);
    }
    for profile in &sources.profiles {
        let carry = sources.current(&profile.id, model);
        if let Ok(fixed) = sources.fixed(&carry) {
            return switch_agent(
                store,
                &begun.run_id,
                from_column,
                carry,
                fixed,
                SwitchKind::Automatic,
            );
        }
    }
    Ok(begun)
}

pub(crate) fn begin_agent(
    store: &Store,
    card_id: &str,
    step_id: &str,
    from_column: Option<&str>,
    carry: AttemptCarry,
    fixed: impl FnOnce(&AttemptCarry) -> Result<FixedAgent, String>,
) -> Result<BegunAgent, devpit_core::store::StoreError> {
    let mut fixed_agent = None;
    let (run_id, _, _) = store.begin_checked_attempt(
        RunAsk::Step {
            card: card_id.to_owned(),
            step: step_id.to_owned(),
        },
        from_column,
        carry,
        |carried| match fixed(carried) {
            Ok(ready) => {
                fixed_agent = Some(ready);
                None
            }
            Err(reason) => Some(reason),
        },
    )?;
    Ok(BegunAgent {
        run_id,
        paused: fixed_agent.is_none(),
        fixed: fixed_agent,
    })
}

pub(crate) fn record_effective_model(
    store: &Store,
    run_id: &str,
    model: Option<&str>,
) -> Result<(), devpit_core::store::StoreError> {
    if let Some(model) = model.filter(|model| !model.is_empty()) {
        store.set_run_effective_model(run_id, model)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "run_start_tests.rs"]
mod tests;
