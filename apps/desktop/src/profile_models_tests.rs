use devpit_rpc::Declared;

use super::*;

fn open(dir: &Path) -> Store {
    Store::open(&dir.join("state.db")).expect("open")
}

fn kept_as(store: &Store, profile_id: &str, models: ProfileModels) {
    let mut all = kept(store);
    all.insert(profile_id.to_owned(), models);
    store
        .set_preference(
            preference::AGENTS_PROFILE_MODELS,
            &serde_json::to_string(&all).expect("json"),
        )
        .expect("write");
}

#[test]
fn every_model_is_on_and_there_is_no_advisor_until_one_is_set() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(of(&open(dir.path()), "glm"), ProfileModels::default());
}

#[test]
fn a_model_switched_off_leaves_the_profiles_offered_models() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    let declared = Declared {
        id: "glm".to_owned(),
        label: "GLM".to_owned(),
        base: "claude".to_owned(),
        command: String::new(),
        args: Vec::new(),
        env: Vec::new(),
        models: vec!["glm-4.6".to_owned(), "glm-4.5-air".to_owned()],
    };
    crate::agent_profiles::save(&store, &[declared]).expect("save");
    let offered = |store: &Store| {
        crate::agent_profiles::all(store)
            .expect("profiles")
            .into_iter()
            .find(|one| one.id == "glm")
            .expect("glm")
            .models
    };
    assert!(offered(&store).contains(&"glm-4.5-air".to_owned()));

    kept_as(
        &store,
        "glm",
        ProfileModels {
            models_off: vec!["glm-4.5-air".to_owned()],
            advisor_model: None,
            ..Default::default()
        },
    );
    let now = offered(&store);
    assert!(!now.contains(&"glm-4.5-air".to_owned()), "{now:?}");
    assert!(now.contains(&"glm-4.6".to_owned()), "{now:?}");
}

#[test]
fn only_model_names_are_kept_and_an_empty_advisor_is_none() {
    assert!(checked(ProfileModels {
        models_off: vec!["opus; rm -rf".to_owned()],
        advisor_model: None,
        ..Default::default()
    })
    .is_err());
    assert!(checked(ProfileModels {
        models_off: Vec::new(),
        advisor_model: Some("not a model".to_owned()),
        ..Default::default()
    })
    .is_err());
    assert_eq!(
        checked(ProfileModels {
            models_off: vec!["haiku".to_owned()],
            advisor_model: Some("  ".to_owned()),
            ..Default::default()
        }),
        Ok(ProfileModels {
            models_off: vec!["haiku".to_owned()],
            advisor_model: None,
            ..Default::default()
        })
    );
}

#[test]
fn the_default_model_is_never_switched_off() {
    assert!(checked(ProfileModels {
        models_off: vec![DEFAULT_MODEL.to_owned()],
        advisor_model: None,
        ..Default::default()
    })
    .is_err());
}

#[test]
fn a_profile_with_an_advisor_gets_a_hooks_file_of_its_own() {
    let root = Path::new("/home/me/.devpit");
    assert_eq!(hooks_file(root, None), root.join("hooks.json"));
    assert_eq!(hooks_file(root, Some("glm")), root.join("hooks-glm.json"));
    assert_eq!(
        hooks_file(root, Some("../x y")),
        root.join("hooks-___x_y.json")
    );
}

#[test]
fn the_advisor_is_written_in_devpits_settings_for_the_profiles_sessions_only() {
    let profile = format!("advisor-{}", ulid::Ulid::generate());
    let store = Store::open_default().expect("store");
    kept_as(
        &store,
        &profile,
        ProfileModels {
            models_off: Vec::new(),
            advisor_model: Some("opus".to_owned()),
            ..Default::default()
        },
    );
    let own = crate::steps::hook_settings_for(Some(&profile)).expect("own settings");
    assert!(own.ends_with(&format!("hooks-{profile}.json")), "{own}");
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&own).expect("read")).expect("json");
    assert_eq!(written["advisorModel"], "opus");
    assert!(written["hooks"].is_object());

    let shared = crate::steps::hook_settings_for(Some("no-advisor-here")).expect("shared");
    assert!(shared.ends_with("hooks.json"), "{shared}");
    let plain: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&shared).expect("read")).expect("json");
    assert!(plain.get("advisorModel").is_none());
}

#[test]
fn checks_run_every_five_minutes_until_another_offered_time_is_chosen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    assert_eq!(check_minutes(&store), 5);
    store
        .set_preference(preference::PROVIDERS_CHECK_MINUTES, "15")
        .expect("write");
    assert_eq!(check_minutes(&store), 15);
    store
        .set_preference(preference::PROVIDERS_CHECK_MINUTES, "2")
        .expect("write");
    assert_eq!(check_minutes(&store), 5);
}

/// Profiles' settings from before a default model and effort existed read
/// unchanged, with neither set.
#[test]
fn settings_kept_before_the_default_model_and_effort_read_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    store
        .set_preference(
            preference::AGENTS_PROFILE_MODELS,
            r#"{"glm":{"modelsOff":["glm-4.5-air"],"advisorModel":"opus"}}"#,
        )
        .expect("write");
    let glm = of(&store, "glm");
    assert_eq!(glm.models_off, ["glm-4.5-air"]);
    assert_eq!(glm.advisor_model.as_deref(), Some("opus"));
    assert_eq!(glm.model, None);
    assert_eq!(glm.effort, None);
}

#[test]
fn a_profiles_default_model_and_effort_are_checked_and_trimmed() {
    let set = checked(ProfileModels {
        model: Some(" gpt-5.5 ".to_owned()),
        effort: Some("high".to_owned()),
        ..Default::default()
    })
    .expect("allowed");
    assert_eq!(set.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(set.effort.as_deref(), Some("high"));
    assert!(checked(ProfileModels {
        effort: Some("high; rm".to_owned()),
        ..Default::default()
    })
    .is_err());
    let empty = checked(ProfileModels {
        model: Some(" ".to_owned()),
        effort: Some(String::new()),
        ..Default::default()
    })
    .expect("allowed");
    assert_eq!((empty.model, empty.effort), (None, None));
}

#[test]
fn a_profiles_own_effort_is_the_one_its_chats_start_with() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(dir.path());
    crate::agent_profiles::save(
        &store,
        &[Declared {
            id: "work".to_owned(),
            label: "Work".to_owned(),
            base: "claude".to_owned(),
            command: String::new(),
            args: Vec::new(),
            env: Vec::new(),
            models: Vec::new(),
        }],
    )
    .expect("save");
    kept_as(
        &store,
        "work",
        ProfileModels {
            model: Some("opus".to_owned()),
            effort: Some("low".to_owned()),
            ..Default::default()
        },
    );
    let work = crate::agent_profiles::all(&store)
        .expect("profiles")
        .into_iter()
        .find(|one| one.id == "work")
        .expect("work");
    assert_eq!(work.effort_default.as_deref(), Some("low"));
    assert_eq!(default_model(&store, "work").as_deref(), Some("opus"));
    assert_eq!(default_model(&store, "elsewhere"), None);
}
