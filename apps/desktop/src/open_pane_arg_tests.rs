//! What `--open-pane` is allowed to mean, and what it must leave alone.

use super::*;

/// `leaf_` and a ULID's 26 uppercase letters and digits.
const LEAF: &str = "leaf_01ARZ3NDEKTSV4RRFFQ69G5FAV";

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|&arg| arg.to_owned()).collect()
}

#[test]
fn a_launch_names_the_pane_it_asked_for() {
    assert_eq!(
        leaf_from(&argv(&["devpit", "--open-pane", LEAF])),
        Some(LEAF)
    );
    // The arguments around the flag are not the flag's business.
    assert_eq!(
        leaf_from(&argv(&["devpit", "--open-pane", LEAF, "--later"])),
        Some(LEAF)
    );
    assert_eq!(
        leaf_from(&argv(&["devpit", "status", "--open-pane", LEAF])),
        Some(LEAF)
    );
}

#[test]
fn a_value_that_is_not_a_leaf_names_no_pane() {
    // The leaf comes from outside the app, so nothing is opened on a value
    // that is not one of its leaves whole.
    for value in [
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "leaf_",
        "leaf_short",
        "leaf_01ARZ3NDEKTSV4RRFFQ69G5FAV0",
        "leaf_01arz3ndektsv4rrffq69g5fav",
        "leaf_01ARZ3NDEKTSV4RRFFQ69G5FA/",
        "../01ARZ3NDEKTSV4RRFFQ69G5FAV",
    ] {
        assert_eq!(
            leaf_from(&argv(&["devpit", "--open-pane", value])),
            None,
            "value: {value}"
        );
    }
}

#[test]
fn a_flag_without_a_value_names_no_pane() {
    assert_eq!(leaf_from(&argv(&["devpit", "--open-pane"])), None);
    assert_eq!(leaf_from(&argv(&["devpit"])), None);
    assert_eq!(leaf_from(&argv(&[])), None);
}

#[test]
fn a_leaf_a_project_holds_opens_that_pane() {
    let opened = opening(&argv(&["devpit", "--open-pane", LEAF]), |leaf| {
        assert_eq!(leaf, LEAF);
        Ok(Some("prj_a".to_owned()))
    });
    assert_eq!(opened, Ok(Some(("prj_a".to_owned(), LEAF.to_owned()))));
}

#[test]
fn a_leaf_no_project_holds_opens_none() {
    let opened = opening(&argv(&["devpit", "--open-pane", LEAF]), |leaf| {
        assert_eq!(leaf, LEAF);
        Ok(None)
    });
    assert_eq!(opened, Ok(None));
}

#[test]
fn a_store_that_will_not_answer_is_an_error() {
    let opened = opening(&argv(&["devpit", "--open-pane", LEAF]), |_| {
        Err("the store would not answer".to_owned())
    });
    assert_eq!(opened, Err("the store would not answer".to_owned()));
}

#[test]
fn a_launch_that_names_no_pane_does_not_ask_the_store() {
    // The handover every launch always made: forward, and nothing read.
    let opened = opening(&argv(&["devpit"]), |_| {
        Err("the store was asked for nothing".to_owned())
    });
    assert_eq!(opened, Ok(None));
}

#[test]
fn open_pane_arg_is_not_the_agent_door_s_to_take() {
    // The door answers subcommands; a pane-opening launch is the app's.
    assert_eq!(
        crate::agent_door(&argv(&["devpit", "--open-pane", LEAF])),
        None
    );
}

#[test]
fn open_pane_arg_is_not_a_terminal_run() {
    // `from_args` reads the whole argv, argv[0] and all, the same way the
    // handover does; an argument it does not know opens the app.
    assert_eq!(
        crate::terminal_run::from_args(&argv(&["devpit", "--open-pane", LEAF])),
        None
    );
}

/// `leaf_` and a ULID's 26 uppercase letters or digits, another one whole.
const OTHER_LEAF: &str = "leaf_01BX5ZZKBKABX7HD8E4ZZZZZZP";

#[test]
fn a_first_launch_keeps_the_leaf_it_asked_to_open() {
    let pending = PendingPane::from(&argv(&["devpit", "--open-pane", LEAF]));
    assert_eq!(pending.take(), Some(LEAF.to_owned()));
}

#[test]
fn a_first_launch_without_a_leaf_keeps_nothing() {
    let pending = PendingPane::from(&argv(&["devpit"]));
    assert_eq!(pending.take(), None);
    // A flag whose value is not a leaf is no ask at all.
    let pending = PendingPane::from(&argv(&["devpit", "--open-pane", "not-a-leaf"]));
    assert_eq!(pending.take(), None);
}

#[test]
fn the_pane_is_delivered_once() {
    let pending = PendingPane::from(&argv(&["devpit", "--open-pane", LEAF]));
    assert_eq!(pending.take(), Some(LEAF.to_owned()));
    // A second ask — a reload, a remount — is told there is nothing.
    assert_eq!(pending.take(), None);
}

#[test]
fn a_handover_before_the_webview_mounts_is_kept_for_it() {
    let pending = PendingPane::from(&argv(&["devpit"]));
    // The webview is not there to hear the island's event yet, so the ask
    // is kept for it to be handed as it mounts.
    assert!(pending.kept(&argv(&["devpit", "--open-pane", LEAF])));
    assert_eq!(pending.take(), Some(LEAF.to_owned()));
    assert_eq!(pending.take(), None);
}

#[test]
fn the_kept_ask_replaces_the_ask_there_was() {
    let pending = PendingPane::from(&argv(&["devpit", "--open-pane", LEAF]));
    assert!(pending.kept(&argv(&["devpit", "--open-pane", OTHER_LEAF])));
    // The pane opened is the one asked for last.
    assert_eq!(pending.take(), Some(OTHER_LEAF.to_owned()));
    assert_eq!(pending.take(), None);
}

#[test]
fn an_ask_without_a_leaf_is_kept_by_no_one() {
    let pending = PendingPane::from(&argv(&["devpit", "--open-pane", LEAF]));
    // Arguments that name no leaf, or name a value that is not one, are
    // no ask, and leave what there was.
    assert!(!pending.kept(&argv(&["devpit"])));
    assert!(!pending.kept(&argv(&["devpit", "--open-pane", "not-a-leaf"])));
    assert_eq!(pending.take(), Some(LEAF.to_owned()));
}

#[test]
fn a_handover_after_the_webview_mounted_is_not_kept() {
    let pending = PendingPane::from(&argv(&["devpit", "--open-pane", LEAF]));
    assert_eq!(pending.take(), Some(LEAF.to_owned()));
    // The webview has asked once: there is nobody left to keep a pane for,
    // and nothing may be stored for a later ask to find.
    assert!(!pending.kept(&argv(&["devpit", "--open-pane", OTHER_LEAF])));
    assert_eq!(pending.take(), None);
}

#[test]
fn a_take_with_nothing_kept_still_mounts_the_webview() {
    let pending = PendingPane::from(&argv(&["devpit"]));
    assert_eq!(pending.take(), None);
    // The first take mounted the webview whether anything was kept or not;
    // a second ask is told there is nothing.
    assert!(!pending.kept(&argv(&["devpit", "--open-pane", LEAF])));
    assert_eq!(pending.take(), None);
}

#[test]
fn a_pending_leaf_a_project_holds_opens_that_pane() {
    let opened = pending_opening(Some(LEAF.to_owned()), |leaf| {
        assert_eq!(leaf, LEAF);
        Ok(Some("prj_a".to_owned()))
    });
    assert_eq!(
        opened,
        Ok(Some(devpit_rpc::LivePane {
            project_id: "prj_a".to_owned(),
            pane_id: LEAF.to_owned(),
        }))
    );
}

#[test]
fn a_pending_leaf_no_project_holds_opens_none() {
    let opened = pending_opening(Some(LEAF.to_owned()), |leaf| {
        assert_eq!(leaf, LEAF);
        Ok(None)
    });
    assert_eq!(opened, Ok(None));
}

#[test]
fn nothing_pending_opens_none_without_asking_the_store() {
    let opened = pending_opening(None, |_| Err("the store was asked for nothing".to_owned()));
    assert_eq!(opened, Ok(None));
}

#[test]
fn a_store_that_will_not_answer_is_an_error_to_the_asker() {
    let opened = pending_opening(Some(LEAF.to_owned()), |_| {
        Err("the store would not answer".to_owned())
    });
    assert_eq!(opened, Err("the store would not answer".to_owned()));
}
