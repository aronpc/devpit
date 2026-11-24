use super::{brief_in, imported, on, IMPORT, KEY};

fn en() -> crate::session_locale::SessionLocale {
    crate::session_locale::SessionLocale::of(devpit_core::locale::Locale::En)
}

fn pt_br() -> crate::session_locale::SessionLocale {
    crate::session_locale::SessionLocale::of(devpit_core::locale::Locale::PtBr)
}

const SEEDED: &str = include_str!("orchestrator_claude.md");

/// The orchestrator's context: what `CLAUDE.md` imports, and whether each
/// imported file is there to be read.
fn context(folder: &std::path::Path) -> Vec<(String, bool)> {
    std::fs::read_to_string(folder.join("CLAUDE.md"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().strip_prefix('@'))
        .map(|path| (path.to_owned(), folder.join(path).is_file()))
        .collect()
}

#[test]
fn with_assistant_mode_on_the_context_imports_the_assistant_brief_and_off_it_is_gone() {
    let dir = tempfile::tempdir().expect("tmp");
    let folder = dir.path();
    std::fs::write(folder.join("CLAUDE.md"), SEEDED).expect("seeded");
    std::fs::create_dir_all(folder.join(".devpit")).expect("dir");
    std::fs::write(folder.join(".devpit/orchestrator.md"), "devpit").expect("brief");

    brief_in(folder, true, en()).expect("on");
    let on = context(folder);
    assert_eq!(
        on,
        [
            (".devpit/orchestrator.md".to_owned(), true),
            (".devpit/assistant.md".to_owned(), true),
        ]
    );
    let brief = std::fs::read_to_string(folder.join(".devpit/assistant.md")).expect("brief");
    assert!(brief.contains("Only the person schedules"));

    brief_in(folder, false, en()).expect("off");
    assert_eq!(
        context(folder),
        [(".devpit/orchestrator.md".to_owned(), true)]
    );
    assert!(!folder.join(".devpit/assistant.md").exists());
    assert_eq!(
        std::fs::read_to_string(folder.join("CLAUDE.md")).expect("read"),
        SEEDED,
        "the person's file is as it was"
    );
}

#[test]
fn the_import_goes_under_devpits_brief_once_and_keeps_the_persons_lines() {
    let mine = "# Mine\n\n@.devpit/orchestrator.md\n\nKeep it short.\n";
    let on = imported(mine, true);
    assert_eq!(
        on,
        format!("# Mine\n\n@.devpit/orchestrator.md\n{IMPORT}\n\nKeep it short.\n")
    );
    assert_eq!(imported(&on, true), on, "turned on twice, imported once");
    assert_eq!(imported(&on, false), mine);
    // A file whose brief line was removed still gets the import, at its end.
    assert_eq!(imported("Notes", true), format!("Notes\n{IMPORT}\n"));
    assert_eq!(imported("Notes", false), "Notes");
}

/// Off for a new orchestrator, on once the person turns it on, and kept.
#[test]
fn assistant_mode_is_off_by_default_and_kept_as_the_person_set_it() {
    let dir = tempfile::tempdir().expect("tmp");
    let store = devpit_core::Store::open(&dir.path().join("state.db")).expect("store");
    let folder = dir.path().join("orch");
    std::fs::create_dir_all(&folder).expect("folder");
    assert!(!on(&store, &folder), "off by default");
    store
        .set_orchestrator_setting(&folder, KEY, serde_json::json!(true))
        .expect("on");
    let reopened = devpit_core::Store::open(&dir.path().join("state.db")).expect("store");
    assert!(on(&reopened, &folder), "kept");
    store
        .set_orchestrator_setting(&folder, KEY, serde_json::json!(false))
        .expect("off");
    assert!(!on(&store, &folder));
}

/// A brief that is a link to a file elsewhere is not written through.
#[cfg(unix)]
#[test]
fn a_brief_linked_out_of_the_folder_is_left_alone() {
    let dir = tempfile::tempdir().expect("tmp");
    let folder = dir.path().join("orch");
    std::fs::create_dir_all(folder.join(".devpit")).expect("dir");
    std::fs::write(folder.join("CLAUDE.md"), SEEDED).expect("seeded");
    let elsewhere = dir.path().join("elsewhere.txt");
    std::fs::write(&elsewhere, "mine").expect("elsewhere");
    std::os::unix::fs::symlink(&elsewhere, folder.join(".devpit/assistant.md")).expect("link");
    assert!(brief_in(&folder, true, en()).is_err());
    assert_eq!(std::fs::read_to_string(&elsewhere).expect("read"), "mine");
}

/// Portuguese writes the pt-BR variant of the assistant brief, under the
/// same line naming the build: one text per language, one shape for both.
#[test]
fn portuguese_writes_the_pt_br_assistant_brief() {
    let dir = tempfile::tempdir().expect("tmp");
    let folder = dir.path();
    std::fs::write(folder.join("CLAUDE.md"), SEEDED).expect("seeded");
    std::fs::create_dir_all(folder.join(".devpit")).expect("dir");
    std::fs::write(folder.join(".devpit/orchestrator.md"), "devpit").expect("brief");

    brief_in(folder, true, pt_br()).expect("on");
    assert_eq!(
        std::fs::read_to_string(folder.join(".devpit/assistant.md")).expect("assistant"),
        format!(
            "<!-- written by devpit {} -->\n{}",
            env!("CARGO_PKG_VERSION"),
            include_str!("assistant_devpit_pt_br.md")
        )
    );
}

/// Each pt-BR brief stays within a quarter more characters than its English
/// base: a translation, not a rewrite that outgrows what it translates.
#[test]
fn the_pt_br_briefs_stay_within_a_quarter_more_characters() {
    for (en, pt) in [
        (
            include_str!("orchestrator_devpit.md"),
            include_str!("orchestrator_devpit_pt_br.md"),
        ),
        (
            include_str!("orchestrator_claude.md"),
            include_str!("orchestrator_claude_pt_br.md"),
        ),
        (
            include_str!("assistant_devpit.md"),
            include_str!("assistant_devpit_pt_br.md"),
        ),
    ] {
        assert!(
            pt.chars().count() <= en.chars().count() * 5 / 4,
            "a {}-character base carries a {}-character pt-BR brief",
            en.chars().count(),
            pt.chars().count()
        );
    }
}

/// English writes the assistant brief as it always did: the base text under
/// the line naming the build, byte for byte, whatever came to carry the
/// language.
#[test]
fn english_writes_the_assistants_bytes_as_they_always_were() {
    let dir = tempfile::tempdir().expect("tmp");
    let folder = dir.path();
    std::fs::write(folder.join("CLAUDE.md"), SEEDED).expect("seeded");
    std::fs::create_dir_all(folder.join(".devpit")).expect("dir");
    std::fs::write(folder.join(".devpit/orchestrator.md"), "devpit").expect("brief");

    brief_in(folder, true, en()).expect("on");
    assert_eq!(
        std::fs::read_to_string(folder.join(".devpit/assistant.md")).expect("assistant"),
        format!(
            "<!-- written by devpit {} -->\n{}",
            env!("CARGO_PKG_VERSION"),
            include_str!("assistant_devpit.md")
        )
    );
}
