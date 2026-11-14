use std::collections::HashSet;

use devpit_agentcli::profile::{profiles, Base};
use devpit_rpc::{Credentials, EnvVar};

use super::*;

fn codex_work(codex_home: &str) -> Declared {
    Declared {
        id: "codex-work".to_owned(),
        label: "Codex (work)".to_owned(),
        base: "codex".to_owned(),
        command: String::new(),
        args: Vec::new(),
        env: vec![EnvVar {
            name: "CODEX_HOME".to_owned(),
            value: codex_home.to_owned(),
        }],
        models: Vec::new(),
    }
}

fn resolved(declared: &[Declared]) -> Vec<Profile> {
    let base = |id: &str| {
        devpit_pty::agents::known(id).map(|agent| Base {
            program: agent.launch.to_owned(),
            driver: agent.driver.to_owned(),
        })
    };
    profiles(declared, base, &HashSet::new())
}

#[test]
fn a_second_codex_profile_is_signed_in_at_its_own_codex_home() {
    let home = tempfile::tempdir().expect("home");
    let work = tempfile::tempdir().expect("codex home");
    std::fs::create_dir_all(home.path().join(".codex")).expect("dir");
    std::fs::write(home.path().join(".codex/auth.json"), "{}").expect("default sign-in");
    let declared = [codex_work(&work.path().to_string_lossy())];
    let none = |_: &str| None;

    let before = profile_sign_ins(&declared, home.path(), &none);
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].id, "codex-work");
    assert_eq!(before[0].signed_in, Credentials::Missing);

    std::fs::write(work.path().join("auth.json"), "{}").expect("its own sign-in");
    let after = profile_sign_ins(&declared, home.path(), &none);
    assert_eq!(after[0].signed_in, Credentials::Saved);
}

#[test]
fn a_second_codex_profile_signs_in_and_starts_with_its_codex_home() {
    let all = resolved(&[codex_work("/home/me/.codex-work")]);
    assert_eq!(
        login_for(&all, "codex-work").expect("a login"),
        "CODEX_HOME='/home/me/.codex-work' codex login"
    );
    let line = devpit_agentcli::running::line(&runner(&all[0]));
    assert_eq!(line, "CODEX_HOME='/home/me/.codex-work' codex");
}

#[test]
fn a_catalogue_agent_signs_in_with_its_own_login() {
    assert_eq!(
        login_for(&[], "claude").expect("a login"),
        "claude auth login"
    );
    assert_eq!(
        login_for(&[], "opencode").expect("a login"),
        "opencode auth login"
    );
    assert!(login_for(&[], "nothing").is_err());
}

#[test]
fn a_profile_of_another_cli_has_no_provider_sign_in() {
    let home = tempfile::tempdir().expect("home");
    let mut other = codex_work("/x");
    other.base = "elsewhere".to_owned();
    assert!(profile_sign_ins(&[other], home.path(), &|_| None).is_empty());
}
