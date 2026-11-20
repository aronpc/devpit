use devpit_rpc::{AccountQuota, Credentials, OfferedModel, PlanWindow, Profile, QuotaState, Reach};

use super::*;

fn pinned(agent: &str, gateway: Option<&str>, model: Option<&str>) -> PinnedProfile {
    PinnedProfile {
        id: "work".to_owned(),
        label: "Work".to_owned(),
        agent: agent.to_owned(),
        config_dir: Some("~/.claude-work".to_owned()),
        base_url: gateway.map(str::to_owned),
        requested_model: model.map(str::to_owned),
    }
}

fn profile(enabled: bool, unavailable: Option<(&str, &str)>) -> Profile {
    Profile {
        id: "work".to_owned(),
        label: "Work".to_owned(),
        command: "claude".to_owned(),
        driver: "claude".to_owned(),
        path: Some("/usr/bin/claude".to_owned()),
        reach: Reach::Runnable,
        base: "claude".to_owned(),
        args: Vec::new(),
        env: Vec::new(),
        mine: true,
        models: vec!["sonnet".to_owned()],
        offered: unavailable
            .map(|(id, why)| OfferedModel {
                id: id.to_owned(),
                name: id.to_owned(),
                unavailable: Some(why.to_owned()),
            })
            .into_iter()
            .collect(),
        own_models: Vec::new(),
        efforts: Vec::new(),
        effort_default: None,
        metered: true,
        enabled,
    }
}

/// A quota whose 5-hour window is at `percent`, flagged the way the reader flags it.
fn quota(state: QuotaState, percent: f64) -> AccountQuota {
    let windows = vec![PlanWindow {
        label: "5-hour".to_owned(),
        percent,
        resets_at: None,
    }];
    AccountQuota {
        name: "work".to_owned(),
        config_dir: "/home/me/.claude-work".to_owned(),
        state,
        nearly_out: crate::account_quota::nearly_out(&windows),
        windows,
        read_at: 1.0,
    }
}

fn reason(result: StepCheck) -> String {
    match result {
        StepCheck::Blocked(reason) => reason,
        StepCheck::Ready(_) => panic!("the step was ready"),
    }
}

#[test]
fn a_missing_profile_blocks_the_step() {
    assert_eq!(
        reason(check(None, None, Credentials::Saved, None)),
        "the profile no longer exists"
    );
}

#[test]
fn a_switched_off_profile_blocks_the_step() {
    let profile = profile(false, None);
    assert_eq!(
        reason(check(
            Some(pinned("claude", None, None)),
            Some(&profile),
            Credentials::Saved,
            None,
        )),
        "Work is switched off"
    );
}

#[test]
fn a_profile_without_a_saved_sign_in_blocks_the_step() {
    let profile = profile(true, None);
    for (credentials, quota) in [
        (Credentials::Missing, None),
        (
            Credentials::Unknown,
            Some(quota(QuotaState::SignedOut, 0.0)),
        ),
    ] {
        assert_eq!(
            reason(check(
                Some(pinned("claude", None, None)),
                Some(&profile),
                credentials,
                quota.as_ref(),
            )),
            "Work has no saved sign-in"
        );
    }
}

#[test]
fn a_claude_account_without_quota_blocks_the_step() {
    let profile = profile(true, None);
    let exhausted = quota(QuotaState::Read, 100.0);
    assert_eq!(
        reason(check(
            Some(pinned("claude", None, None)),
            Some(&profile),
            Credentials::Saved,
            Some(&exhausted),
        )),
        "Work's 5-hour quota is exhausted"
    );
}

/// Nearly out is a hint for the next session, not a reason to stop a step.
#[test]
fn a_nearly_out_quota_does_not_block_the_step() {
    let profile = profile(true, None);
    let low = quota(QuotaState::Read, 95.0);
    assert!(low.nearly_out, "95% is past the nearly-out line");
    assert!(matches!(
        check(
            Some(pinned("claude", None, None)),
            Some(&profile),
            Credentials::Saved,
            Some(&low),
        ),
        StepCheck::Ready(_)
    ));
}

#[test]
fn a_model_the_agent_refused_blocks_the_step() {
    let profile = profile(true, Some(("sonnet", "not available on this plan")));
    assert_eq!(
        reason(check(
            Some(pinned("claude", None, Some("sonnet"))),
            Some(&profile),
            Credentials::Saved,
            None,
        )),
        "sonnet is unavailable for Work: not available on this plan"
    );
}

#[test]
fn a_ready_step_returns_the_profile_snapshot() {
    let profile = profile(true, None);
    let snapshot = pinned("claude", None, Some("sonnet"));
    assert_eq!(
        check(
            Some(snapshot.clone()),
            Some(&profile),
            Credentials::Saved,
            Some(&quota(QuotaState::Read, 0.0)),
        ),
        StepCheck::Ready(snapshot)
    );
}

#[test]
fn unknown_quota_never_blocks_a_step() {
    let profile = profile(true, None);
    for (snapshot, quota) in [
        (
            pinned("claude", None, None),
            Some(quota(QuotaState::Unavailable, 100.0)),
        ),
        (
            pinned("claude", Some("https://api.z.ai"), None),
            Some(quota(QuotaState::Read, 100.0)),
        ),
        (
            pinned("codex", None, None),
            Some(quota(QuotaState::Read, 100.0)),
        ),
    ] {
        assert!(matches!(
            check(
                Some(snapshot),
                Some(&profile),
                Credentials::Saved,
                quota.as_ref(),
            ),
            StepCheck::Ready(_)
        ));
    }
}

#[test]
fn a_lapsed_access_token_does_not_block_a_saved_login() {
    let profile = profile(true, None);
    assert!(matches!(
        check(
            Some(pinned("claude", None, None)),
            Some(&profile),
            Credentials::Saved,
            Some(&quota(QuotaState::Expired, 0.0)),
        ),
        StepCheck::Ready(_)
    ));
}
