//! What each ptyd pane shows, with no window on it.
//!
//! The daemon keeps a pane's bytes, not its screen, so [`crate::ptyd_tap`]
//! keeps a terminal per pane: seeded from the attach's replay, fed what the
//! pane prints. It is what `capture-pane` is for a tmux pane — the question
//! an agent is stopped on, read and answered — and it says how the program
//! there wants its keys and pastes.
//!
//! A screen is drawn at a size, and the daemon never says a pane's: it is
//! the last one devpit told it, kept here and in a file under devpit's home,
//! so a screen read after a restart, before any window, is the right shape.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::pane_io::Modes;

/// A pane's size until devpit told it one: what `ptyd_link::asked` starts it at.
const SPAWNED: (u16, u16) = (24, 80);

/// The most of the sizes file read back: a line a pane, and a handful of panes.
const MOST_KEPT: u64 = 64 * 1024;

/// The file the sizes are kept in, under devpit's home.
pub(crate) const SIZES: &str = "ptyd-sizes";

/// One pane's terminal, and whether the last frame it had was its replay.
struct Screen {
    parser: vt100::Parser,
    replaying: bool,
}

#[derive(Default)]
pub(crate) struct Screens {
    screens: HashMap<u32, Screen>,
    sizes: HashMap<u32, (u16, u16)>,
    /// Where the sizes are kept across runs; nowhere in a test.
    kept: Option<PathBuf>,
}

impl Screens {
    /// Reads the sizes kept in `file`, and keeps them there from now on.
    pub(crate) fn keep_at(&mut self, file: PathBuf) {
        self.sizes.extend(read(&file));
        self.kept = Some(file);
    }

    /// A replay starts a pane's screen over: the modes, then the tail of what
    /// it printed, are all there is to draw it from.
    pub(crate) fn replay(&mut self, pane: u32, bytes: &[u8]) {
        let fresh = self.screens.get(&pane).is_none_or(|one| !one.replaying);
        if fresh {
            let parser = self.parser(pane);
            self.screens.insert(
                pane,
                Screen {
                    parser,
                    replaying: true,
                },
            );
        }
        if let Some(one) = self.screens.get_mut(&pane) {
            one.parser.process(bytes);
        }
    }

    pub(crate) fn output(&mut self, pane: u32, bytes: &[u8]) {
        if !self.screens.contains_key(&pane) {
            let parser = self.parser(pane);
            self.screens.insert(
                pane,
                Screen {
                    parser,
                    replaying: false,
                },
            );
        }
        if let Some(one) = self.screens.get_mut(&pane) {
            one.replaying = false;
            one.parser.process(bytes);
        }
    }

    pub(crate) fn ended(&mut self, pane: u32) {
        self.screens.remove(&pane);
        if self.sizes.remove(&pane).is_some() {
            self.write();
        }
    }

    /// The size devpit just told the daemon pane `pane` is.
    pub(crate) fn sized(&mut self, pane: u32, rows: u16, cols: u16) {
        let (Some(height), Some(width)) = (non_zero(rows), non_zero(cols)) else {
            return;
        };
        if self.sizes.insert(pane, (rows, cols)) == Some((rows, cols)) {
            return;
        }
        if let Some(one) = self.screens.get_mut(&pane) {
            one.parser.set_size(height, width);
        }
        self.write();
    }

    /// The pane's screen as text, a line a row as `capture-pane -p` gives a
    /// tmux one: a long line wrapped is two.
    pub(crate) fn contents(&self, pane: u32) -> Option<String> {
        let screen = self.screens.get(&pane)?.parser.screen();
        let (_, cols) = screen.size();
        let rows: Vec<String> = screen.rows(0, cols.get()).collect();
        Some(rows.join("\n"))
    }

    /// Where the cursor stands, as (column, row) of the screen.
    pub(crate) fn cursor(&self, pane: u32) -> Option<(u16, u16)> {
        let (row, column) = self.screens.get(&pane)?.parser.screen().cursor_position();
        Some((column, row))
    }

    pub(crate) fn modes(&self, pane: u32) -> Option<Modes> {
        let screen = self.screens.get(&pane)?.parser.screen();
        Some(Modes {
            application_cursor: screen.application_cursor(),
            bracketed_paste: screen.bracketed_paste(),
        })
    }

    fn parser(&self, pane: u32) -> vt100::Parser {
        let (rows, cols) = self.sizes.get(&pane).copied().unwrap_or(SPAWNED);
        // The screen only, as `capture-pane -p` reads it: no scrollback.
        vt100::Parser::new(
            non_zero(rows).unwrap_or(vt100::Parser::DEFAULT_ROWS),
            non_zero(cols).unwrap_or(vt100::Parser::DEFAULT_COLS),
            0,
        )
    }

    fn write(&self) {
        let Some(file) = &self.kept else {
            return;
        };
        let mut lines: Vec<String> = (self.sizes.iter())
            .map(|(pane, (rows, cols))| format!("{pane} {rows} {cols}\n"))
            .collect();
        lines.sort();
        let _ = std::fs::write(file, lines.concat());
    }
}

fn non_zero(n: u16) -> Option<std::num::NonZeroU16> {
    std::num::NonZeroU16::new(n)
}

/// `pane rows cols` a line; a line that is not is skipped.
pub(crate) fn read(file: &Path) -> HashMap<u32, (u16, u16)> {
    let mut text = String::new();
    if let Ok(opened) = std::fs::File::open(file) {
        let _ = opened.take(MOST_KEPT).read_to_string(&mut text);
    }
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace().map(str::parse::<u32>);
            let (pane, rows, cols) = (
                words.next()?.ok()?,
                words.next()?.ok()?,
                words.next()?.ok()?,
            );
            Some((pane, (u16::try_from(rows).ok()?, u16::try_from(cols).ok()?)))
        })
        .collect()
}

#[cfg(test)]
#[path = "ptyd_screens_tests.rs"]
mod tests;
