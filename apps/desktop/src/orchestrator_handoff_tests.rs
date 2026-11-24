use super::{handoff, switched, take};

/// A Claude Code orchestrator switched to OpenCode keeps its folder, notes and
/// links; its first turn there is handed over from its notes, once, and
/// switching back hands over again.
#[test]
fn a_switched_orchestrator_keeps_its_notes_and_links_and_is_handed_over_once() {
    let store = crate::projects::store().expect("store");
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path();
    crate::orchestrator::seed(
        folder,
        crate::session_locale::SessionLocale::of(devpit_core::locale::Locale::En),
    )
    .expect("seeded");
    std::fs::write(
        folder.join("context/now.md"),
        "# Now\n\napi-worker is busy on #42\n",
    )
    .expect("now");
    std::fs::write(
        folder.join("context/sessions.md"),
        "- api-worker: card #42, started\n",
    )
    .expect("log");
    crate::orchestrator_links::set_in(
        &store,
        folder,
        "projects",
        serde_json::json!(["prj_api", "prj_web"]),
    )
    .expect("linked");
    crate::orchestrator::speaks_as(&store, folder, "claude").expect("claude");

    switched(&store, folder, "claude", "opencode");
    crate::orchestrator::speaks_as(&store, folder, "opencode").expect("opencode");

    assert_eq!(
        crate::orchestrator_links::linked_in(&store, folder),
        ["prj_api", "prj_web"]
    );
    assert!(folder.join("context/now.md").is_file());
    assert!(take(folder, "claude").is_none(), "not switched to claude");
    let handed = take(folder, "opencode").expect("handed over");
    assert!(
        handed.contains("switched from claude to opencode"),
        "{handed}"
    );
    assert!(handed.contains("api-worker is busy on #42"), "{handed}");
    assert!(
        handed.contains("- api-worker: card #42, started"),
        "{handed}"
    );
    assert!(take(folder, "opencode").is_none(), "handed over twice");

    switched(&store, folder, "opencode", "claude");
    let back = take(folder, "claude").expect("handed back");
    assert!(back.contains("switched from opencode to claude"), "{back}");
}

/// The handoff is one notice, so the person's words under it are still
/// theirs alone, and a long note is cut.
#[test]
fn a_handoff_is_one_notice_and_short() {
    let now = format!("# Now\n\n{}", "x".repeat(5000));
    let said = handoff("claude", "codex", &now, "\n\n- a\n\n- b\n");
    assert!(!said.contains("\n\n"), "{said}");
    assert!(said.chars().count() < 3500, "{}", said.chars().count());
    let sent = format!("{said}\n\n✅ api");
    let read = devpit_agentcli::person::said_in("t", &sent).expect("the person's");
    assert_eq!(read.text, "✅ api");
}
