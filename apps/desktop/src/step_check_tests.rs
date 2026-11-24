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
        reason(check(
            None,
            None,
            Credentials::Saved,
            None,
            OtherSignIn::default()
        )),
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
            OtherSignIn::default(),
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
                OtherSignIn::default(),
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
            OtherSignIn::default(),
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
            OtherSignIn::default(),
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
            OtherSignIn::default(),
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
            OtherSignIn::default(),
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
                OtherSignIn::default(),
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
            OtherSignIn::default(),
        ),
        StepCheck::Ready(_)
    ));
}

/// A Claude profile that signs in with a key, a token or a gateway, in its
/// own variables or as devpit's environment hands it on, has no file to miss:
/// before the check, such steps ran, and they still do.
#[test]
fn a_profile_that_signs_in_with_a_key_or_a_gateway_is_not_signed_out() {
    let signed_out = quota(QuotaState::SignedOut, 0.0);
    for name in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "CLAUDE_CODE_USE_BEDROCK",
    ] {
        let mut keyed = profile(true, None);
        keyed.env.push(devpit_rpc::EnvVar {
            name: name.to_owned(),
            value: "set".to_owned(),
        });
        for (credentials, quota) in [
            (Credentials::Missing, None),
            (Credentials::Unknown, Some(&signed_out)),
        ] {
            assert!(
                matches!(
                    check(
                        Some(pinned("claude", None, None)),
                        Some(&keyed),
                        credentials,
                        quota,
                        OtherSignIn::default(),
                    ),
                    StepCheck::Ready(_)
                ),
                "{name} was taken for a missing sign-in"
            );
        }
    }

    let plain = profile(true, None);
    let inherited = OtherSignIn::of([("ANTHROPIC_AUTH_TOKEN", "set")]);
    assert!(matches!(
        check(
            Some(pinned("claude", None, None)),
            Some(&plain),
            Credentials::Missing,
            None,
            inherited,
        ),
        StepCheck::Ready(_)
    ));
    let mut empty = profile(true, None);
    empty.env.push(devpit_rpc::EnvVar {
        name: "ANTHROPIC_API_KEY".to_owned(),
        value: String::new(),
    });
    assert_eq!(
        reason(check(
            Some(pinned("claude", None, None)),
            Some(&empty),
            Credentials::Missing,
            None,
            OtherSignIn::default(),
        )),
        "Work has no saved sign-in",
        "an empty key signs nothing in"
    );
}

/// A variable another program reads signs Claude in with nothing: only
/// Claude Code's own count.
#[test]
fn only_claudes_own_variables_count_as_a_way_in() {
    let other = OtherSignIn::of([
        ("OPENAI_API_KEY", "sk"),
        ("SENTRY_AUTH_TOKEN", "t"),
        ("NPM_CONFIG_BASE_URL", "u"),
    ]);
    assert_eq!(other, OtherSignIn::default());
}

/// Claude Code also takes a key or a gateway from its `settings.json`: the
/// `env` block, as z.ai documents, or an `apiKeyHelper`.
#[test]
fn a_key_or_a_gateway_in_claudes_settings_is_a_way_in() {
    let gateway = OtherSignIn::in_settings(
        r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.z.ai/api/anthropic","ANTHROPIC_AUTH_TOKEN":"k"}}"#,
    );
    assert!(gateway.any && gateway.elsewhere);
    let helper = OtherSignIn::in_settings(r#"{"apiKeyHelper":"~/bin/key.sh"}"#);
    assert!(helper.any && helper.elsewhere);
    for nothing in [
        r#"{"env":{"ANTHROPIC_AUTH_TOKEN":""},"apiKeyHelper":" "}"#,
        r#"{"env":{"DISABLE_TELEMETRY":"1"}}"#,
        "not json",
        "",
    ] {
        assert_eq!(
            OtherSignIn::in_settings(nothing),
            OtherSignIn::default(),
            "{nothing}"
        );
    }

    let plain = profile(true, None);
    assert!(matches!(
        check(
            Some(pinned("claude", None, None)),
            Some(&plain),
            Credentials::Missing,
            None,
            gateway,
        ),
        StepCheck::Ready(_)
    ));
}

/// The subscription's own OAuth token signs in without the file, and spends
/// the subscription's quota all the same.
#[test]
fn a_subscription_token_still_stops_on_an_exhausted_quota() {
    let mut token = profile(true, None);
    token.env.push(devpit_rpc::EnvVar {
        name: "CLAUDE_CODE_OAUTH_TOKEN".to_owned(),
        value: "set".to_owned(),
    });
    assert_eq!(
        reason(check(
            Some(pinned("claude", None, None)),
            Some(&token),
            Credentials::Missing,
            Some(&quota(QuotaState::Read, 100.0)),
            OtherSignIn::default(),
        )),
        "Work's 5-hour quota is exhausted"
    );
}

/// Another CLI takes keys devpit does not list, so a missing file of its own
/// is not known to mean signed out.
#[test]
fn another_clis_missing_file_does_not_stop_its_step() {
    let profile = profile(true, None);
    assert!(matches!(
        check(
            Some(pinned("codex", None, None)),
            Some(&profile),
            Credentials::Missing,
            None,
            OtherSignIn::default(),
        ),
        StepCheck::Ready(_)
    ));
}
