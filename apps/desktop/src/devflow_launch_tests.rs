use devflow_engine::Ask;
use serde_json::{json, Value};

use super::*;

const EXE: &str = "/opt/devpit";

/// The session of these tests, born in English, the base.
fn en() -> crate::session_locale::SessionLocale {
    crate::session_locale::SessionLocale::of(devpit_core::locale::Locale::En)
}

#[test]
fn claude_code_gets_the_plugin_folder_for_its_session_only() {
    let root = tempfile::tempdir().expect("root");
    let flag = flags_for(en(), Cli::Claude, root.path(), EXE).expect("a flag");
    let base = folder(root.path(), Cli::Claude);
    assert_eq!(flag, format!("--plugin-dir='{}'", base.display()));
    let hooks: Value = serde_json::from_str(
        &std::fs::read_to_string(base.join("hooks/hooks.json")).expect("hooks"),
    )
    .expect("json");
    assert_eq!(
        hooks["hooks"]["Stop"][0]["hooks"][0]["tool"],
        "devpit_devflow_gate"
    );
    assert!(base.join(".claude-plugin/plugin.json").is_file());
    assert!(base.join("skills/completion/SKILL.md").is_file());
}

#[cfg(not(windows))]
#[test]
fn codex_gets_its_hooks_and_their_trust_on_the_line_and_its_skills_as_a_marketplace() {
    let root = tempfile::tempdir().expect("root");
    let flag = flags_for(en(), Cli::Codex, root.path(), EXE).expect("a flag");
    assert!(flag.starts_with("-c 'hooks={"), "{flag}");
    assert!(flag.contains("trusted_hash="), "{flag}");
    let base = folder(root.path(), Cli::Codex);
    assert!(base.join(".agents/plugins/marketplace.json").is_file());
    assert!(base
        .join("devflow/skills/devflow-completion/SKILL.md")
        .is_file());
}

/// The two crates name the same tool and the same cap.
#[test]
fn the_hooks_call_the_tool_the_server_lists_and_follow_the_same_cap() {
    assert_eq!(
        devpit_agentapi::devflow::GATE_TOOL,
        devflow_engine::plugin::launch::GATE_TOOL
    );
    assert_eq!(
        devpit_agentapi::devflow::CAP_ENV,
        devflow_engine::gate::CAP_ENV
    );
}

/// A project nobody set Devflow up in starts its sessions as before Devflow:
/// no tools listed, no plugin folder, no Stop hook. One already on stays on.
/// Sabotage: offer the setup tools where there is no file, and this fails.
#[test]
fn a_project_without_devflow_gets_none_of_it_and_one_with_it_keeps_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let store = crate::projects::store().expect("store");
    let id = store.add_project(&root, None).expect("project");
    let here = crate::devflow_screen::project_here(&store, &id).expect("project");
    let offer = || {
        crate::devflow::respond_from(
            devpit_core::locale::Locale::En,
            None,
            "devflow_offer",
            &here,
            &root,
            None,
            &json!({}),
        )
        .expect("offer")["offer"]
            .clone()
    };
    assert_eq!(offer(), "off");
    assert_eq!(flags(en(), "claude", &id), None);
    assert_eq!(flags(en(), "codex", &id), None);

    // Set up and switched on before this release: nothing to do again.
    std::fs::create_dir_all(root.join(".devpit/devflow")).expect("folder");
    std::fs::write(
        root.join(".devpit/devflow/devflow.yaml"),
        "version: 1\nenabled: true\n",
    )
    .expect("yaml");
    crate::devflow_asks::remember(&store, &id, Ask::Enable, true).expect("clicked");
    assert_eq!(offer(), "active");
    assert!(flags(en(), "claude", &id).is_some_and(|flag| flag.starts_with("--plugin-dir=")));
}

/// The devflow.yaml devpit's own repository kept before the engine, as it
/// was written: version 2, its own check script, no reviewers.
const BEFORE_THE_ENGINE: &str = "version: 2
enabled: true
stages:
  - uses: builtin/briefing
    enabled: true
    approve: person
  - uses: builtin/context
    enabled: true
  - uses: builtin/plan
    enabled: true
    approve: person
    publish: board   # board | jira | linear | github
  - uses: builtin/execution
    enabled: true
    checks:
      - name: unit
        run: sh .devpit/devflow/scripts/check-unit.sh
        report: none
  - uses: builtin/review
    enabled: true
  - uses: builtin/completion
    enabled: true
    gate: devflow-done
review:
  reviewers: []
  block_on: [critical, high]
  consolidate: jev
decisions:
  use: true
  mode: shadow
";

/// A `.devpit/devflow/` set up and switched on before the engine is read as
/// it is: still on, the same check, the file untouched.
/// Sabotage: read the folder from another path, and the offer is off.
#[test]
fn a_devflow_folder_from_before_the_engine_stays_on_without_a_migration() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let store = crate::projects::store().expect("store");
    let id = store.add_project(&root, None).expect("project");
    let here = crate::devflow_screen::project_here(&store, &id).expect("project");
    let file = root.join(".devpit/devflow/devflow.yaml");
    std::fs::create_dir_all(file.parent().expect("folder")).expect("folder");
    std::fs::write(&file, BEFORE_THE_ENGINE).expect("yaml");
    crate::devflow_asks::remember(&store, &id, Ask::Enable, true).expect("clicked");
    let ask = |method: &str| {
        crate::devflow::respond_from(
            devpit_core::locale::Locale::En,
            None,
            method,
            &here,
            &root,
            None,
            &json!({}),
        )
        .expect(method)
    };

    assert_eq!(ask("devflow_offer")["offer"], "active");
    let status = ask("devflow_status");
    assert_eq!(status["state"], "active");
    let checks = &status["config"]["stages"][3]["checks"];
    assert_eq!(checks[0]["name"], "unit");
    assert_eq!(checks[0]["run"], "sh .devpit/devflow/scripts/check-unit.sh");
    assert!(flags(en(), "claude", &id).is_some_and(|flag| flag.starts_with("--plugin-dir=")));
    assert_eq!(
        std::fs::read_to_string(&file).expect("yaml"),
        BEFORE_THE_ENGINE
    );
}

/// Switched on, a session of the project starts with Devflow's hooks;
/// switched off, the next one starts without them.
#[test]
fn switching_devflow_off_takes_the_hooks_off_the_next_launch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().expect("root");
    let store = crate::projects::store().expect("store");
    let id = store.add_project(&root, None).expect("project");
    std::fs::create_dir_all(root.join(".devpit/devflow")).expect("folder");
    std::fs::write(
        root.join(".devpit/devflow/devflow.yaml"),
        "version: 1\nenabled: true\n",
    )
    .expect("yaml");
    assert_eq!(
        flags(en(), "claude", &id),
        None,
        "the file alone waits for the click"
    );

    crate::devflow_asks::remember(&store, &id, Ask::Enable, true).expect("clicked");
    let on = flags(en(), "claude", &id).expect("on");
    assert!(on.starts_with("--plugin-dir="), "{on}");
    assert_eq!(
        flags(en(), "gemini", &id),
        None,
        "no hooks for a CLI devflow has none for"
    );

    crate::devflow_asks::remember(&store, &id, Ask::Enable, false).expect("off");
    assert_eq!(flags(en(), "claude", &id), None);
}
