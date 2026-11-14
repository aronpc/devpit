use devpit_rpc::{EnvVar, Reach};

use super::*;

fn open(dir: &std::path::Path) -> Store {
    Store::open(&dir.join("state.db")).expect("open")
}

fn glm() -> Declared {
    // The profile this feature was measured against: seven variables, the base
    // agent's own program, one flag.
    Declared {
        id: String::new(),
        label: "GLM".to_owned(),
        base: "claude".to_owned(),
        command: String::new(),
        args: vec![
            "--permission-mode".to_owned(),
            "bypassPermissions".to_owned(),
        ],
        models: Vec::new(),
        env: vec![
            EnvVar {
                name: "ANTHROPIC_BASE_URL".to_owned(),
                value: "https://api.z.ai/api/anthropic".to_owned(),
            },
            EnvVar {
                name: "CLAUDE_CONFIG_DIR".to_owned(),
                value: "/home/someone/.claude-glm".to_owned(),
            },
        ],
    }
}

fn written(store: &Store, mut one: Declared) -> Declared {
    if one.id.is_empty() {
        one.id = ulid::Ulid::generate().to_string();
    }
    let mut list = stored(store).expect("read");
    list.push(one.clone());
    save(store, &list).expect("write");
    one
}

#[test]
fn a_saved_profile_survives_closing_the_database() {
    // It decides which account somebody's board spends money through, so it
    // had better still be there when they come back.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let mine = written(&store, glm());
    drop(store);

    let store = open(dir.path());
    let back = stored(&store).expect("read");
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].id, mine.id);
    assert_eq!(back[0].label, "GLM");
    assert_eq!(back[0].env.len(), 2);
    assert_eq!(back[0].args, ["--permission-mode", "bypassPermissions"]);
}

#[test]
fn a_fresh_database_declares_nothing() {
    // Built-ins stay derived, not stored: nobody's first run should come with
    // rows in it.
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(stored(&open(dir.path())).expect("read"), Vec::new());
}

#[test]
fn a_profile_with_no_command_runs_its_bases_program() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let mine = written(&store, glm());

    let resolved = profiles(
        &stored(&store).expect("read"),
        base_of,
        &std::collections::HashSet::new(),
    );
    let listed = resolved
        .iter()
        .find(|one| one.id == mine.id)
        .expect("there");
    assert_eq!(listed.command, "claude", "it should run Claude Code's own");
    assert_eq!(listed.driver, "claude", "and read it with Claude's driver");
    assert!(listed.mine, "the person declared it");
}

#[test]
fn a_profile_naming_its_own_program_keeps_it() {
    // `clawz` pointed at a debug build in a source tree.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let mut local = glm();
    local.command = "/somewhere/target/debug/claw".to_owned();
    let mine = written(&store, local);

    let resolved = profiles(
        &stored(&store).expect("read"),
        base_of,
        &std::collections::HashSet::new(),
    );
    let listed = resolved
        .iter()
        .find(|one| one.id == mine.id)
        .expect("there");
    assert_eq!(listed.command, "/somewhere/target/debug/claw");
    assert_eq!(listed.reach, Reach::Missing, "nothing is there to run");
}

#[test]
fn a_discovered_command_is_not_the_persons_to_edit() {
    let resolved = profiles(&[], base_of, &["claude".to_owned()].into_iter().collect());
    let found = resolved.iter().find(|one| one.command == "claude");
    if let Some(found) = found {
        assert!(!found.mine, "devpit noticing is not somebody choosing");
    }
}

#[test]
fn a_step_that_runs_under_a_profile_is_found_by_name() {
    // What stands between deleting a profile and a lane that fails days later
    // with no connection to the deletion.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let project = store.add_project(dir.path(), None).expect("project");
    store
        .create_step(
            &project,
            "agent",
            "Review",
            r#"{"profile":"01JGLM"}"#,
            false,
        )
        .expect("step");

    let used = store.steps_using_profile("01JGLM").expect("query");
    assert_eq!(used.len(), 1);
    assert_eq!(used[0].step, "Review");
    assert!(
        !used[0].project.is_empty(),
        "the refusal has to say which board to go and look at"
    );
    assert!(store
        .steps_using_profile("01JOTHER")
        .expect("query")
        .is_empty());
}

#[test]
fn a_step_whose_name_merely_contains_the_id_is_not_a_use() {
    // `LIKE` would have matched this. The field is asked for by name.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let project = store.add_project(dir.path(), None).expect("project");
    store
        .create_step(
            &project,
            "command",
            "build 01JGLM",
            r#"{"command":"make"}"#,
            false,
        )
        .expect("step");

    assert!(store
        .steps_using_profile("01JGLM")
        .expect("query")
        .is_empty());
}

#[test]
fn a_config_that_is_not_json_does_not_break_the_question() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let project = store.add_project(dir.path(), None).expect("project");
    store
        .create_step(&project, "command", "odd", "not json at all", false)
        .expect("step");

    assert!(store
        .steps_using_profile("01JGLM")
        .expect("query")
        .is_empty());
}

#[test]
fn a_preference_nobody_can_parse_reads_as_an_empty_list() {
    // One bad row in a settings table must not take the pane with it.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    store
        .set_preference(preference::AGENT_PROFILES, "{ not json")
        .expect("write");
    assert_eq!(stored(&store).expect("read"), Vec::new());
}

#[test]
fn a_preference_nobody_can_parse_is_never_written_over() {
    // Saving over it would lose every profile the person had, for one typo.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let broken = "[{\"id\": \"01JGLM\", \"label\": \"GLM\"";
    store
        .set_preference(preference::AGENT_PROFILES, broken)
        .expect("write");
    let mut one = glm();
    one.id = "01JNEW".to_owned();

    assert!(upsert(&store, one).is_err());
    assert!(remove(&store, "01JGLM").is_err());
    assert_eq!(
        store
            .preference(preference::AGENT_PROFILES)
            .expect("read")
            .as_deref(),
        Some(broken)
    );
}

#[test]
fn a_subagent_name_is_not_a_profile() {
    // `config.agent` is a subagent from somebody's frontmatter and
    // `config.profile` is which account runs the step. They share the JSON and
    // nothing else, and asking for the wrong one refuses deletions nobody
    // asked about while allowing the ones that matter.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let project = store.add_project(dir.path(), None).expect("project");
    store
        .create_step(
            &project,
            "agent",
            "Review",
            r#"{"agent":"reviewer"}"#,
            false,
        )
        .expect("step");

    assert!(store
        .steps_using_profile("reviewer")
        .expect("query")
        .is_empty());
}

/*
 * The round trip, through a real database.
 *
 * Everything else about this feature is proved against values in memory. This
 * is what a person types, saved, closed, reopened, resolved and turned into
 * the line a terminal is asked to type — which is the seam the pieces cannot
 * see between them.
 */

fn glm_with_secret() -> Declared {
    let mut one = glm();
    one.id = "01JGLM".to_owned();
    one.env.push(EnvVar {
        name: "ANTHROPIC_AUTH_TOKEN".to_owned(),
        value: "sk-not-a-real-token".to_owned(),
    });
    one
}

#[test]
fn what_a_person_typed_is_what_the_terminal_is_asked_to_type() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mine = glm_with_secret();

    // Refused here or nowhere: this is the check the command runs.
    devpit_agentcli::declaring::allowed(&mine, |id| base_of(id).is_some())
        .expect("a real profile was refused");

    let store = open(dir.path());
    written(&store, mine);
    drop(store);

    let store = open(dir.path());
    let found = all(&store)
        .expect("resolve")
        .into_iter()
        .find(|one| one.id == "01JGLM")
        .expect("it did not survive the round trip");

    assert_eq!(found.label, "GLM");
    assert_eq!(found.command, "claude", "the base lends its program");
    assert_eq!(found.driver, "claude", "and its driver");
    assert!(found.mine);

    assert_eq!(
        devpit_agentcli::running::line(&devpit_agentcli::running::runner(&found)),
        "ANTHROPIC_BASE_URL='https://api.z.ai/api/anthropic' \
         CLAUDE_CONFIG_DIR='/home/someone/.claude-glm' \
         ANTHROPIC_AUTH_TOKEN='sk-not-a-real-token' \
         claude --permission-mode bypassPermissions"
    );
}

#[test]
fn two_accounts_differing_in_environment_alone_stay_two() {
    // The case that started this. If the store or the resolver flattened them
    // into one, the feature would have no point.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());

    written(&store, glm());
    let mut claude2 = glm();
    claude2.label = "Claude 2".to_owned();
    claude2.env = vec![EnvVar {
        name: "CLAUDE_CONFIG_DIR".to_owned(),
        value: "/home/someone/.claude-2".to_owned(),
    }];
    written(&store, claude2);

    let lines: Vec<String> = all(&store)
        .expect("resolve")
        .iter()
        .filter(|one| one.mine)
        .map(|one| devpit_agentcli::running::line(&devpit_agentcli::running::runner(one)))
        .collect();

    assert_eq!(lines.len(), 2);
    assert_ne!(lines[0], lines[1], "two accounts became one command");
    assert!(lines.iter().all(|said| said.contains("claude")));
}

#[test]
fn a_profile_whose_base_left_the_build_is_still_listed() {
    // Somebody chose it. Dropping the row explains nothing, and a person who
    // cannot see it cannot delete it either.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let mut orphan = glm();
    orphan.base = "an-agent-this-build-forgot".to_owned();
    orphan.command = "claude".to_owned();
    let mine = written(&store, orphan);

    let found = all(&store)
        .expect("resolve")
        .into_iter()
        .find(|one| one.id == mine.id)
        .expect("it was dropped");
    assert_eq!(found.command, "claude", "it kept the program it named");
    assert_eq!(found.driver, "", "and lost the driver it no longer has");
}

#[test]
fn a_step_naming_a_profile_that_is_gone_says_so_before_it_spends_anything() {
    // `runner_for` is what the board calls. A missing profile has to be an
    // error and never a quiet fall back to the default account — the
    // difference between them is somebody's bill.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let why = runner_for(&store, "01JGONE").expect_err("it resolved something");
    assert!(why.contains("no longer exists"), "{why}");
}

#[test]
fn a_step_naming_a_shell_function_is_refused_rather_than_spawned() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let mut wrapped = glm();
    wrapped.command = "definitely-not-a-real-command".to_owned();
    let mine = written(&store, wrapped);

    let why = runner_for(&store, &mine.id).expect_err("it resolved something");
    assert!(why.contains("outside a terminal"), "{why}");
}

#[test]
fn a_profile_switched_off_says_so_in_the_list() {
    // The composer and the step editor read this list, not `agents.known`, so
    // a switch that only reached the other list switched nothing off for them.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let off = written(&store, glm());
    let on = written(&store, glm_with_secret());
    store
        .set_preference(
            devpit_core::store::preference::AGENTS_DISABLED,
            &serde_json::to_string(&[&off.id]).expect("json"),
        )
        .expect("write");

    let found = all(&store).expect("list");
    let enabled = |id: &str| found.iter().find(|one| one.id == id).map(|one| one.enabled);
    assert_eq!(enabled(&off.id), Some(false));
    assert_eq!(enabled(&on.id), Some(true));
}

#[test]
fn a_home_written_before_it_was_made_absolute_is_made_absolute_on_the_way_out() {
    // Profiles saved before `at_home` ran on save still hold a literal `~`,
    // which no shell ever expands and which names a folder called `~`.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let mut old = glm();
    old.env[1].value = "~/.claude-glm".to_owned();
    written(&store, old);

    let home = crate::installations::home().expect("home");
    let back = stored(&store).expect("read");
    assert_eq!(
        back[0].env[1].value,
        format!(
            "{}/.claude-glm",
            home.to_string_lossy().trim_end_matches('/')
        )
    );
}

/// Profiles stored by an earlier version, one from before profiles named
/// their own models, open as written and save back without losing a field.
#[test]
fn profiles_stored_by_an_earlier_version_open_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let earlier = r#"[{"id":"01J0GLM","label":"GLM","base":"claude","command":"","args":["--permission-mode","bypassPermissions"],"env":[{"name":"CLAUDE_CONFIG_DIR","value":"/home/someone/.claude-glm"}],"models":["glm-4.6"]},{"id":"01J0OLD","label":"Old","base":"codex"}]"#;
    store
        .set_preference(preference::AGENT_PROFILES, earlier)
        .expect("write");

    let read = stored(&store).expect("read");
    assert_eq!(read.len(), 2);
    assert_eq!(read[0].args, ["--permission-mode", "bypassPermissions"]);
    assert_eq!(read[0].env[0].value, "/home/someone/.claude-glm");
    assert_eq!(read[0].models, ["glm-4.6"]);
    assert_eq!(
        (read[1].base.as_str(), read[1].command.as_str()),
        ("codex", "")
    );

    save(&store, &read).expect("save back");
    assert_eq!(stored(&store).expect("read again"), read);
    let listed = all(&store).expect("profiles");
    assert!(listed
        .iter()
        .any(|one| one.id == "01J0OLD" && one.command == "codex"));
}
