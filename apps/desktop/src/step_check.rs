//! Whether an agent step can start under the profile it names.

use devpit_rpc::{AccountQuota, Credentials, Profile, QuotaState};

use crate::account_quota::exhausted;
use crate::run_pin::PinnedProfile;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StepCheck {
    Ready(PinnedProfile),
    Blocked(String),
}

pub(crate) fn check(
    pinned: Option<PinnedProfile>,
    profile: Option<&Profile>,
    credentials: Credentials,
    quota: Option<&AccountQuota>,
) -> StepCheck {
    let (Some(pinned), Some(profile)) = (pinned, profile) else {
        return StepCheck::Blocked("the profile no longer exists".to_owned());
    };
    if !profile.enabled {
        return StepCheck::Blocked(format!("{} is switched off", profile.label));
    }
    let claude_account = pinned.agent == "claude" && pinned.base_url.is_none();
    if credentials == Credentials::Missing
        || (claude_account && quota.is_some_and(|one| one.state == QuotaState::SignedOut))
    {
        return StepCheck::Blocked(format!("{} has no saved sign-in", profile.label));
    }
    if claude_account
        && quota.is_some_and(|one| one.state == QuotaState::Read && exhausted(&one.windows))
    {
        return StepCheck::Blocked(format!("{}'s 5-hour quota is exhausted", profile.label));
    }
    if let Some((model, why)) = pinned.requested_model.as_deref().and_then(|model| {
        profile
            .offered
            .iter()
            .find(|offered| offered.id == model)
            .and_then(|offered| offered.unavailable.as_deref())
            .map(|why| (model, why))
    }) {
        return StepCheck::Blocked(format!(
            "{model} is unavailable for {}: {why}",
            profile.label
        ));
    }
    StepCheck::Ready(pinned)
}

#[cfg(test)]
#[path = "step_check_tests.rs"]
mod tests;
