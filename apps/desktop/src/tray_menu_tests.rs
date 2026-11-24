use devpit_core::locale::Locale;

use super::{said, Act, Entry, Go, Look, Recent, Waiting, LONGEST, MOST, RECENT};

/// Every label the menu offers, in order.
fn labels(entries: &[Entry]) -> Vec<&str> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Act { label, .. } | Entry::Go { label, .. } | Entry::Heading(label) => {
                Some(label.as_str())
            }
            _ => None,
        })
        .collect()
}

fn session(n: usize, pane: Option<&str>) -> Waiting {
    Waiting::Session {
        session_id: format!("s{n}"),
        who: format!("Card {n}"),
        project_id: Some("p1".to_owned()),
        pane_id: pane.map(str::to_owned),
    }
}

fn project(n: usize) -> Recent {
    Recent {
        id: format!("p{n}"),
        name: format!("Project {n}"),
    }
}

fn acts(entries: &[Entry]) -> Vec<Act> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Act { act, .. } => Some(*act),
            _ => None,
        })
        .collect()
}

fn gos(entries: &[Entry]) -> Vec<(&str, &Go)> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Go { label, go } => Some((label.as_str(), go)),
            _ => None,
        })
        .collect()
}

/// With nothing waiting the icon is plain, carries no count, and the menu
/// opens on devpit itself rather than on an empty heading.
#[test]
fn nothing_waiting_is_a_plain_icon_and_no_waiting_section() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let shown = said(&[], &[], false);
    assert_eq!(shown.look, Look::Plain);
    assert_eq!(shown.title, None);
    assert_eq!(shown.tooltip, "devpit");
    assert_eq!(
        shown.entries.first(),
        Some(&Entry::Act {
            label: "Open devpit".to_owned(),
            act: Act::Open
        })
    );
    assert!(!shown
        .entries
        .iter()
        .any(|entry| matches!(entry, Entry::Heading(_))));
}

/// Everything waiting is counted, but the menu lists only the first few and
/// says how many more devpit holds.
#[test]
fn the_menu_lists_the_first_few_waiting_and_counts_them_all() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let waiting: Vec<Waiting> = (0..MOST + 2).map(|n| session(n, Some("%1"))).collect();
    let shown = said(&waiting, &[], false);

    assert_eq!(shown.look, Look::Waiting);
    assert_eq!(shown.title.as_deref(), Some("7"));
    assert_eq!(
        shown.entries.first(),
        Some(&Entry::Heading("Waiting on you (7)".to_owned()))
    );
    let listed = gos(&shown.entries);
    assert_eq!(listed.len(), MOST);
    assert_eq!(listed[0].0, "Card 0");
    assert!(shown.entries.contains(&Entry::Act {
        label: "2 more in devpit".to_owned(),
        act: Act::Open
    }));
}

/// Each kind of waiting opens where it is dealt with: a terminal's session at
/// its pane, a chat's by its session, a draft at its orchestrator, a
/// reminder at its card.
#[test]
fn each_waiting_thing_goes_where_it_is_dealt_with() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let waiting = [
        session(1, Some("%7")),
        session(2, None),
        Waiting::Draft {
            profile: "claude".to_owned(),
            session: "api-fix".to_owned(),
        },
        Waiting::Due {
            project_id: "p2".to_owned(),
            card_id: "c9".to_owned(),
            title: "Ship the release".to_owned(),
        },
    ];
    let shown = said(&waiting, &[], false);
    let listed = gos(&shown.entries);
    assert_eq!(
        listed,
        vec![
            (
                "Card 1",
                &Go::Pane {
                    project_id: Some("p1".to_owned()),
                    pane_id: "%7".to_owned()
                }
            ),
            (
                "Card 2",
                &Go::Chat {
                    session_id: "s2".to_owned()
                }
            ),
            (
                "Send the draft for api-fix",
                &Go::Drafts {
                    profile: "claude".to_owned()
                }
            ),
            (
                "Due: Ship the release",
                &Go::Card {
                    project_id: "p2".to_owned(),
                    card_id: "c9".to_owned()
                }
            ),
        ]
    );
}

/// Paused wins the look, Resume replaces Pause, and what waits is still
/// counted: the pause quiets devpit, it does not make the work go away.
#[test]
fn a_pause_fades_the_icon_and_offers_resume() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let shown = said(&[session(1, None)], &[], true);
    assert_eq!(shown.look, Look::Paused);
    assert_eq!(shown.title.as_deref(), Some("1"));
    assert_eq!(shown.tooltip, "devpit — paused");
    let offered = acts(&shown.entries);
    assert!(offered.contains(&Act::Resume));
    assert!(!offered.contains(&Act::Pause));

    let running = acts(&said(&[], &[], false).entries);
    assert!(running.contains(&Act::Pause));
    assert!(!running.contains(&Act::Resume));
}

/// Recent projects and a new terminal only when there is a project to put
/// them in; Settings and Quit always.
#[test]
fn projects_and_a_new_terminal_only_with_a_project() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let none = said(&[], &[], false);
    assert!(!acts(&none.entries).contains(&Act::Terminal));
    assert!(!none
        .entries
        .iter()
        .any(|entry| matches!(entry, Entry::Projects(_))));
    for act in [Act::Settings, Act::Quit] {
        assert!(acts(&none.entries).contains(&act), "{act:?}");
    }

    let recent: Vec<Recent> = (0..RECENT + 3).map(project).collect();
    let some = said(&[], &recent, false);
    assert!(acts(&some.entries).contains(&Act::Terminal));
    let Some(Entry::Projects(listed)) = some
        .entries
        .iter()
        .find(|entry| matches!(entry, Entry::Projects(_)))
    else {
        panic!("no projects submenu");
    };
    assert_eq!(listed.len(), RECENT);
    assert_eq!(
        listed[0],
        (
            "Project 0".to_owned(),
            Go::Project {
                project_id: "p0".to_owned()
            }
        )
    );
}

/// Quit is the last line, so it is never the item under a hurried click.
#[test]
fn quit_is_last() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let shown = said(&[session(1, None)], &[project(1)], false);
    assert_eq!(
        shown.entries.last(),
        Some(&Entry::Act {
            label: "Quit devpit".to_owned(),
            act: Act::Quit
        })
    );
}

/// A long title is one short line in the menu.
#[test]
fn a_long_title_is_cut_to_one_line() {
    let _turn = crate::locale_turn::settled(Locale::En);
    let waiting = [Waiting::Due {
        project_id: "p".to_owned(),
        card_id: "c".to_owned(),
        title: "A title\nthat runs on and on well past the width any menu should be".to_owned(),
    }];
    let shown = said(&waiting, &[], false);
    let (label, _) = gos(&shown.entries)[0];
    assert!(label.ends_with('…'), "{label}");
    assert!(!label.contains('\n'), "{label}");
    // The cut, and the ellipsis after it.
    assert_eq!(label.chars().count(), LONGEST + 1, "{label}");
}

/// A menu id carries the whole target and reads back as it went in; an id
/// that is not one of these reads as nothing.
#[test]
fn a_menu_id_reads_back_as_its_target() {
    for go in [
        Go::Pane {
            project_id: None,
            pane_id: "%3".to_owned(),
        },
        Go::Card {
            project_id: "p:1".to_owned(),
            card_id: "c\"2".to_owned(),
        },
        Go::Project {
            project_id: "p".to_owned(),
        },
    ] {
        assert_eq!(Go::from_id(&go.id()), Some(go));
    }
    for act in [
        Act::Open,
        Act::Terminal,
        Act::Pause,
        Act::Resume,
        Act::Settings,
        Act::Browser,
        Act::Quit,
    ] {
        assert_eq!(Act::from_id(act.id()), Some(act));
        assert_eq!(Go::from_id(act.id()), None);
    }
    assert_eq!(Go::from_id("go:{not json"), None);
}

/// The menu and the tooltip are the person's words: every label comes from
/// the catalog, in whichever language the process settled on. English says
/// what it always said; Portuguese says it in Portuguese.
#[test]
fn the_menu_and_tooltip_are_said_in_the_persons_language() {
    let waiting = [session(1, None), session(2, None)];
    let recent = [project(0)];

    {
        let _turn = crate::locale_turn::settled(Locale::PtBr);
        let shown = said(&waiting, &recent, false);
        assert_eq!(shown.tooltip, "devpit — 2 esperando você");
        assert_eq!(
            shown.entries.first(),
            Some(&Entry::Heading("Esperando você (2)".to_owned()))
        );
        let offered = labels(&shown.entries);
        for label in [
            "Abrir o devpit",
            "Novo terminal",
            "Pausar por uma hora",
            "Configurações…",
            "Abrir no navegador",
            "Sair do devpit",
        ] {
            assert!(offered.contains(&label), "{label} missing: {offered:?}");
        }
        let paused = said(&[session(1, None)], &[], true);
        assert_eq!(paused.tooltip, "devpit — pausado");
        assert!(labels(&paused.entries).contains(&"Retomar"));

        let more: Vec<Waiting> = (0..MOST + 1).map(|n| session(n, None)).collect();
        assert!(labels(&said(&more, &[], false).entries).contains(&"Mais 1 no devpit"));
        let draft = [Waiting::Draft {
            profile: "claude".to_owned(),
            session: "api-fix".to_owned(),
        }];
        assert!(
            labels(&said(&draft, &[], false).entries).contains(&"Enviar o rascunho para api-fix")
        );
        let due = [Waiting::Due {
            project_id: "p".to_owned(),
            card_id: "c".to_owned(),
            title: "Ship the release".to_owned(),
        }];
        assert!(labels(&said(&due, &[], false).entries).contains(&"Vencido: Ship the release"));
    }

    {
        let _turn = crate::locale_turn::settled(Locale::En);
        let shown = said(&waiting, &recent, false);
        assert_eq!(shown.tooltip, "devpit — 2 waiting on you");
        let offered = labels(&shown.entries);
        for label in [
            "Waiting on you (2)",
            "Open devpit",
            "New terminal",
            "Pause for an hour",
            "Settings…",
            "Open in browser",
            "Quit devpit",
        ] {
            assert!(offered.contains(&label), "{label} missing: {offered:?}");
        }
    }
}
