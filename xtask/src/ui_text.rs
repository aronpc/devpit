//! Fixed text on the screen, counted per file and only ever allowed to go down.
//!
//! The catalog is where words live (AGENTS.md); the screens are where they are
//! read. A sentence hardcoded into a component renders in one language on a
//! screen that promised another, and nothing in the diff that adds it says so
//! — the sentence is usually the smallest change in it. So the stretches of
//! fixed text are counted, the count is on record, and it only ever goes
//! down: each translation slice lowers the files it touched, until the list
//! is empty.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::Finding;

/// Where the counts live. One `path count` pair per line.
pub(crate) const BASELINE: &str = include_str!("../ui_text_baseline.txt");

/// The screens of the app, and only they.
const AREAS: [&str; 3] = ["shell", "island", "remote"];

/// Attributes whose literal is words the person reads.
const READ_ALOUD: [&str; 5] = ["title", "aria-label", "placeholder", "alt", "label"];

/// Calls that say a sentence; their first argument is it.
const SAYS: [&str; 3] = ["toast", "notify", "confirm"];

/// No file may grow fixed screen text, and none may start.
pub fn uncatalogued_screen_text_never_grows(root: &Path) -> Vec<Finding> {
    counts_in(BASELINE, root)
}

/// The guard over a given list, so a test can exercise it without editing
/// the real one.
fn counts_in(list: &str, root: &Path) -> Vec<Finding> {
    let allowed = parse(list);
    let mut findings = Vec::new();

    for file in screens(&root.join("web/src")) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let relative = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .to_string_lossy()
            .into_owned();
        let said = unmarked(&text);
        let budget = allowed.get(&relative).copied().unwrap_or(0);
        if said.len() > budget {
            findings.push(Finding {
                file: PathBuf::from(&relative),
                line: said[0],
                what: format!(
                    "{} stretch(es) of fixed screen text, {budget} allowed — move them to \
                     the catalog, or say why on the line: `// ui-text: ok <reason>`",
                    said.len()
                ),
            });
        }
    }

    for (relative, count) in &allowed {
        if !root.join(relative).exists() {
            findings.push(Finding {
                file: PathBuf::from(relative),
                line: 1,
                what: format!("has a count of {count} but no file — drop the entry"),
            });
        }
    }

    findings
}

fn parse(list: &str) -> BTreeMap<String, usize> {
    list.lines()
        .filter_map(|line| {
            let entry = line.split('#').next().unwrap_or(line).trim();
            let (path, count) = entry.rsplit_once(char::is_whitespace)?;
            Some((path.trim().to_owned(), count.trim().parse().ok()?))
        })
        .collect()
}

/// The `.ts`/`.tsx` of the screens. Tests are not screens; neither is
/// anything generated, nor the catalogs themselves.
pub(crate) fn screens(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for area in AREAS {
        for entry in walkdir::WalkDir::new(dir.join(area))
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.file_name().to_string_lossy().contains(".test.") {
                continue;
            }
            if path
                .components()
                .any(|c| matches!(c.as_os_str().to_str(), Some("gen" | "i18n")))
            {
                continue;
            }
            if path.extension().is_some_and(|e| e == "ts" || e == "tsx") {
                found.push(path.to_path_buf());
            }
        }
    }
    found.sort();
    found
}

/// The line each stretch of fixed screen text starts on.
///
/// A lexical walk, not a parse: TypeScript and JSX read with the three states
/// any reader holds — code, inside a tag, between tags — skipping what
/// carries no words (strings, comments, names). Template literals hold
/// interpolated words, which the translation slices move by hand; only plain
/// literals are counted. The walk can be fooled by code no one writes
/// (`a<b` squeezed together, a regex holding a quote), and the line
/// `// ui-text: ok <reason>` is the way out when it is.
pub(crate) fn fixed_text_lines(text: &str) -> Vec<usize> {
    let mut scan = Scan {
        bytes: text.as_bytes(),
        text,
        found: Vec::new(),
        at: 0,
    };
    scan.code(Stop::Eof);
    scan.found
}

/// What ends a stretch of TypeScript.
enum Stop {
    Eof,
    /// The `}` that closes the expression we are inside.
    Brace,
}

struct Scan<'a> {
    bytes: &'a [u8],
    text: &'a str,
    found: Vec<usize>,
    at: usize,
}

impl<'a> Scan<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn ahead(&self, by: usize) -> Option<u8> {
        self.bytes.get(self.at + by).copied()
    }

    fn bump(&mut self) {
        if self.at < self.bytes.len() {
            self.at += 1;
        }
    }

    /// Records the stretch starting at `at`, by its line.
    fn say(&mut self, at: usize) {
        let line = 1 + self.bytes[..at].iter().filter(|&&b| b == b'\n').count();
        self.found.push(line);
    }

    /// TypeScript until `stop`.
    fn code(&mut self, stop: Stop) {
        let mut word = String::new();
        let mut depth = 0usize;
        loop {
            let Some(byte) = self.peek() else { return };
            match byte {
                b'\'' | b'"' => {
                    word.clear();
                    self.skip_string(byte);
                }
                b'`' => {
                    word.clear();
                    self.template();
                }
                b'/' if self.ahead(1) == Some(b'/') => {
                    word.clear();
                    while !matches!(self.peek(), None | Some(b'\n')) {
                        self.bump();
                    }
                }
                b'/' if self.ahead(1) == Some(b'*') => {
                    word.clear();
                    self.bump();
                    self.bump();
                    while let Some(b) = self.peek() {
                        if b == b'*' && self.ahead(1) == Some(b'/') {
                            self.bump();
                            self.bump();
                            break;
                        }
                        self.bump();
                    }
                }
                b'/' if self.regex_not_division() => {
                    word.clear();
                    self.skip_regex();
                }
                b'{' => {
                    word.clear();
                    depth += 1;
                    self.bump();
                }
                b'}' => {
                    word.clear();
                    if depth == 0 && matches!(stop, Stop::Brace) {
                        self.bump();
                        return;
                    }
                    depth = depth.saturating_sub(1);
                    self.bump();
                }
                b'(' => {
                    if SAYS.contains(&word.as_str()) {
                        self.first_argument();
                    }
                    word.clear();
                    self.bump();
                }
                b'<' => {
                    let markup = match self.ahead(1) {
                        Some(b'>') | Some(b'/') => true,
                        Some(c) if c.is_ascii_alphabetic() => {
                            self.markup_not_type() && !self.generic_arrow()
                        }
                        _ => false,
                    };
                    word.clear();
                    self.bump();
                    if markup {
                        let closing = self.peek() == Some(b'/');
                        self.tag(closing);
                    }
                }
                c if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' => {
                    word.push(c as char);
                    self.bump();
                }
                c if c.is_ascii_whitespace() => self.bump(),
                _ => {
                    word.clear();
                    self.bump();
                }
            }
        }
    }

    /// Whether a `<` in TypeScript opens markup rather than a type argument.
    ///
    /// `Record<string, string>` has a name in front of it; `return <div>`
    /// and `=> <Row>` do not. That is the whole distinction, and it holds
    /// because a squeezed comparison (`a<b`) is not code anyone writes.
    fn markup_not_type(&self) -> bool {
        let mut at = self.at;
        // Spaces and tabs only: a `<` at the start of a line opens markup,
        // because a type argument always follows a name on the same line.
        while at > 0 && matches!(self.bytes[at - 1], b' ' | b'\t') {
            at -= 1;
        }
        if at == 0 || self.bytes[at - 1] == b'\n' {
            return true;
        }
        match self.bytes[at - 1] {
            // `=>` returns markup, and so does the `>` a closed tag leaves.
            b'>' => true,
            // A type argument straight inside another (`Array<Array<X>>`).
            b'<' => false,
            b')' | b']' => false,
            c if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' => {
                let name: String = self.bytes[..at]
                    .iter()
                    .rev()
                    .take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .map(|&b| b as char)
                    .collect();
                matches!(
                    name.as_str(),
                    "return"
                        | "await"
                        | "yield"
                        | "typeof"
                        | "case"
                        | "default"
                        | "of"
                        | "in"
                        | "do"
                        | "else"
                        | "new"
                        | "void"
                        | "delete"
                        | "throw"
                )
            }
            _ => true,
        }
    }

    /// Whether a `/` opens a regular expression rather than dividing: a
    /// regex can only follow an operator, and division only an operand.
    fn regex_not_division(&self) -> bool {
        let mut at = self.at;
        while at > 0 && matches!(self.bytes[at - 1], b' ' | b'\t' | b'\n') {
            at -= 1;
        }
        match self.bytes.get(at.wrapping_sub(1)).copied() {
            None => true,
            // An operand can be divided; a regex can only follow an operator.
            Some(b')' | b']') => false,
            Some(b'=' | b'(' | b'[' | b'{' | b',' | b';' | b':' | b'&' | b'|' | b'!') => true,
            Some(c) => {
                !(c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'"' | b'\'' | b'`'))
            }
        }
    }

    /// Through the closing `/`, with the flags behind it. A `/` inside a
    /// character class closes nothing, and `\` escapes anything.
    fn skip_regex(&mut self) {
        let mut in_class = false;
        self.bump();
        while let Some(b) = self.peek() {
            match b {
                b'\\' => {
                    self.bump();
                    self.bump();
                }
                b'[' => {
                    in_class = true;
                    self.bump();
                }
                b']' => {
                    in_class = false;
                    self.bump();
                }
                b'/' if !in_class => {
                    self.bump();
                    return;
                }
                b'\n' => return,
                _ => self.bump(),
            }
        }
    }

    /// An arrow's type parameters — `<T,>()`, `<T extends string>`, plain
    /// `<T>` with the parameters right behind. The one place where markup
    /// and TypeScript wear the same opening, told apart by what a type
    /// parameter may be followed by (`extends` is not an attribute name, and
    /// no tag is called `T` and called with `(`).
    fn generic_arrow(&self) -> bool {
        let bytes = self.bytes;
        let start = self.at + 1;
        let mut at = start;
        while at < bytes.len()
            && (bytes[at].is_ascii_alphanumeric() || matches!(bytes[at], b'_' | b'$'))
        {
            at += 1;
        }
        if at == start {
            return false;
        }
        match bytes.get(at) {
            Some(b',') | Some(b'=') => return true,
            Some(b'>') => {
                return bytes.get(at + 1..).is_some_and(|rest| {
                    rest.iter()
                        .find(|&&b| !b.is_ascii_whitespace())
                        .is_some_and(|&b| b == b'(')
                });
            }
            _ => {}
        }
        self.text[at..].trim_start().starts_with("extends ")
    }

    /// The literal a `toast(`/`notify(`/`confirm(` says, when its first
    /// argument is one. Nothing is consumed; the walk reads it again as an
    /// ordinary string.
    fn first_argument(&mut self) {
        let mut at = self.at + 1;
        while matches!(self.bytes.get(at), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            at += 1;
        }
        let Some(&quote) = self.bytes.get(at) else {
            return;
        };
        if quote != b'\'' && quote != b'"' {
            return;
        }
        let mut end = at + 1;
        while let Some(&b) = self.bytes.get(end) {
            if b == b'\\' {
                end += 2;
            } else if b == quote || b == b'\n' {
                break;
            } else {
                end += 1;
            }
        }
        if self.bytes.get(end) == Some(&quote) && says_words(&self.text[at + 1..end]) {
            self.say(at);
        }
    }

    /// A tag's inside: from just past `<` to the `>` that closes it, with the
    /// children when it has them.
    fn tag(&mut self, closing: bool) {
        let mut attr = String::new();
        let mut pending: Option<String> = None;
        loop {
            let Some(byte) = self.peek() else { return };
            match byte {
                b'"' | b'\'' => {
                    let start = self.at;
                    self.bump();
                    let body = self.at;
                    self.skip_string(byte);
                    let end = if self.bytes.get(self.at.wrapping_sub(1)) == Some(&byte) {
                        self.at - 1
                    } else {
                        self.at
                    };
                    if pending.as_deref().is_some_and(|a| READ_ALOUD.contains(&a))
                        && says_words(&self.text[body..end])
                    {
                        self.say(start);
                    }
                    pending = None;
                    attr.clear();
                }
                b'{' => {
                    self.bump();
                    let body = self.at;
                    self.code(Stop::Brace);
                    let end = self.at.saturating_sub(1);
                    if pending.as_deref().is_some_and(|a| READ_ALOUD.contains(&a)) {
                        if let Some((at, literal)) = lone_literal(&self.text[body..end]) {
                            if says_words(literal) {
                                self.say(body + at);
                            }
                        }
                    }
                    pending = None;
                    attr.clear();
                }
                b'/' if self.ahead(1) == Some(b'>') => {
                    self.bump();
                    self.bump();
                    return;
                }
                b'>' => {
                    self.bump();
                    if !closing {
                        self.children();
                    }
                    return;
                }
                b'=' => {
                    if !attr.is_empty() {
                        pending = Some(attr.clone());
                    }
                    attr.clear();
                    self.bump();
                }
                c if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b':') => {
                    attr.push(c as char);
                    self.bump();
                }
                c if c.is_ascii_whitespace() => {
                    attr.clear();
                    self.bump();
                }
                _ => {
                    attr.clear();
                    pending = None;
                    self.bump();
                }
            }
        }
    }

    /// What a tag wraps, until its closing tag: text, expressions, and
    /// whatever those open.
    fn children(&mut self) {
        let mut start = self.at;
        loop {
            let Some(byte) = self.peek() else {
                self.end_text(start);
                return;
            };
            match byte {
                b'<' => {
                    self.end_text(start);
                    if self.ahead(1) == Some(b'/') {
                        self.bump();
                        self.bump();
                        while let Some(b) = self.peek() {
                            self.bump();
                            if b == b'>' {
                                return;
                            }
                        }
                        return;
                    }
                    self.bump();
                    self.tag(false);
                    start = self.at;
                }
                b'{' => {
                    self.end_text(start);
                    self.bump();
                    let body = self.at;
                    self.code(Stop::Brace);
                    let end = self.at.saturating_sub(1);
                    if let Some((at, literal)) = lone_literal(&self.text[body..end]) {
                        if says_words(literal) {
                            self.say(body + at);
                        }
                    }
                    start = self.at;
                }
                _ => self.bump(),
            }
        }
    }

    /// A run of text between markup, when it says anything.
    fn end_text(&mut self, start: usize) {
        if let Some(at) = first_letter(&self.text[start..self.at]) {
            self.say(start + at);
        }
    }

    /// A template literal, whose `${…}` holes are TypeScript again.
    fn template(&mut self) {
        self.bump();
        loop {
            let Some(byte) = self.peek() else { return };
            match byte {
                b'`' => {
                    self.bump();
                    return;
                }
                b'\\' => {
                    self.bump();
                    self.bump();
                }
                b'$' if self.ahead(1) == Some(b'{') => {
                    self.bump();
                    self.bump();
                    self.code(Stop::Brace);
                }
                _ => self.bump(),
            }
        }
    }

    /// Through the closing quote. A string does not cross a newline; one
    /// that never closes stops there, holding the damage to its own line.
    fn skip_string(&mut self, quote: u8) {
        self.bump();
        while let Some(b) = self.peek() {
            match b {
                b'\\' => {
                    self.bump();
                    self.bump();
                }
                b'\n' => return,
                b if b == quote => {
                    self.bump();
                    return;
                }
                _ => self.bump(),
            }
        }
    }
}

/// Whether a run of markup text says words: a letter, once the HTML
/// entities (`&mdash;`, `&#8212;`) are set aside — they are punctuation
/// wearing letters.
fn says_words(text: &str) -> bool {
    first_letter(text).is_some()
}

/// Where the first letter is, entities aside.
fn first_letter(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'&' {
            if let Some(end) = entity_end(&text[at..]) {
                at += end;
                continue;
            }
        }
        let c = text[at..].chars().next()?;
        if c.is_alphabetic() {
            return Some(at);
        }
        at += c.len_utf8();
    }
    None
}

/// An entity runs to its `;` and holds nothing but names and digits.
fn entity_end(from: &str) -> Option<usize> {
    let bytes = from.as_bytes();
    let mut at = 1;
    while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'#') {
        at += 1;
    }
    (at > 1 && bytes.get(at) == Some(&b';')).then(|| at + 1)
}

/// A string literal and nothing else — `{'Save'}` is what JSX reads as text,
/// and `{cond ? 'Save' : 'Load'}` is a choice, not a saying. Returns where
/// the literal starts, and what it says.
fn lone_literal(body: &str) -> Option<(usize, &str)> {
    let at = body.find(['\'', '"'])?;
    if !body[..at].trim().is_empty() {
        return None;
    }
    let quote = body.as_bytes()[at];
    let bytes = body.as_bytes();
    let mut end = at + 1;
    while end < bytes.len() {
        match bytes[end] {
            b'\\' => end += 2,
            b'\n' => return None,
            b if b == quote => break,
            _ => end += 1,
        }
    }
    if bytes.get(end) != Some(&quote) || !body[end + 1..].trim().is_empty() {
        return None;
    }
    Some((at, &body[at + 1..end]))
}

/// The stretches no line has excused.
fn unmarked(text: &str) -> Vec<usize> {
    let excused = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("// ui-text: ok"))
        .map(|(n, _)| n + 1)
        .collect::<BTreeSet<_>>();
    fixed_text_lines(text)
        .into_iter()
        .filter(|line| !excused.contains(line))
        .collect()
}

/// The count a file keeps after a reseed: what it had, unless the tree now
/// says less — and nothing at all when the list never carried it.
fn next_count(had: Option<&usize>, said: usize) -> usize {
    had.map_or(0, |had| (*had).min(said))
}

/// Rewrites `xtask/ui_text_baseline.txt` from what the tree says now.
///
/// Tightening only, for the same reason as the ceilings: a count already on
/// record is kept whenever the file now wants a larger one, and a file with
/// no count never gets one. The list is born once — from an empty file, the
/// state it is first compiled against. Returns how many stretches are left.
pub fn reseed(root: &Path) -> std::io::Result<usize> {
    let born = BASELINE.trim().is_empty();
    let existing = parse(BASELINE);
    let mut lines = vec![
        "# Fixed screen text, per file. One \"path count\" per line.".to_owned(),
        "#".to_owned(),
        "# Only ever goes down. A file not listed here is allowed none at all, which".to_owned(),
        "# is the rule for anything written from here on; a slice that translated its".to_owned(),
        "# files runs `cargo xtask ui-text` to take the credit.".to_owned(),
        String::new(),
    ];

    let mut left = 0;
    for file in screens(&root.join("web/src")) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let relative = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .to_string_lossy()
            .into_owned();
        let said = unmarked(&text).len();
        let count = if born {
            said
        } else {
            next_count(existing.get(&relative), said)
        };
        if count > 0 {
            lines.push(format!("{relative} {count}"));
            left += count;
        }
    }

    std::fs::write(
        root.join("xtask/ui_text_baseline.txt"),
        lines.join("\n") + "\n",
    )?;
    Ok(left)
}

#[cfg(test)]
#[path = "ui_text_tests.rs"]
mod tests;
