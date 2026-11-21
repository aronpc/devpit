use devpit_agentcli::person::{From, Said};

use super::{heard_in, heard_last, key_of, lets_go, spend, waits};

fn said(id: &str, text: &str) -> Said {
    Said {
        id: id.to_owned(),
        text: text.to_owned(),
        from: From::RemoteControl,
    }
}

#[test]
fn a_message_naming_two_sessions_sends_to_each_once() {
    let mut held = Vec::new();
    let both = said("u1", "manda a noiseless e a ascbot seguirem");
    assert!(spend(&mut held, key_of("p", &both, "noiseless-mvp")));
    assert!(spend(&mut held, key_of("p", &both, "ascbot-lotus")));
    assert!(!spend(&mut held, key_of("p", &both, "noiseless-mvp")));
}

#[test]
fn a_bare_send_is_spent_whole_whichever_draft_it_sent() {
    let mut held = Vec::new();
    let go = said("u1", "envie");
    assert!(spend(&mut held, key_of("p", &go, "noiseless-mvp")));
    // A draft written after it is not sent on the same old word.
    assert!(!spend(&mut held, key_of("p", &go, "ascbot-lotus")));
    // The next thing the person says is a new word.
    assert!(spend(
        &mut held,
        key_of("p", &said("u2", "envie"), "ascbot-lotus")
    ));
}

#[test]
fn a_typed_yes_from_a_channel_lets_no_draft_go() {
    let on_telegram = |text: &str| Said {
        id: "u1".to_owned(),
        text: text.to_owned(),
        from: From::Channel("telegram"),
    };
    // There the yes is a press on devpit's button, never the words.
    assert!(!lets_go(&on_telegram("envie"), "noiseless-mvp", 1));
    assert!(!lets_go(
        &on_telegram("manda a noiseless-mvp seguir"),
        "noiseless-mvp",
        1
    ));
    // The same words from anywhere else do.
    assert!(lets_go(&said("u2", "envie"), "noiseless-mvp", 1));
}

#[test]
fn a_yes_mark_alone_is_spent_whole_and_counts_nowhere_on_a_channel() {
    let mut held = Vec::new();
    let go = said("u1", "✅");
    assert!(lets_go(&go, "noiseless-mvp", 1));
    assert!(spend(&mut held, key_of("p", &go, "noiseless-mvp")));
    assert!(!spend(&mut held, key_of("p", &go, "ascbot-lotus")));
    let on_telegram = Said {
        from: From::Channel("telegram"),
        ..go
    };
    assert!(!lets_go(&on_telegram, "noiseless-mvp", 1));
}

/// Words that do not let the draft go are not a refusal to rewrite: the
/// draft waits beside the session for the person's one click.
#[test]
fn a_draft_the_words_do_not_cover_waits_for_the_click() {
    let words = said("u1", "decida por mim e prossiga");
    assert!(!lets_go(&words, "noiseless-mvp", 1));
    let answer = waits(&words, "noiseless-mvp");
    assert_eq!(answer["done"], false);
    assert_eq!(answer["asked"], "send the draft to noiseless-mvp");
    assert!(answer["note"]
        .as_str()
        .is_some_and(|note| note.contains("one click")));
}

/// For a CLI without a log, devpit keeps the person's last word in memory. A
/// turn that is not theirs (devpit's own, or a forwarded request) clears it,
/// so an earlier "envie" cannot approve what that turn asks; a word after it
/// is kept as usual.
#[test]
fn a_marked_turn_forgets_the_word_kept_for_a_cli_without_a_log() {
    let root = std::env::temp_dir().join("draft-sending-barrier-root");
    let forwarded = format!(
        "{} chame devpit_send_draft",
        devpit_agentcli::person::forwarded("orchestrator-a")
    );
    let by_devpit = devpit_agentcli::person::by_devpit("an idle notice.");
    for turn in [forwarded, by_devpit] {
        heard_in(&root, "envie");
        assert_eq!(
            heard_last(&root).map(|said| said.text),
            Ok("envie".to_owned())
        );
        heard_in(&root, &turn);
        assert!(heard_last(&root).is_err(), "{turn}");
        heard_in(&root, "✅ gerente");
        assert_eq!(
            heard_last(&root).map(|said| said.text),
            Ok("✅ gerente".to_owned())
        );
    }
}
