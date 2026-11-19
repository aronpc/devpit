//! What a directory read can refuse, in the person's language.

use std::path::PathBuf;

use crate::i18n::t_for;
use crate::locale::Locale;

#[derive(Debug, thiserror::Error)]
pub enum TreeError {
    #[error("{}", self.said(Locale::current()))]
    Outside { path: PathBuf },

    #[error("{}", self.said(Locale::current()))]
    Unreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{}", self.said(Locale::current()))]
    AlreadyExists { path: PathBuf },
}

impl TreeError {
    /// The sentence in `locale`. The screen gets the person's language
    /// (`Display`); the error report always asks for English.
    pub fn said(&self, locale: Locale) -> String {
        match self {
            Self::Outside { path } => t_for(
                locale,
                "error.outsideProject",
                &[("path", &path.display().to_string())],
            ),
            Self::Unreadable { path, source } => t_for(
                locale,
                "error.unreadable",
                &[
                    ("path", &path.display().to_string()),
                    ("reason", &source.to_string()),
                ],
            ),
            Self::AlreadyExists { path } => t_for(
                locale,
                "error.alreadyExists",
                &[("path", &path.display().to_string())],
            ),
        }
    }
}
