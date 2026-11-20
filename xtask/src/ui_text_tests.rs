//! The fixed-screen-text guard, and each way of breaking it.

use super::*;

fn count(markup: &str) -> usize {
    fixed_text_lines(markup).len()
}

fn as_text(found: &[Finding]) -> String {
    found
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// A one-file tree of screens, for the guard to read.
fn screen_tree(name: &str, contents: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let shell = dir.path().join("web/src/shell");
    std::fs::create_dir_all(&shell).expect("create");
    std::fs::write(shell.join(name), contents).expect("write");
    dir
}

#[test]
fn jsx_text_that_says_words_is_counted() {
    assert_eq!(count("<p>Save everything</p>"), 1);
}

/// A second stretch in the same element is a second thing to translate.
#[test]
fn text_beside_an_expression_is_its_own_stretch() {
    assert_eq!(count("<p>for more than {minutes} min</p>"), 2);
}

#[test]
fn symbols_and_numbers_are_not_words() {
    assert_eq!(count("<p>—</p>"), 0);
    assert_eq!(count("<p>123</p>"), 0);
    assert_eq!(count(r#"<img alt="&times;" />"#), 0);
    assert_eq!(count("<p>1&nbsp;234</p>"), 0);
}

/// An entity wears letters (`&mdash;`) but says punctuation.
#[test]
fn an_entity_alone_is_not_words_but_text_around_it_is() {
    assert_eq!(count("<p>&mdash;</p>"), 0);
    assert_eq!(count("<p>Save&nbsp;now</p>"), 1);
}

#[test]
fn a_string_literal_as_a_child_is_counted_but_a_value_is_not() {
    assert_eq!(count("<p>{'Save everything'}</p>"), 1);
    assert_eq!(count("<p>{count}</p>"), 0);
    assert_eq!(count("<p>{a ? t('save') : t('load')}</p>"), 0);
}

/// Each branch of a choice is a sentence of its own, and it reaches the
/// screen as surely as a child does (`Preview.tsx`, `step ? … : 'Nothing yet'`).
#[test]
fn a_literal_branch_of_a_ternary_is_counted() {
    assert_eq!(count("<p>{a ? 'Save' : 'Load'}</p>"), 2);
    assert_eq!(
        count("const shown = step ? verbOf(step) : 'Nothing yet'"),
        1
    );
    assert_eq!(count("const shown = step ? 'Running now' : idle"), 1);
    assert_eq!(
        count("<span>{step ? verbOf(step.tool) : 'Nothing yet'}</span>"),
        1
    );
    assert_eq!(count("const a = x ? { kind: 'edit' } : 'Not shown'"), 1);
    assert_eq!(count("const a = x ? f(y, 'edit') : 'Not shown'"), 1);
}

/// The fallback of `??` is what shows when the value is missing.
#[test]
fn the_fallback_of_nullish_coalescing_is_counted() {
    assert_eq!(count("const said = answer.error ?? 'Not shown'"), 1);
    assert_eq!(count("<p>{answer.error ?? 'Not shown'}</p>"), 1);
}

/// A `label` in an object is a menu entry or a pill — words for the person.
#[test]
fn a_label_in_an_object_is_counted() {
    assert_eq!(
        count("const items = [{ id: 'open', label: 'Open card' }]"),
        1
    );
    assert_eq!(count("const one = { label: \"Move to doing\", run }"), 1);
    assert_eq!(
        count("const one = { title: 'Archive', placeholder: 'Search' }"),
        2
    );
}

/// What is not words for a person stays out: ids, catalog keys, CSS classes,
/// imports, optional chaining, optional members and the other keys of an
/// object.
#[test]
fn ids_keys_classes_and_imports_in_those_places_are_not_counted() {
    assert_eq!(count("const c = on ? 'isl-peek isl-peek--on' : ''"), 0);
    assert_eq!(count("const k = key ?? 'island.preview.nothingYet'"), 0);
    assert_eq!(
        count("const k = a ? t('island.preview.nothingYet') : t('b')"),
        0
    );
    assert_eq!(count("const k = a ? 'edit' : 'write'"), 0);
    assert_eq!(count("const k = a ? 'AgentChip' : 'TOOL_USE'"), 0);
    assert_eq!(count("import Preview from './Preview'"), 0);
    assert_eq!(
        count("const one = { id: 'open-card', kind: 'Edit', label: t('x') }"),
        0
    );
    assert_eq!(count("const a = b?.c ?? null"), 0);
    assert_eq!(count("type A = { label?: string; title?: 'x' }"), 0);
    assert_eq!(count("function f(a?: string, b?) {}"), 0);
    assert_eq!(count("switch (k) { case 'Open': return 1 }"), 0);
    assert_eq!(
        count("<path d={down ? 'M12 5v14M6 13l6 6 6-6' : 'M12 19V5'} />"),
        0
    );
}

/// The way out keeps working for the new places.
#[test]
fn an_excused_line_still_passes_with_the_new_places() {
    let markup = "const said = error ?? 'Not shown' // ui-text: ok shown only in logs\n\
                  const one = { label: 'Open card' } // ui-text: ok a test fixture\n\
                  const s = a ? 'Running now' : 'Idle now' // ui-text: ok a reason";
    assert_eq!(fixed_text_lines(markup).len(), 4);
    assert!(unmarked(markup).is_empty());
}

#[test]
fn words_across_lines_are_one_stretch_on_the_line_of_the_letter() {
    assert_eq!(fixed_text_lines("<p>\n  Save\n</p>"), vec![2]);
}

#[test]
fn attribute_literals_the_person_reads_are_counted() {
    for attribute in READ_ALOUD {
        assert_eq!(
            count(&format!("<input {attribute}=\"Search\" />")),
            1,
            "{attribute}"
        );
    }
}

/// `className`, `key` and `data-*` carry names, not words — the only text
/// here is the child.
#[test]
fn attributes_that_carry_no_words_are_not_counted() {
    assert_eq!(
        count(r#"<div className="chip" key="a" data-on="true">x</div>"#),
        1
    );
}

#[test]
fn an_attribute_literal_in_braces_is_counted_and_so_is_each_branch_of_a_choice() {
    assert_eq!(count("<input title={'Search'} />"), 1);
    assert_eq!(count("<input title={cond ? 'Search' : 'Replace'} />"), 2);
    assert_eq!(count("<input title={t('search')} />"), 0);
}

/// The tag `<label>` is not the attribute `label`; its child is what reads.
#[test]
fn a_label_tag_is_not_a_label_attribute() {
    assert_eq!(count("<label>Project</label>"), 1);
}

#[test]
fn the_first_argument_of_a_call_that_says_something_is_counted() {
    for call in SAYS {
        assert_eq!(count(&format!("{call}('Saved')")), 1, "{call}");
    }
    assert_eq!(count("notify('Done', { at: 1 })"), 1);
    assert_eq!(count("toast(\n  'Saved'\n)"), 1);
}

#[test]
fn a_said_call_passing_anything_else_is_not_counted() {
    assert_eq!(count("toast(t('saved'))"), 0);
    assert_eq!(count("toast(message)"), 0);
    assert_eq!(count("notify(`Done ${at}`)"), 0);
    assert_eq!(count("asked?.confirm()"), 0);
}

#[test]
fn typescript_strings_and_comments_are_not_screen_text() {
    assert_eq!(count("const said = 'Saved'; // Saved"), 0);
    assert_eq!(count("/* Saved */"), 0);
    assert_eq!(count("const said = `Saved`"), 0);
    assert_eq!(count("<p>{/* Saved */}</p>"), 0);
}

/// A regular expression can hold the angle brackets and quotes that would
/// open a tag, and one of these fooled the walk for real (`terminalLinks`).
#[test]
fn a_regular_expression_is_not_markup() {
    assert_eq!(count(r#"const web = /\bhttps?:\/\/[^\s<>"'`]+/gi"#), 0);
    assert_eq!(
        count(r#"const url = /https?:\/\/[^\s<>"'`]+[^\s<>"'`.,;:!?)\]}]/g"#),
        0
    );
    assert_eq!(count("const half = width / 2"), 0);
}

/// The classic foolers of a lexical scan: `Foo<Bar>` is a type, and the
/// arrow returns markup rather than comparing anything.
#[test]
fn type_arguments_are_not_markup() {
    assert_eq!(count("const a: Record<string, string> = {}"), 0);
    assert_eq!(count("const [a, setA] = useState<Person | null>(null)"), 0);
    assert_eq!(count("{items.map((one) => <li>{one.name}</li>)}"), 0);
}

/// One type argument straight inside another: the inner `<` follows the
/// outer one, not a name.
#[test]
fn nested_type_arguments_are_not_markup() {
    assert_eq!(count("const a: Map<string, Array<X>> = {}"), 0);
}

/// The generic arrow wears markup's opening: `<T,>` is the `.tsx` spelling
/// and `<T extends …>` the constraint, with the parameters behind the `>`.
#[test]
fn a_generic_arrow_is_not_markup() {
    assert_eq!(
        count("const waiting = <T,>(): Asked<T> => ({ data: null })"),
        0
    );
    assert_eq!(
        count(
            "const byPosition = <T extends { position: number }>(a: T, b: T): number => a.position"
        ),
        0
    );
    assert_eq!(count("const id = <T>(value: T): T => value"), 0);
}

/// And an element whose text happens to open with a parenthesis is still an
/// element — the desambiguation cannot eat real markup.
#[test]
fn markup_is_still_markup_beside_the_generic_arrow() {
    assert_eq!(count("const row = <li>Item the first</li>"), 1);
}

#[test]
fn an_arrow_can_return_markup() {
    assert_eq!(count("const row = () => <th>Project</th>"), 1);
}

#[test]
fn a_greater_than_inside_an_attribute_does_not_end_the_tag() {
    assert_eq!(
        count(r#"<input title={n > 0 ? "some" : "none"} placeholder="Search" />"#),
        1
    );
}

#[test]
fn a_line_excused_with_its_reason_is_not_counted() {
    let markup = "<p>Save</p> // ui-text: ok a proper noun\n<p>Load</p>";
    assert_eq!(unmarked(markup).len(), 1);
}

#[test]
fn a_file_inside_its_count_passes() {
    let dir = screen_tree("A.tsx", "<p>Save</p><p>Load</p>\n");
    let found = counts_in("web/src/shell/A.tsx 2\n", dir.path());
    assert!(found.is_empty(), "{}", as_text(&found));
}

/// The guard exists to catch exactly this: one more than there was.
#[test]
fn one_more_stretch_than_the_count_allows_fails() {
    let dir = screen_tree("A.tsx", "<p>Save</p><p>Load</p>\n");
    let found = counts_in("web/src/shell/A.tsx 1\n", dir.path());
    assert_eq!(found.len(), 1, "{}", as_text(&found));
    assert!(found[0].what.contains("2 stretch(es)"), "{}", found[0].what);
    assert_eq!(found[0].line, 1, "the finding points at the stretch");
}

#[test]
fn a_new_file_gets_no_count_at_all() {
    let dir = screen_tree("New.tsx", "<p>Save</p>\n");
    let found = counts_in("", dir.path());
    assert_eq!(found.len(), 1, "{}", as_text(&found));
    assert!(found[0].file.ends_with("New.tsx"));
}

#[test]
fn a_count_for_a_file_that_is_gone_is_reported() {
    let dir = screen_tree("A.tsx", "");
    let found = counts_in("web/src/shell/Gone.tsx 3\n", dir.path());
    assert_eq!(found.len(), 1, "{}", as_text(&found));
    assert!(found[0].what.contains("no file"), "{}", found[0].what);
}

/// Tests are not screens, generated files are not read, and the catalogs are
/// where the words come from, not where they are counted.
#[test]
fn tests_generated_files_and_catalogs_are_not_screens() {
    let dir = tempfile::tempdir().expect("tempdir");
    for (path, body) in [
        ("web/src/shell/A.test.tsx", "<p>Save</p>"),
        ("web/src/shell/A.test.ts", "toast('Save')"),
        ("web/src/i18n/en/common.json", "{\"a\": \"Save\"}"),
        ("web/src/gen/bindings.ts", "const a = 'Save'"),
        ("web/src/plugins/quiet.ts", "const a = 'Save'"),
    ] {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("dirs");
        std::fs::write(full, body).expect("body");
    }
    let found = counts_in("", dir.path());
    assert!(found.is_empty(), "{}", as_text(&found));
}

/// A count already on record is kept when the file now wants a larger one:
/// the command must not become the way around the guard.
#[test]
fn a_reseed_never_raises_a_count() {
    assert_eq!(next_count(Some(&1), 2), 1);
    assert_eq!(next_count(Some(&1), 1), 1);
}

/// That is what the translation slices are for: the number follows the work
/// down, and a finished file leaves the list.
#[test]
fn a_reseed_takes_the_credit_when_the_tree_says_less() {
    assert_eq!(next_count(Some(&3), 1), 1);
    assert_eq!(next_count(Some(&3), 0), 0);
}

#[test]
fn a_file_the_list_never_had_gets_nothing() {
    assert_eq!(next_count(None, 4), 0);
}

/// And the real tree passes, which is what makes the sabotages worth having.
#[test]
fn the_real_repository_passes() {
    let found = uncatalogued_screen_text_never_grows(&crate::workspace_root());
    assert!(found.is_empty(), "{}", as_text(&found));
}
