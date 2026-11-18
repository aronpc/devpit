use super::*;

/// Sabotage: pin `.devflow` in `pinned`, and a project already on
/// `.devpit/devflow/` would be set up again from nothing.
#[test]
fn every_project_keeps_devflow_where_devpit_always_did() {
    assert_eq!(file(), ".devpit/devflow/devflow.yaml");
    assert_eq!(layout().dir, ".devpit/devflow");
    assert_eq!(layout().dialect, &*DEVPIT);
}

/// Sabotage: serve devflow's text without the words it ships for devpit,
/// and the agent is told devflow's tools instead of `devpit_devflow_*`.
#[test]
fn the_skills_served_are_devflows_in_devpits_words() {
    for (name, _, text) in DEVPIT.assets.skills {
        assert!(!text.contains("{{tool:"), "{name} names a tool unsaid");
        assert!(!text.contains("[["), "{name} keeps a phrase unsaid");
    }
    let plan = DEVPIT
        .assets
        .skills
        .iter()
        .find(|(name, ..)| *name == "devflow-plan")
        .map(|(.., text)| *text)
        .expect("plan");
    assert!(plan.contains("`devpit_devflow_plan`"), "{plan}");
    let verify = DEVPIT
        .assets
        .parts
        .iter()
        .find(|(skill, part, _)| *skill == "devflow-review" && *part == "verify")
        .map(|(.., text)| *text)
        .expect("the review's verify part");
    assert!(!verify.contains("{{tool:"), "{verify}");
}

#[test]
fn devflow_host_is_told_to_speak_devpits_words() {
    assert_eq!(host_words(), serde_json::json!({ "host": "devpit" }));
}
