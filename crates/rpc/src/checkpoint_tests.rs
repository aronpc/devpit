//! The three states, each asked directly.

use super::*;

fn read(passed: u32, failed: u32) -> Option<Report> {
    Some(Report { passed, failed })
}

/// The whole point of keeping the process and the result apart.
///
/// Sabotage: make `verdict` answer `Passed` for `RunState::Ok` with no report
/// and this fails — which is what "exit code zero means the tests passed"
/// looks like from the inside.
#[test]
fn exit_zero_with_nothing_to_read_is_not_passing() {
    assert_eq!(verdict(RunState::Ok, None), Verdict::Inconclusive);
}

#[test]
fn a_report_that_passed_is_the_only_way_to_pass() {
    assert_eq!(verdict(RunState::Ok, read(29, 0)), Verdict::Passed);
}

/// A runner that matched no test files exits zero and checked nothing.
/// Passing zero checks is not passing.
#[test]
fn a_report_with_no_checks_in_it_is_not_passing() {
    assert_eq!(verdict(RunState::Ok, read(0, 0)), Verdict::Inconclusive);
}

/// A suite can fail a test and still exit zero — a reporter that swallows the
/// code, a wrapper that does not forward it. The report wins.
#[test]
fn a_failed_check_is_failed_whatever_the_process_said() {
    assert_eq!(verdict(RunState::Ok, read(28, 1)), Verdict::Failed);
    assert_eq!(verdict(RunState::Failed, read(28, 1)), Verdict::Failed);
}

/// The command failed and nothing says which check did: a build that did not
/// compile, a runner that is not installed. Something went wrong and this does
/// not claim to know what.
#[test]
fn a_failed_command_with_no_report_does_not_claim_to_know_what_failed() {
    assert_eq!(verdict(RunState::Failed, None), Verdict::Inconclusive);
}

/// Sabotage: make `Lost` answer `Passed` and this fails. A run whose process
/// vanished is never an approval, however green its last line looked.
#[test]
fn a_lost_run_is_never_an_approval() {
    assert_eq!(verdict(RunState::Lost, None), Verdict::Inconclusive);
    assert_eq!(
        verdict(RunState::Lost, read(29, 0)),
        Verdict::Inconclusive,
        "a report from a run nobody watched end was taken as proof"
    );
}

/// Stopping work is not the work failing, and it is not the work passing.
#[test]
fn stopping_and_still_going_are_both_not_run() {
    assert_eq!(verdict(RunState::Cancelled, None), Verdict::NotRun);
    assert_eq!(
        verdict(RunState::Cancelled, read(29, 0)),
        Verdict::NotRun,
        "a cancelled run was approved by what it managed to print first"
    );
    assert_eq!(verdict(RunState::Running, None), Verdict::NotRun);
}

/// Sabotage: make `Paused` answer `Inconclusive` and this fails. A pause is a
/// question, not a result: nothing ran while it waited.
#[test]
fn a_run_waiting_out_a_pause_is_not_run() {
    assert_eq!(verdict(RunState::Paused, None), Verdict::NotRun);
    assert_eq!(
        verdict(RunState::Paused, read(29, 0)),
        Verdict::NotRun,
        "a report that reached a pause was taken as a result"
    );
}

fn at(revision: &str, changes: &str) -> Fingerprint {
    Fingerprint {
        revision: Some(revision.to_owned()),
        changes: Some(changes.to_owned()),
    }
}

#[test]
fn the_same_code_is_current() {
    assert_eq!(
        validity(&at("2f0bee9", "clean"), &at("2f0bee9", "clean")),
        Validity::Current
    );
}

#[test]
fn a_commit_since_makes_it_stale() {
    assert_eq!(
        validity(&at("2f0bee9", "clean"), &at("ed16baf", "clean")),
        Validity::Stale
    );
}

/// Uncommitted work is most of what a person is looking at, so the revision
/// alone cannot answer this.
///
/// Sabotage: compare only the revision and this reads a result from before an
/// edit as current.
#[test]
fn an_edit_since_makes_it_stale_even_on_the_same_commit() {
    assert_eq!(
        validity(&at("2f0bee9", "clean"), &at("2f0bee9", "one file changed")),
        Validity::Stale
    );
}

/// Every run from before migration 017 recorded nothing, and nothing is not
/// an answer in either direction.
#[test]
fn a_run_that_recorded_nothing_is_unknown_and_not_current() {
    let nothing = Fingerprint {
        revision: None,
        changes: None,
    };
    assert_eq!(
        validity(&nothing, &at("2f0bee9", "clean")),
        Validity::Unknown
    );
    assert_eq!(
        validity(&at("2f0bee9", "clean"), &nothing),
        Validity::Unknown
    );
    assert_eq!(validity(&nothing, &nothing), Validity::Unknown);
}

/// Half an answer is not an answer: a matching revision with nobody having
/// looked at the working tree is `Unknown`, not `Current`.
#[test]
fn a_matching_revision_with_no_look_at_the_tree_is_unknown() {
    let only_revision = Fingerprint {
        revision: Some("2f0bee9".to_owned()),
        changes: None,
    };
    assert_eq!(
        validity(&only_revision, &at("2f0bee9", "clean")),
        Validity::Unknown
    );
    assert_eq!(
        validity(&at("2f0bee9", "clean"), &only_revision),
        Validity::Unknown
    );
}
