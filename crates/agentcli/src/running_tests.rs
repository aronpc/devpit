use devpit_rpc::{EnvVar, Profile, Reach};

use super::*;

fn profile(args: &[&str], env: &[(&str, &str)]) -> Profile {
    Profile {
        id: "01JGLM".to_owned(),
        label: "GLM".to_owned(),
        command: "claude".to_owned(),
        driver: "claude".to_owned(),
        path: Some("/usr/bin/claude".to_owned()),
        reach: Reach::Runnable,
        base: "claude".to_owned(),
        args: args.iter().map(|one| (*one).to_owned()).collect(),
        env: env
            .iter()
            .map(|(name, value)| EnvVar {
                name: (*name).to_owned(),
                value: (*value).to_owned(),
            })
            .collect(),
        mine: true,
        models: Vec::new(),
        offered: Vec::new(),
        own_models: Vec::new(),
        efforts: Vec::new(),
        effort_default: None,
        metered: true,
        enabled: true,
    }
}

#[test]
fn the_profile_measured_in_a_real_zshrc_becomes_the_line_it_was() {
    // `glm` as it was written by hand, rebuilt from data. If these two do not
    // match, the feature has not replaced anything.
    let glm = profile(
        &["--permission-mode", "bypassPermissions"],
        &[
            ("ANTHROPIC_BASE_URL", "https://api.z.ai/api/anthropic"),
            ("CLAUDE_CONFIG_DIR", "/home/someone/.claude-glm"),
        ],
    );
    assert_eq!(
        line(&runner(&glm)),
        "ANTHROPIC_BASE_URL='https://api.z.ai/api/anthropic' \
         CLAUDE_CONFIG_DIR='/home/someone/.claude-glm' \
         claude --permission-mode bypassPermissions"
    );
}

#[test]
fn a_profile_with_nothing_added_is_just_its_program() {
    assert_eq!(line(&runner(&profile(&[], &[]))), "claude");
}

#[test]
fn a_value_with_a_space_stays_one_word() {
    let odd = profile(&[], &[("HOME_ISH", "/home/some one")]);
    assert_eq!(line(&runner(&odd)), "HOME_ISH='/home/some one' claude");
}

#[test]
fn a_value_carrying_shell_punctuation_is_still_a_value() {
    let hostile = profile(&[], &[("TOKEN", "a;rm -rf /")]);
    assert_eq!(line(&runner(&hostile)), "TOKEN='a;rm -rf /' claude");
}

#[test]
fn a_line_means_to_a_shell_exactly_what_it_says() {
    // The escape is only as good as what a shell does with it, so a shell is
    // what checks it. `env` prints what it was actually given.
    let sneaky = profile(
        &[],
        &[("TOKEN", "'; touch /tmp/devpit-should-not-exist; '")],
    );
    let script = format!("{} sh -c 'printf %s \"$TOKEN\"'", line(&runner(&sneaky)));
    let out = std::process::Command::new("sh")
        .args(["-c", &script.replace("claude", "env")])
        .output()
        .expect("sh");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "'; touch /tmp/devpit-should-not-exist; '"
    );
    assert!(
        !std::path::Path::new("/tmp/devpit-should-not-exist").exists(),
        "the value escaped its quotes and ran"
    );
}

#[test]
fn the_profiles_arguments_come_before_the_callers() {
    // `--permission-mode` is how the person wants the agent to behave; the
    // turn's own flags are about this one turn. The profile sets the scene.
    let glm = profile(&["--permission-mode", "bypassPermissions"], &[]);
    assert_eq!(
        runner(&glm).argv(&["-p".to_owned(), "--output-format".to_owned()]),
        [
            "--permission-mode",
            "bypassPermissions",
            "-p",
            "--output-format"
        ]
    );
}

#[test]
fn a_runner_hands_the_environment_over_as_pairs() {
    // Not as text. This is what makes the spawned path unable to misparse.
    let one = profile(&[], &[("A", "1"), ("B", "2")]);
    assert_eq!(
        runner(&one).env,
        [
            ("A".to_owned(), "1".to_owned()),
            ("B".to_owned(), "2".to_owned())
        ]
    );
}

/// The session's language is devpit's to set at the session's birth: a
/// profile's own `DEVPIT_LOCALE` never reaches the process.
#[test]
fn a_profile_cannot_set_the_sessions_language() {
    let speaking = profile(&[], &[("DEVPIT_LOCALE", "en"), ("TOKEN", "x")]);
    assert_eq!(
        runner(&speaking).env,
        [("TOKEN".to_owned(), "x".to_owned())]
    );
}
