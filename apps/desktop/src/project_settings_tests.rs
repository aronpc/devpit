use super::*;

fn scoped(model: Option<&str>, mode: Option<&str>, pattern: Option<&str>) -> Scoped {
    Scoped {
        model: model.map(str::to_owned),
        permission_mode: mode.map(str::to_owned),
        branch_pattern: pattern.map(str::to_owned),
        automatic_switch: None,
    }
}

fn repo_file(root: &Path, text: &str) {
    std::fs::create_dir_all(root.join(".devpit")).expect("mkdir");
    std::fs::write(root.join(REPO_FILE), text).expect("write");
}

#[test]
fn the_project_wins_over_its_repo_which_wins_over_the_machine() {
    let own = scoped(Some("opus"), None, None);
    let repo = scoped(Some("sonnet"), Some("plan"), None);
    let machine = scoped(Some("haiku"), Some("manual"), Some("feat/{number}-{slug}"));
    assert_eq!(
        applied(&own, &repo, &machine),
        scoped(Some("opus"), Some("plan"), Some("feat/{number}-{slug}"))
    );
    assert_eq!(
        applied(&Scoped::default(), &Scoped::default(), &Scoped::default()),
        Scoped::default()
    );
}

#[test]
fn the_repo_file_gives_its_model_permission_mode_and_branch_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(from_repo(dir.path()), Ok(Scoped::default()));
    repo_file(
        dir.path(),
        r#"{"model":"sonnet","permissionMode":"plan","worktree":{"branchPattern":"fix/{slug}"}}"#,
    );
    assert_eq!(
        from_repo(dir.path()),
        Ok(scoped(Some("sonnet"), Some("plan"), Some("fix/{slug}")))
    );
}

#[test]
fn a_repo_file_that_is_invalid_too_big_or_refused_is_left_out() {
    let dir = tempfile::tempdir().expect("tempdir");
    repo_file(dir.path(), "{ not json");
    assert!(from_repo(dir.path()).is_err());
    repo_file(dir.path(), r#"{"permissionMode":"yolo"}"#);
    assert!(from_repo(dir.path()).is_err());
    repo_file(dir.path(), r#"{"model":"opus; curl evil"}"#);
    assert!(from_repo(dir.path()).is_err());
    let padded = format!(
        r#"{{"model":"opus","pad":"{}"}}"#,
        "x".repeat(LARGEST as usize)
    );
    repo_file(dir.path(), &padded);
    let said = from_repo(dir.path()).expect_err("too big");
    assert!(said.contains("bigger than"), "{said}");
}

#[cfg(unix)]
#[test]
fn a_repo_file_that_leads_outside_the_project_is_not_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(outside.path().join("settings.json"), r#"{"model":"opus"}"#).expect("write");
    std::fs::create_dir_all(dir.path().join(".devpit")).expect("mkdir");
    std::os::unix::fs::symlink(
        outside.path().join("settings.json"),
        dir.path().join(REPO_FILE),
    )
    .expect("link");
    let said = from_repo(dir.path()).expect_err("outside");
    assert!(said.contains("outside the project"), "{said}");
}

#[test]
fn a_project_reads_its_own_its_repos_and_the_machines_settings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("state.db")).expect("open");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("mkdir");
    let project = store.add_project(&root, None).expect("project");

    // Nothing set anywhere: new chats and worktrees start as they always did.
    let nothing = read(&store, Some(&project)).expect("read");
    assert_eq!(nothing.applied, Scoped::default());
    assert_eq!(branch_pattern(&store, &project), "");

    store
        .set_preference(preference::WORKTREE_BRANCH, "feat/{number}-{slug}")
        .expect("machine");
    store
        .set_preference(preference::CHAT_MODEL, "haiku")
        .expect("machine");
    repo_file(&root, r#"{"permissionMode":"plan","model":"sonnet"}"#);
    let mut own = owned(&store);
    own.insert(project.clone(), scoped(Some("opus"), None, None));
    store
        .set_preference(
            preference::PROJECT_SETTINGS,
            &serde_json::to_string(&own).expect("json"),
        )
        .expect("own");

    let shown = read(&store, Some(&project)).expect("read");
    assert_eq!(
        shown.applied,
        scoped(Some("opus"), Some("plan"), Some("feat/{number}-{slug}"))
    );
    assert_eq!(shown.repo.model.as_deref(), Some("sonnet"));
    assert_eq!(shown.repo_problem, None);
    assert_eq!(branch_pattern(&store, &project), "feat/{number}-{slug}");

    let machine = read(&store, None).expect("machine");
    assert_eq!(machine.applied.model.as_deref(), Some("haiku"));
    assert_eq!(machine.own, Scoped::default());
}

#[test]
fn only_model_names_known_modes_and_branch_patterns_are_kept() {
    assert!(refused(&scoped(
        Some("opus"),
        Some("bypassPermissions"),
        Some("feat/{slug}")
    ))
    .is_none());
    assert!(refused(&scoped(Some("op us"), None, None)).is_some());
    assert!(refused(&scoped(None, Some("everything"), None)).is_some());
    assert!(refused(&scoped(None, None, Some("feat/..{slug}"))).is_some());
}

#[test]
fn a_repository_never_starts_chats_in_a_mode_that_asks_less_than_supervised() {
    let dir = tempfile::tempdir().expect("tempdir");
    for mode in ["bypassPermissions", "acceptEdits"] {
        repo_file(
            dir.path(),
            &format!(r#"{{"permissionMode":"{mode}","model":"opus"}}"#),
        );
        let said = from_repo(dir.path()).expect_err(mode);
        assert!(said.contains("only you choose"), "{said}");
    }
    repo_file(dir.path(), r#"{"permissionMode":"plan"}"#);
    assert!(from_repo(dir.path()).is_ok());
}

#[cfg(unix)]
#[test]
fn a_repo_file_that_is_not_a_file_is_not_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(REPO_FILE)).expect("a folder in its place");
    let said = from_repo(dir.path()).expect_err("not a file");
    assert!(said.contains("not a file"), "{said}");
}

#[test]
fn one_setting_changes_and_the_others_stay() {
    let set = scoped(Some("opus"), Some("plan"), Some("fix/{slug}"));
    assert_eq!(
        with_field(set.clone(), "model", Some("haiku".to_owned())),
        Ok(scoped(Some("haiku"), Some("plan"), Some("fix/{slug}")))
    );
    assert_eq!(
        with_field(set.clone(), "permissionMode", Some("  ".to_owned())),
        Ok(scoped(Some("opus"), None, Some("fix/{slug}")))
    );
    assert!(with_field(set.clone(), "theme", Some("dark".to_owned())).is_err());
    assert!(with_field(set, "model", Some("not a model".to_owned())).is_err());
    assert_eq!(
        with_field(
            Scoped::default(),
            "automaticSwitch",
            Some("true".to_owned())
        )
        .expect("automatic switch")
        .automatic_switch,
        Some(true)
    );
}
