//! What each pane's agent last said through its hooks, so the live sessions
//! read a pane's screen only when its agent says it waits on a person.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::card_activity::Doing;

#[derive(Default)]
pub(crate) struct PaneStates {
    heard: HashMap<String, Doing>,
    /// Panes no hook spoke for, alive from before the app or an agent with
    /// none: whether their last read showed a prompt.
    looked: HashMap<String, bool>,
}

impl PaneStates {
    pub(crate) fn heard(&mut self, leaf: &str, doing: Doing) {
        self.heard.insert(leaf.to_owned(), doing);
    }

    /// What a read of a pane no hook spoke for showed.
    pub(crate) fn read(&mut self, leaf: &str, prompt: bool) {
        if !self.heard.contains_key(leaf) {
            self.looked.insert(leaf.to_owned(), prompt);
        }
    }

    /// What the pane's agent last said through a hook, if it ever spoke.
    pub(crate) fn heard_of(&self, leaf: &str) -> Option<Doing> {
        self.heard.get(leaf).copied()
    }

    /// Whether the screen is worth reading: the agent said it waits, or it has
    /// said nothing and was never read or was last seen on a prompt.
    pub(crate) fn should_look(&self, leaf: &str) -> bool {
        match self.heard.get(leaf) {
            Some(Doing::Waiting) => true,
            Some(_) => false,
            None => self.looked.get(leaf).copied().unwrap_or(true),
        }
    }
}

fn states() -> &'static Mutex<PaneStates> {
    static STATES: std::sync::OnceLock<Mutex<PaneStates>> = std::sync::OnceLock::new();
    STATES.get_or_init(Mutex::default)
}

pub(crate) fn heard(leaf: &str, doing: Doing) {
    if let Ok(mut states) = states().lock() {
        states.heard(leaf, doing);
    }
}

pub(crate) fn read(leaf: &str, prompt: bool) {
    if let Ok(mut states) = states().lock() {
        states.read(leaf, prompt);
    }
}

pub(crate) fn heard_of(leaf: &str) -> Option<Doing> {
    states().lock().ok()?.heard_of(leaf)
}

pub(crate) fn should_look(leaf: &str) -> bool {
    states().lock().is_ok_and(|states| states.should_look(leaf))
}

#[cfg(test)]
#[path = "pane_said_tests.rs"]
mod tests;
