use devpit_rpc::{Accounts, ClaudeAccount, ProjectAccount};

use super::*;

fn account(name: &str, dir: &str) -> ClaudeAccount {
    ClaudeAccount {
        name: name.to_owned(),
        config_dir: dir.to_owned(),
    }
}

fn default_of(project: &str, account: &str) -> ProjectAccount {
    ProjectAccount {
        project_id: project.to_owned(),
        account: account.to_owned(),
    }
}

#[test]
fn a_kept_list_reads_home_and_drops_defaults_that_name_nothing() {
    let kept = checked(
        Accounts {
            accounts: vec![
                account(" work ", "~/.cfg-work"),
                account("home", "/srv/cfg"),
            ],
            projects: vec![
                default_of("prj_a", "work"),
                default_of("prj_a", "home"),
                default_of("prj_b", "gone"),
            ],
        },
        "/home/me",
    )
    .expect("kept");
    assert_eq!(kept.accounts[0], account("work", "/home/me/.cfg-work"));
    assert_eq!(kept.projects, [default_of("prj_a", "work")]);
}

#[test]
fn a_list_that_cannot_start_a_session_is_refused() {
    let refused = |accounts: Vec<ClaudeAccount>| {
        checked(
            Accounts {
                accounts,
                projects: Vec::new(),
            },
            "/home/me",
        )
        .is_err()
    };
    assert!(refused(vec![account("", "/a")]));
    assert!(refused(vec![account("a", "relative/dir")]));
    assert!(refused(vec![account("a", "/a"), account("a", "/b")]));
    assert!(refused(vec![account("a", "/a\u{3}b")]));
    assert!(!refused(vec![account("a", "/a"), account("b", "/a b")]));
}

#[test]
fn a_project_opens_in_its_own_account() {
    let accounts = Accounts {
        accounts: vec![account("work", "/w"), account("home", "/h")],
        projects: vec![default_of("prj_a", "home")],
    };
    assert_eq!(of_project(&accounts, "prj_a"), Some(&account("home", "/h")));
    assert_eq!(of_project(&accounts, "prj_b"), None);
}

#[test]
fn the_launch_line_carries_the_directory_quoted_and_a_profile_of_its_own_still_wins() {
    assert_eq!(
        in_dir("claude --settings x", "/home/me/it's here"),
        "CLAUDE_CONFIG_DIR='/home/me/it'\\''s here' claude --settings x"
    );
    // Nothing else rides along: no endpoint, which would cost Remote Control.
    assert!(!in_dir("claude", "/w").contains("ANTHROPIC_BASE_URL"));
    let profile = "CLAUDE_CONFIG_DIR='/own' claude";
    assert!(in_dir(profile, "/w").ends_with(profile));
}

#[test]
fn the_registry_survives_a_round_trip_and_a_missing_file_is_empty() {
    let root = tempfile::tempdir().expect("tempdir");
    assert_eq!(read_at(root.path()), Accounts::default());
    let accounts = Accounts {
        accounts: vec![account("work", "/w")],
        projects: vec![default_of("prj_a", "work")],
    };
    write_at(root.path(), &accounts).expect("written");
    assert_eq!(read_at(root.path()), accounts);
}
