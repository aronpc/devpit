//! devpit's words for the devflow engine: a card where the engine says item,
//! the `devpit_devflow_*` tools, `DEVPIT_*` variables, and `.devpit/devflow/`
//! as every project's folder. The flow's texts are devflow's own, said in the
//! words devflow ships for devpit.

use std::sync::LazyLock;

use devflow_engine::dialect::{Assets, Tools};
use devflow_engine::skills::{self, Words};
use devflow_engine::Dialect;
use devpit_core::i18n::t_for;
use devpit_core::locale::Locale;

/// devflow's skills, their parts and its publish templates in devpit's words;
/// the personas, rubrics and schema as devflow ships them.
fn said_for_devpit() -> &'static Assets {
    let words = Words::shipped("devpit").unwrap_or_default();
    let say =
        |text: &str| -> &'static str { Box::leak(skills::rendered(text, &words).into_boxed_str()) };
    let shipped = Assets::DEVFLOW;
    let parts: Vec<(&'static str, &'static str, &'static str)> = shipped
        .parts
        .iter()
        .map(|(skill, part, text)| (*skill, *part, say(text)))
        .collect();
    Box::leak(Box::new(Assets {
        skills: shipped
            .skills
            .map(|(name, stage, text)| (name, stage, say(text))),
        parts: Box::leak(parts.into_boxed_slice()),
        publish: shipped.publish.map(say),
        ..shipped
    }))
}

/// devpit's, as the engine's golden test pins it.
pub(crate) static DEVPIT: LazyLock<Dialect> = LazyLock::new(|| Dialect {
    product: "devpit",
    version: env!("CARGO_PKG_VERSION"),
    env: "DEVPIT",
    folder: devflow_engine::folder::LEGACY,
    item: "card",
    items: "cards",
    an_item: "a card",
    work_item: "card",
    a_work_item: "a card",
    item_field: "card",
    tools: Tools {
        evidence: "devpit_devflow_evidence",
        gate: "devpit_devflow_gate",
        set: "devpit_devflow_set",
        stage: "devpit_devflow_stage",
        review: "devpit_devflow_review",
        publish: "devpit_devflow_publish",
        handoff: "devpit_handoff",
        finish: "devpit_finish_card",
    },
    files_label: "Attached",
    files_more: "among the artifacts",
    packet_heading: "Card",
    plan_board: "cards are made on the devpit board.",
    plan_outside: "is not another card of this plan, nor a card on the board (`card_…` or `#42`)",
    yaml_header: "# Devflow: briefing → context → plan → execution → review → completion,\n\
                  # served by devpit's MCP. This file is the source of truth: devpit edits\n\
                  # it and never commits it.\n",
    yaml_enabled: "# Switched on by the person's click in the devpit app, never by an agent.\n",
    yaml_enforce: "waits for the person's click in the app",
    yaml_provider: "the provider in Settings → Decisions",
    decisions_opted_out: "this project's state is not sent to Decisions — see Settings → Decisions",
    decisions_no_key: "no key — set one in Settings → Decisions",
    assets: said_for_devpit(),
});

/// The dialect a session reads in its own language. Only what the engine says
/// to the agent in sentences moves: the card's words, the tools, the
/// variables and the folder are the engine's vocabulary and stay literal, and
/// so do the skills, the plugin and the `devflow.yaml` header, which go into
/// the person's repository and are shared with a team: they say the same
/// thing whoever installed them. In English it is [`DEVPIT`] itself.
pub(crate) fn dialect_in(locale: Locale) -> &'static Dialect {
    static PORTUGUESE: LazyLock<Dialect> = LazyLock::new(|| {
        let say = |key: &str| -> &'static str {
            Box::leak(t_for(Locale::PtBr, key, &[]).into_boxed_str())
        };
        Dialect {
            files_label: say("devflow.filesLabel"),
            files_more: say("devflow.filesMore"),
            packet_heading: say("devflow.packetHeading"),
            plan_board: say("devflow.planBoard"),
            plan_outside: say("devflow.planOutside"),
            decisions_opted_out: say("devflow.decisionsOptedOut"),
            decisions_no_key: say("devflow.decisionsNoKey"),
            ..*DEVPIT
        }
    });
    match locale {
        Locale::En => &DEVPIT,
        Locale::PtBr => &PORTUGUESE,
    }
}

/// devpit's words as `devflow host` takes them at `initialize`: the ones
/// devflow ships for it.
pub(crate) fn host_words() -> serde_json::Value {
    serde_json::json!({ "host": "devpit" })
}

/// Pins `.devpit/devflow/` and devpit's way of starting programs, once,
/// before the engine reads anything. Later calls do nothing.
pub(crate) fn pinned() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = devflow_engine::folder::pin(devflow_engine::folder::LEGACY);
        let _ = devflow_engine::env::spawn_with(|program| devpit_pty::host_env::command(program));
    });
}

/// `.devpit/devflow/devflow.yaml`, relative to a project's root.
pub(crate) fn file() -> String {
    pinned();
    devflow_engine::folder::default_file()
}

/// The layout every project's folder is written in.
pub(crate) fn layout() -> devflow_engine::folder::Layout {
    pinned();
    devflow_engine::folder::Layout {
        dialect: &DEVPIT,
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "devflow_dialect_tests.rs"]
mod tests;
