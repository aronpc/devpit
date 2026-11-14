use devpit_rpc::{Profile, Reach};

use super::profile;

fn one(id: &str, driver: &str, enabled: bool) -> Profile {
    Profile {
        id: id.to_owned(),
        label: id.to_owned(),
        command: id.to_owned(),
        driver: driver.to_owned(),
        path: None,
        reach: Reach::Runnable,
        base: driver.to_owned(),
        args: Vec::new(),
        env: Vec::new(),
        mine: true,
        models: Vec::new(),
        own_models: Vec::new(),
        efforts: Vec::new(),
        effort_default: None,
        metered: true,
        enabled,
    }
}

#[test]
fn the_default_account_runs_when_it_is_claude() {
    let all = [
        one("claude", "claude", true),
        one("claudin", "claude", true),
    ];
    assert_eq!(profile("claudin", &all).as_deref(), Some("claudin"));
}

#[test]
fn another_agent_as_default_falls_back_to_a_claude_account() {
    let all = [
        one("codex", "codex", true),
        one("off", "claude", false),
        one("claude", "claude", true),
    ];
    assert_eq!(profile("codex", &all).as_deref(), Some("claude"));
    assert_eq!(profile("", &[one("codex", "codex", true)]), None);
}
