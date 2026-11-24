use super::*;
use devpit_core::locale::Locale;

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

/// Sabotage: build the Portuguese dialect from `t()` instead of `t_for`, and
/// a session born in Portuguese is told the engine's words in English.
#[test]
fn the_dialect_says_its_sentences_in_the_sessions_language() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let pt = dialect_in(Locale::PtBr);
    assert_eq!(pt.plan_board, "os cards são feitos no quadro do devpit.");
    assert_eq!(pt.files_label, "Anexados");
    assert!(pt.decisions_no_key.contains("Configurações"));
    assert!(
        pt.plan_outside.contains("`card_…` ou `#42`"),
        "{}",
        pt.plan_outside
    );
}

/// Sabotage: translate a field of `DEVPIT` itself, and the English a person
/// who stays on English reads moves.
#[test]
fn in_english_the_dialect_is_the_one_devpit_always_had() {
    let en = dialect_in(Locale::En);
    assert!(std::ptr::eq(en, &*DEVPIT));
    assert_eq!(en.plan_board, "cards are made on the devpit board.");
    assert_eq!(en.files_label, "Attached");
    assert_eq!(
        en.decisions_no_key,
        "no key — set one in Settings → Decisions"
    );
}

/// Sabotage: put the interpreted vocabulary through the catalog, and the
/// engine stops recognising the tools, the variables and the folder.
#[test]
fn what_the_engine_interprets_stays_literal_in_every_language() {
    let en = dialect_in(Locale::En);
    let pt = dialect_in(Locale::PtBr);
    assert_eq!(pt.tools.gate, "devpit_devflow_gate");
    assert_eq!(pt.tools.stage, "devpit_devflow_stage");
    assert_eq!(pt.tools.finish, "devpit_finish_card");
    for (en, pt) in [
        (en.product, pt.product),
        (en.env, pt.env),
        (en.folder, pt.folder),
        (en.item, pt.item),
        (en.items, pt.items),
        (en.item_field, pt.item_field),
        (en.yaml_header, pt.yaml_header),
        (en.yaml_enabled, pt.yaml_enabled),
        (en.yaml_enforce, pt.yaml_enforce),
        (en.yaml_provider, pt.yaml_provider),
    ] {
        assert_eq!(en, pt);
    }
    assert!(std::ptr::eq(en.assets, pt.assets), "the assets are one set");
}

/// The assets installed into a person's repository are shared with their
/// team, so they do not follow whoever installed them: the skills carry no
/// Portuguese, and the language of the session does not reach the files the
/// plugin writes. Sabotage: render a skill through the catalog.
#[test]
fn the_installed_assets_stay_in_english_whatever_the_language() {
    let _turn = crate::locale_turn::settled(Locale::PtBr);
    for (name, _, text) in DEVPIT.assets.skills {
        for pt in ["você", "Você", "não ", "Responda", "ção "] {
            assert!(!text.contains(pt), "{name} says `{pt}`");
        }
    }
    let sources = [
        include_str!("devflow_skills.rs"),
        include_str!("devflow_plugin.rs"),
    ];
    for source in sources {
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(
            !production.contains("t_for"),
            "an asset goes through the catalog"
        );
        assert!(
            !production.contains("i18n::t"),
            "an asset goes through the catalog"
        );
    }
}
