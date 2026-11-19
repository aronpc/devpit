//! What the tray says: its look, its count and its menu, from what waits on
//! the person.
//!
//! Pure, so a test can ask what the menu lists for a given state; `tray`
//! gathers that state and draws the answer.

use devpit_core::i18n::t;
use serde::{Deserialize, Serialize};

/// The most the menu lists of what waits; the rest are a click away in devpit.
pub(crate) const MOST: usize = 5;
/// The most recent projects the menu offers.
pub(crate) const RECENT: usize = 5;
/// The longest a line of the menu runs before it is cut, in characters.
pub(crate) const LONGEST: usize = 48;

/// Something waiting on the person.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Waiting {
    /// A session stopped on them: a question, a permission, a choice.
    Session {
        session_id: String,
        /// Its card, else its project, as the island names it.
        who: String,
        project_id: Option<String>,
        pane_id: Option<String>,
    },
    /// A reply an orchestrator drafted, for them to send.
    Draft { profile: String, session: String },
    /// A card's reminder went off and nobody has dealt with it.
    Due {
        project_id: String,
        card_id: String,
        title: String,
    },
}

/// A project the menu can open.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Recent {
    pub id: String,
    pub name: String,
}

/// Where a click in the menu takes devpit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "to", rename_all = "snake_case")]
pub(crate) enum Go {
    /// A session in one of devpit's terminals.
    Pane {
        project_id: Option<String>,
        pane_id: String,
    },
    /// A session with no terminal: a chat's.
    Chat {
        session_id: String,
    },
    /// A card, for its reminder.
    Card {
        project_id: String,
        card_id: String,
    },
    /// An orchestrator's project, where its drafts wait beside their sessions.
    Drafts {
        profile: String,
    },
    Project {
        project_id: String,
    },
}

const GO: &str = "go:";

impl Go {
    /// The menu item's id. The whole target rather than a position in a list,
    /// so a click on a menu built a moment ago still goes where it said.
    pub(crate) fn id(&self) -> String {
        format!("{GO}{}", serde_json::to_string(self).unwrap_or_default())
    }

    pub(crate) fn from_id(id: &str) -> Option<Go> {
        serde_json::from_str(id.strip_prefix(GO)?).ok()
    }
}

/// The menu's own choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Act {
    Open,
    Terminal,
    Pause,
    Resume,
    Settings,
    /// The same app in a browser, on loopback.
    Browser,
    Quit,
}

const ACTS: [(Act, &str); 7] = [
    (Act::Open, "open"),
    (Act::Terminal, "terminal"),
    (Act::Pause, "pause"),
    (Act::Resume, "resume"),
    (Act::Settings, "settings"),
    (Act::Browser, "browser"),
    (Act::Quit, "quit"),
];

impl Act {
    pub(crate) fn id(self) -> &'static str {
        ACTS.iter()
            .find(|(act, _)| *act == self)
            .map_or("", |(_, id)| id)
    }

    pub(crate) fn from_id(id: &str) -> Option<Act> {
        ACTS.iter()
            .find(|(_, known)| *known == id)
            .map(|(act, _)| *act)
    }
}

/// How the icon looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Look {
    Plain,
    /// A dot: something waits on the person.
    Waiting,
    /// Faded: devpit is paused.
    Paused,
}

/// One line of the menu.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Entry {
    /// Words, not a choice.
    Heading(String),
    Go {
        label: String,
        go: Go,
    },
    Act {
        label: String,
        act: Act,
    },
    /// A submenu of projects, by name.
    Projects(Vec<(String, Go)>),
    Line,
}

/// Everything the tray shows.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Said {
    pub look: Look,
    /// The count beside the icon, where the desktop draws one.
    pub title: Option<String>,
    pub tooltip: String,
    pub entries: Vec<Entry>,
}

/// What the tray shows while `waiting` waits, `recent` are the projects most
/// recently opened, and devpit is `paused` or not.
pub(crate) fn said(waiting: &[Waiting], recent: &[Recent], paused: bool) -> Said {
    let count = waiting.len();
    let look = if paused {
        Look::Paused
    } else if count > 0 {
        Look::Waiting
    } else {
        Look::Plain
    };
    let tooltip = match (paused, count) {
        (true, _) => t("tray.tooltipPaused", &[]),
        (false, 0) => "devpit".to_owned(),
        (false, count) => t("tray.tooltipWaiting", &[("count", &count.to_string())]),
    };

    let mut entries = Vec::new();
    if count > 0 {
        entries.push(Entry::Heading(t(
            "tray.waitingHeading",
            &[("count", &count.to_string())],
        )));
        entries.extend(waiting.iter().take(MOST).map(|one| Entry::Go {
            label: cut(&label(one)),
            go: go(one),
        }));
        if count > MOST {
            entries.push(Entry::Act {
                label: t("tray.more", &[("count", &(count - MOST).to_string())]),
                act: Act::Open,
            });
        }
        entries.push(Entry::Line);
    }
    entries.push(act(&t("tray.open", &[]), Act::Open));
    if !recent.is_empty() {
        entries.push(act(&t("tray.newTerminal", &[]), Act::Terminal));
        entries.push(Entry::Projects(
            recent
                .iter()
                .take(RECENT)
                .map(|one| {
                    (
                        cut(&one.name),
                        Go::Project {
                            project_id: one.id.clone(),
                        },
                    )
                })
                .collect(),
        ));
    }
    entries.push(Entry::Line);
    entries.push(if paused {
        act(&t("tray.resume", &[]), Act::Resume)
    } else {
        act(&t("tray.pause", &[]), Act::Pause)
    });
    entries.push(act(&t("tray.settings", &[]), Act::Settings));
    entries.push(act(&t("tray.browser", &[]), Act::Browser));
    entries.push(Entry::Line);
    entries.push(act(&t("tray.quit", &[]), Act::Quit));

    Said {
        look,
        title: (count > 0).then(|| count.to_string()),
        tooltip,
        entries,
    }
}

fn act(label: &str, act: Act) -> Entry {
    Entry::Act {
        label: label.to_owned(),
        act,
    }
}

fn label(waiting: &Waiting) -> String {
    match waiting {
        Waiting::Session { who, .. } => who.clone(),
        Waiting::Draft { session, .. } => t("tray.draft", &[("session", session)]),
        Waiting::Due { title, .. } => t("tray.due", &[("title", title)]),
    }
}

/// Where a waiting thing is dealt with.
fn go(waiting: &Waiting) -> Go {
    match waiting {
        Waiting::Session {
            pane_id: Some(pane_id),
            project_id,
            ..
        } => Go::Pane {
            project_id: project_id.clone(),
            pane_id: pane_id.clone(),
        },
        Waiting::Session { session_id, .. } => Go::Chat {
            session_id: session_id.clone(),
        },
        Waiting::Draft { profile, .. } => Go::Drafts {
            profile: profile.clone(),
        },
        Waiting::Due {
            project_id,
            card_id,
            ..
        } => Go::Card {
            project_id: project_id.clone(),
            card_id: card_id.clone(),
        },
    }
}

/// One line of a menu: a title can be a paragraph, and a menu is a column.
fn cut(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match text.char_indices().nth(LONGEST) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text,
    }
}

#[cfg(test)]
#[path = "tray_menu_tests.rs"]
mod tests;
