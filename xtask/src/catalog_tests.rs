//! The catalog guard, and each way of breaking it.

use super::*;

fn area(english: &[(&str, serde_json::Value)], portuguese: &[(&str, serde_json::Value)]) -> Area {
    fn map(
        entries: &[(&str, serde_json::Value)],
    ) -> std::collections::BTreeMap<String, serde_json::Value> {
        entries
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect()
    }
    Area {
        name: "common".into(),
        english: map(english),
        portuguese: map(portuguese),
    }
}

/// A pair that keeps step, for the sabotages to bend one side of.
fn parallel() -> Area {
    area(
        &[
            ("greet.hello", serde_json::json!("Hello, {name}")),
            (
                "board.cards",
                serde_json::json!({"one": "{count} card", "other": "{count} cards"}),
            ),
        ],
        &[
            ("greet.hello", serde_json::json!("Olá, {name}")),
            (
                "board.cards",
                serde_json::json!({"one": "{count} cartão", "other": "{count} cartões"}),
            ),
        ],
    )
}

fn disagreements_of(area: &Area, allowed: &[&str]) -> Vec<Finding> {
    let listed: Vec<String> = allowed.iter().map(|key| key.to_string()).collect();
    disagreements(area, &listed, "web/src/i18n")
}

fn as_text(found: &[Finding]) -> String {
    found
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

#[test]
fn a_pair_that_keeps_step_is_not_refused() {
    assert!(
        disagreements_of(&parallel(), &[]).is_empty(),
        "{}",
        as_text(&disagreements_of(&parallel(), &[]))
    );
}

#[test]
fn an_english_key_the_portuguese_catalog_lacks_is_refused_with_file_and_key() {
    let mut bent = parallel();
    bent.english.insert(
        "board.englishOnly".into(),
        serde_json::json!("Only English"),
    );
    let found = disagreements_of(&bent, &[]);
    assert_eq!(found.len(), 1, "{}", as_text(&found));
    assert_eq!(
        found[0].file,
        std::path::Path::new("web/src/i18n/pt-BR/common.json")
    );
    assert!(
        found[0].what.contains("board.englishOnly"),
        "{}",
        found[0].what
    );
}

/// The list is how a key may wait for its translation in the open, not a way
/// around the guard — so it absolves the missing key and nothing else.
#[test]
fn a_key_listed_untranslated_with_its_reason_is_allowed() {
    let mut bent = parallel();
    bent.english.insert(
        "board.englishOnly".into(),
        serde_json::json!("Only English"),
    );
    assert!(
        disagreements_of(&bent, &["board.englishOnly"]).is_empty(),
        "{}",
        as_text(&disagreements_of(&bent, &["board.englishOnly"]))
    );
}

#[test]
fn the_list_is_one_key_per_line_with_its_reason_after_the_hash() {
    let keys = keys_of(
        "# why a key may wait, and how the list is read\nbrand.name  # a proper noun\n\nbrand.old\n",
    );
    assert_eq!(keys, vec!["brand.name", "brand.old"]);
}

#[test]
fn a_value_that_says_nothing_is_refused_on_either_side() {
    let mut bent = parallel();
    bent.portuguese
        .insert("greet.hello".into(), serde_json::json!(""));
    let found = disagreements_of(&bent, &[]);
    assert!(
        found
            .iter()
            .any(|finding| finding.what.contains("`greet.hello` says nothing")),
        "{}",
        as_text(&found)
    );

    let mut bent = parallel();
    bent.english
        .insert("greet.hello".into(), serde_json::json!("  "));
    let found = disagreements_of(&bent, &[]);
    assert!(
        found
            .iter()
            .any(|finding| finding.what.contains("`greet.hello` says nothing")),
        "{}",
        as_text(&found)
    );
}

#[test]
fn a_placeholder_one_side_lacks_is_refused() {
    let mut bent = parallel();
    bent.portuguese.insert(
        "greet.hello".into(),
        serde_json::json!("Olá, {name} {sobrenome}"),
    );
    let found = disagreements_of(&bent, &[]);
    assert_eq!(found.len(), 1, "{}", as_text(&found));
    assert!(
        found[0].what.contains("{sobrenome}"),
        "the extra placeholder is not named: {}",
        found[0].what
    );
}

#[test]
fn a_plural_without_one_and_other_is_refused_on_either_side() {
    let mut bent = parallel();
    bent.portuguese.insert(
        "board.cards".into(),
        serde_json::json!({"one": "{count} cartão"}),
    );
    let found = disagreements_of(&bent, &[]);
    assert_eq!(found.len(), 1, "{}", as_text(&found));
    assert!(
        found[0].what.contains("other") && found[0].what.contains("board.cards"),
        "{}",
        found[0].what
    );

    let mut bent = parallel();
    bent.english.insert(
        "board.cards".into(),
        serde_json::json!({"many": "{count} cards"}),
    );
    let found = disagreements_of(&bent, &[]);
    assert_eq!(found.len(), 2, "{}", as_text(&found));
}

#[test]
fn a_key_only_the_portuguese_catalog_carries_is_refused() {
    let mut bent = parallel();
    bent.portuguese
        .insert("board.ptOnly".into(), serde_json::json!("Só aqui"));
    let found = disagreements_of(&bent, &[]);
    assert_eq!(found.len(), 1, "{}", as_text(&found));
    assert!(found[0].what.contains("board.ptOnly"), "{}", found[0].what);
}

/// One tree in a tempdir: a web pair that keeps step, and whatever the test
/// does to the core pair.
fn a_tree_with_core(english: &str, portuguese: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("tempdir");
    let web = root.path().join("web/src/i18n");
    std::fs::create_dir_all(web.join("en")).expect("en");
    std::fs::create_dir_all(web.join("pt-BR")).expect("pt-BR");
    std::fs::write(web.join("en/common.json"), "{\"greet.hello\": \"Hello\"}").expect("en");
    std::fs::write(web.join("pt-BR/common.json"), "{\"greet.hello\": \"Olá\"}").expect("pt");
    /* The core's catalog is one flat pair, not a folder per language. */
    let core = root.path().join("crates/core/src/i18n");
    std::fs::create_dir_all(&core).expect("core");
    std::fs::write(core.join("en.json"), english).expect("the english catalog");
    std::fs::write(core.join("pt-BR.json"), portuguese).expect("the pt-BR catalog");
    root
}

#[test]
fn the_same_rule_reaches_the_core_catalogs() {
    let root = a_tree_with_core(
        "{\"greet.hello\": \"Hello\", \"board.coreOnly\": \"English alone\"}",
        "{\"greet.hello\": \"Olá\"}",
    );
    let found = every_catalog_keeps_up_with_english(root.path());
    assert_eq!(found.len(), 1, "{}", as_text(&found));
    assert_eq!(
        found[0].file,
        std::path::Path::new("crates/core/src/i18n/pt-BR.json")
    );
}

/// A tree without the core folder has nothing to compare there, and no
/// finding for the absence.
#[test]
fn a_core_folder_that_is_not_there_yet_is_not_a_finding() {
    let root = a_tree_with_core("{\"greet.hello\": \"Hello\"}", "{\"greet.hello\": \"Olá\"}");
    std::fs::remove_dir_all(root.path().join("crates/core/src/i18n")).expect("the core folder");
    assert!(
        every_catalog_keeps_up_with_english(root.path()).is_empty(),
        "{}",
        as_text(&every_catalog_keeps_up_with_english(root.path()))
    );
}

#[test]
fn an_untranslated_list_past_twenty_keys_is_refused() {
    let root = a_tree_with_core("{\"greet.hello\": \"Hello\"}", "{\"greet.hello\": \"Olá\"}");
    let list: String = (0..21)
        .map(|n| format!("key.{n}  # not translated yet\n"))
        .collect();
    std::fs::write(root.path().join("web/src/i18n/untranslated.txt"), list).expect("the list");
    let found = every_catalog_keeps_up_with_english(root.path());
    assert_eq!(found.len(), 1, "{}", as_text(&found));
    assert!(
        found[0].what.contains("20") && found[0].file.ends_with("untranslated.txt"),
        "{}",
        found[0]
    );
}

/// A catalog nobody can read is a broken build, not an empty translation.
#[test]
fn a_catalog_that_does_not_parse_is_refused() {
    let root = a_tree_with_core("{\"greet.hello\": \"Hello\"}", "{\"greet.hello\": \"Olá\"}");
    std::fs::write(
        root.path().join("web/src/i18n/en/common.json"),
        "{\"greet.hello\": ",
    )
    .expect("the broken catalog");
    let found = every_catalog_keeps_up_with_english(root.path());
    assert!(
        found.iter().any(|finding| finding.what.contains("parse")),
        "{}",
        as_text(&found)
    );
}

/// And the real tree passes, which is what makes the sabotages worth having.
#[test]
fn the_real_repository_passes() {
    let found = every_catalog_keeps_up_with_english(&crate::workspace_root());
    assert!(found.is_empty(), "{}", as_text(&found));
}

/// The core's own pair, broken the ways the guard names: a placeholder one
/// side lacks, and a key only Portuguese carries.
#[test]
fn the_core_pair_is_held_to_the_same_rules_as_the_web_pair() {
    let root = a_tree_with_core(
        "{\"tray.greet\": \"Hello, {name}\"}",
        "{\"tray.greet\": \"Olá\", \"tray.ptOnly\": \"Só aqui\"}",
    );
    let found = every_catalog_keeps_up_with_english(root.path());
    assert_eq!(found.len(), 2, "{}", as_text(&found));
    assert!(found
        .iter()
        .all(|f| f.file == std::path::Path::new("crates/core/src/i18n/pt-BR.json")));
}

/// The real core catalogs, as shipped, keep step.
#[test]
fn the_shipped_core_catalogs_keep_step() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let found = every_catalog_keeps_up_with_english(&root);
    assert!(found.is_empty(), "{}", as_text(&found));
}
