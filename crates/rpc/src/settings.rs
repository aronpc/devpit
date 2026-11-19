//! What the person chose, and what this build can offer them.
//!
//! One object rather than a command per switch: the first run reads all of it
//! once and writes what changed, and a screen that has to make four round
//! trips to draw itself is a screen that draws itself four times.

use devpit_core::locale::Locale;
use serde::{Deserialize, Serialize};
use specta::Type;

/// The appearance the window uses.
///
/// An enum and not a string, so a build that does not ship a theme cannot be
/// asked for one. `System` is a third setting, not a third palette: it borrows
/// whichever of the two the machine is already using.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    System,
    Light,
    #[default]
    Dark,
}

impl Theme {
    /// What a stored value means. An unknown name reads as the default rather
    /// than failing the whole settings read for one bad row.
    pub fn parse(stored: &str) -> Self {
        match stored {
            "system" => Self::System,
            "light" => Self::Light,
            _ => Self::Dark,
        }
    }

    pub fn stored(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// The language the person wants devpit's own words in.
///
/// `System` is not a language: it is the choice to take whichever language
/// the machine is already speaking. Stored and spoken in the same short
/// names, so what the store holds and what the interface sends are one
/// vocabulary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Language {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "pt_br")]
    PortugueseBrazil,
}

impl Language {
    /// What a stored value means. An unknown name reads as `System` rather
    /// than failing the whole settings read for one bad row — and as `System`
    /// rather than English, because a row nobody can read is not a choice.
    pub fn parse(stored: &str) -> Self {
        match stored {
            "en" => Self::English,
            "pt_br" => Self::PortugueseBrazil,
            _ => Self::System,
        }
    }

    pub fn stored(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::English => "en",
            Self::PortugueseBrazil => "pt_br",
        }
    }

    /// The locale the choice names outright. `System` names none: the machine
    /// is asked instead, by [`Locale::resolve`].
    pub fn locale(self) -> Option<Locale> {
        match self {
            Self::System => None,
            Self::English => Some(Locale::En),
            Self::PortugueseBrazil => Some(Locale::PtBr),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    /// The language the person chose for devpit's own words. `System` until
    /// they choose.
    pub language: Language,
    /// The language those words are in once the choice met the machine:
    /// `en` or `pt-BR`. Resolved in the core and sent here, so the interface
    /// never has to guess from the browser it happens to run in.
    pub locale: String,
    /// Unix seconds, or null while the first run has not been finished.
    pub onboarded_at: Option<f64>,
    /// Three states, not two: `null` is "never asked", which is what lets a
    /// default be a default rather than a silent choice. Updates are on when
    /// the person has not said otherwise.
    pub automatic_updates: Option<bool>,
    /// Whether closing a terminal with an agent or a command still running
    /// stops to ask first.
    ///
    /// Null is "never asked", and never-asked means yes: the first time
    /// someone closes a tab with work in it, the prompt is the only thing
    /// standing between them and losing it. It goes to `false` when they tick
    /// "don't ask again", which is a choice and not a default.
    pub confirm_stop: Option<bool>,
    /// The contrast xterm lifts every colour to, from 1 (none) to 21. Null is
    /// "never asked", which keeps the old rule: none on the dark ground, 4.5
    /// on the light one.
    pub terminal_contrast: Option<f64>,
    /// Whether the focus mode is offered at all.
    ///
    /// Off unless somebody turns it on, and `null` is off here rather than
    /// "never asked": an unfinished thing does not get to be the default by
    /// virtue of nobody having had an opinion about it yet.
    pub focus_mode: Option<bool>,
    /// Whether devpit keeps its own errors for an anonymous report.
    ///
    /// Opt-in: `null` is off. Turning it off deletes what was kept.
    pub error_reports: Option<bool>,
    /// Whether the island opens above the other windows. `null` is on: it
    /// stays out of sight until an agent does something.
    pub island: Option<bool>,
    /// Whether a card's date goes off as a reminder at its time. Null is
    /// never asked, which is on.
    pub reminders: Option<bool>,
}

/// What the next error report would send, for the person to read first.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ErrorReportsPreview {
    /// Errors kept and not yet reported.
    pub waiting: u32,
    /// The next request's body, exactly as it would go.
    pub next: String,
}

#[cfg(test)]
mod tests {
    use super::{Language, Theme};

    #[test]
    fn a_stored_theme_comes_back_as_itself() {
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            assert_eq!(Theme::parse(theme.stored()), theme);
        }
    }

    #[test]
    fn a_name_this_build_does_not_have_reads_as_dark() {
        // One bad row must not fail the whole settings read.
        assert_eq!(Theme::parse("solarized"), Theme::Dark);
    }

    #[test]
    fn a_stored_language_comes_back_as_itself() {
        for language in [
            Language::System,
            Language::English,
            Language::PortugueseBrazil,
        ] {
            assert_eq!(Language::parse(language.stored()), language);
        }
    }

    #[test]
    fn a_name_this_build_does_not_have_reads_as_system() {
        // Not English: an unknown row is the absence of a choice, and the
        // machine gets to answer instead.
        assert_eq!(Language::parse("klingon"), Language::System);
    }
}

/// Whether devpit starts sessions on its own. Both off unless the person
/// turns them on: a long context is no reason for a new session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionRelay {
    /// A finished card opens the next card of its epic in a session of its own.
    pub next_card: bool,
    /// The share of the context window, from 1 to 100, past which a card's
    /// session hands off to a fresh one in the same folder. Null is never.
    pub handoff_at_percent: Option<u32>,
}
