//! Whether an agent step can start under the profile it names.

use devpit_rpc::{AccountQuota, Credentials, Profile, QuotaState};

use crate::account_quota::exhausted;
use crate::run_pin::PinnedProfile;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StepCheck {
    Ready(PinnedProfile),
    Blocked(String),
}

/// The variables Claude Code signs in with besides its saved file, and
/// whether each bills somewhere other than the subscription. Its own OAuth
/// token is the subscription's, so its 5-hour quota still holds.
const CLAUDE_SIGN_INS: [(&str, bool); 7] = [
    ("ANTHROPIC_API_KEY", true),
    ("ANTHROPIC_AUTH_TOKEN", true),
    ("ANTHROPIC_BASE_URL", true),
    ("CLAUDE_CODE_USE_BEDROCK", true),
    ("CLAUDE_CODE_USE_VERTEX", true),
    ("CLAUDE_CODE_USE_FOUNDRY", true),
    ("CLAUDE_CODE_OAUTH_TOKEN", false),
];

/// How a Claude profile signs in besides `.credentials.json`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct OtherSignIn {
    /// Some other way in: a key, a token, a gateway or a key helper.
    pub(crate) any: bool,
    /// One of them bills elsewhere, so there is no subscription quota.
    pub(crate) elsewhere: bool,
}

impl OtherSignIn {
    /// What these variables, set and not empty, sign in with.
    pub(crate) fn of<'a>(vars: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        vars.into_iter()
            .filter(|(_, value)| !value.is_empty())
            .filter_map(|(name, _)| CLAUDE_SIGN_INS.iter().find(|(one, _)| *one == name))
            .fold(Self::default(), |said, (_, elsewhere)| Self {
                any: true,
                elsewhere: said.elsewhere || *elsewhere,
            })
    }

    /// Either way in.
    pub(crate) fn or(self, other: Self) -> Self {
        Self {
            any: self.any || other.any,
            elsewhere: self.elsewhere || other.elsewhere,
        }
    }

    /// What Claude Code's `settings.json` signs in with: its `env` block,
    /// and an `apiKeyHelper`, which hands it an API key.
    pub(crate) fn in_settings(text: &str) -> Self {
        let Ok(settings) = serde_json::from_str::<serde_json::Value>(text) else {
            return Self::default();
        };
        let env = settings
            .get("env")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(name, value)| Some((name.as_str(), value.as_str()?)));
        let helper = settings
            .get("apiKeyHelper")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|helper| !helper.trim().is_empty());
        Self::of(env).or(Self {
            any: helper,
            elsewhere: helper,
        })
    }
}

/// `other`: how the profile signs in besides its saved file, as the CLI's
/// settings and devpit's environment say; the profile's own variables are
/// read here.
pub(crate) fn check(
    pinned: Option<PinnedProfile>,
    profile: Option<&Profile>,
    credentials: Credentials,
    quota: Option<&AccountQuota>,
    other: OtherSignIn,
) -> StepCheck {
    let (Some(pinned), Some(profile)) = (pinned, profile) else {
        return StepCheck::Blocked("the profile no longer exists".to_owned());
    };
    if !profile.enabled {
        return StepCheck::Blocked(format!("{} is switched off", profile.label));
    }
    let other = other.or(OtherSignIn::of(
        profile
            .env
            .iter()
            .map(|var| (var.name.as_str(), var.value.as_str())),
    ));
    // Only a Claude sign-in is known well enough to stop on: another CLI
    // takes keys devpit does not list. The snapshot drops a gateway that
    // carries a credential, so `other` reads the profile's own variables.
    let claude = pinned.agent == "claude";
    let claude_account = claude && pinned.base_url.is_none() && !other.elsewhere;
    let signed_out = credentials == Credentials::Missing
        || (claude_account && quota.is_some_and(|one| one.state == QuotaState::SignedOut));
    if claude && signed_out && !other.any && pinned.base_url.is_none() {
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
