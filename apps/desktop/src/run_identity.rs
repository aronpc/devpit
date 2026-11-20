//! The safe account, model and attempt words a run exposes to the screen.

use devpit_rpc::RunAttempt;

pub(crate) fn read(
    pinned: Option<&str>,
    requested: Option<String>,
    effective: Option<String>,
    attempt: Option<i64>,
) -> (Option<String>, Option<String>, RunAttempt) {
    let profile = pinned
        .and_then(|json| serde_json::from_str::<crate::run_pin::PinnedProfile>(json).ok())
        .map(|profile| profile.label);
    let model_is_requested = effective.is_none() && requested.is_some();
    (
        profile,
        effective.or(requested),
        RunAttempt {
            number: attempt.and_then(|number| u32::try_from(number).ok()),
            model_is_requested,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_identity_distinguishes_requested_and_effective_models() {
        let (_, requested, first) = read(None, Some("sonnet".to_owned()), None, Some(2));
        assert_eq!(requested.as_deref(), Some("sonnet"));
        assert_eq!(first.number, Some(2));
        assert!(first.model_is_requested);

        let (_, effective, finished) = read(
            None,
            Some("sonnet".to_owned()),
            Some("claude-sonnet-4-6".to_owned()),
            Some(2),
        );
        assert_eq!(effective.as_deref(), Some("claude-sonnet-4-6"));
        assert!(!finished.model_is_requested);
    }
}
