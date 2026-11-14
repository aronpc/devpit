//! A project agent's memory: `.devpit/project-agent/` in the project's folder.
//!
//! Three Markdown files of dated entries, each naming where it came from, so
//! a fact can be re-checked when its files change and proposed for update or
//! removal when it goes stale. Written once, on the agent's first wake, from
//! the guides the project already keeps; after that only what the person
//! approves changes them. A project whose agent was never asked has no such
//! folder.

use std::io::Read;
use std::path::Path;

pub(crate) const DIR: &str = ".devpit/project-agent";
pub(crate) const FILES: [&str; 3] = ["facts.md", "lessons.md", "decisions.md"];
/// The entries of each file the agent wakes with; it reads the rest itself.
pub(crate) const SHOWN: usize = 20;
const SEP: &str = " · ";
/// How much of a guide or an ADR is read for its heading.
pub(crate) const HEAD_BYTES: usize = 16 * 1024;
/// How much of a memory file the summary reads.
const MEMORY_BYTES: u64 = 256 * 1024;

/// The guides a first wake points at, when the project has them.
const GUIDES: [&str; 4] = [
    "AGENTS.md",
    "CLAUDE.md",
    "README.md",
    "docs/architecture.md",
];

pub(crate) struct Entry {
    pub day: String,
    pub source: String,
    pub text: String,
}

impl Entry {
    /// What its source lists, comma-separated: files, cards and pull requests.
    fn parts(&self) -> impl Iterator<Item = &str> {
        self.source
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
    }

    /// The files it rests on: what its source lists that is not a card or a PR.
    pub(crate) fn files(&self) -> Vec<&str> {
        self.parts()
            .filter(|part| {
                !card(part) && !part.starts_with("PR #") && *part != ONBOARDING && *part != AUTO
            })
            .collect()
    }

    /// Whether it entered the memory without the person, on Labs' switch.
    pub(crate) fn auto(&self) -> bool {
        self.parts().any(|part| part == AUTO)
    }

    /// Whether the agent's own read of the project proposed it.
    pub(crate) fn onboarding(&self) -> bool {
        self.parts().any(|part| part == ONBOARDING)
    }

    /// The cards it came from, as `#12`.
    pub(crate) fn cards(&self) -> Vec<&str> {
        self.parts().filter(|part| card(part)).collect()
    }
}

fn card(part: &str) -> bool {
    part.strip_prefix('#').is_some_and(|number| {
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
    })
}

/// A bullet as an entry, or why it is not one; `None` for any other line.
pub(crate) fn entry(line: &str) -> Option<Result<Entry, String>> {
    let rest = line.strip_prefix("- ")?;
    let mut parts = rest.splitn(3, SEP);
    let (Some(day), Some(source), Some(text)) = (parts.next(), parts.next(), parts.next()) else {
        return Some(Err(format!(
            "an entry is `- YYYY-MM-DD{SEP}source{SEP}what it says`"
        )));
    };
    if !dated(day) {
        return Some(Err(format!("`{day}` is not a date as YYYY-MM-DD")));
    }
    let (source, text) = (source.trim(), text.trim());
    if source.is_empty() || text.is_empty() {
        return Some(Err(
            "an entry names its source and says something".to_owned()
        ));
    }
    Some(Ok(Entry {
        day: day.to_owned(),
        source: source.to_owned(),
        text: text.to_owned(),
    }))
}

fn dated(day: &str) -> bool {
    let bytes = day.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(at, byte)| match at {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

/// Writes the memory on the agent's first wake: false when it is already kept.
pub(crate) fn seed(root: &Path, day: &str) -> Result<bool, String> {
    let dir = root.join(DIR);
    if dir.exists() {
        return Ok(false);
    }
    // Resolved inside the project, a link included, before anything is written.
    for folder in [".devpit", DIR] {
        let at = devpit_core::paths::resolve_new(root, folder).map_err(|err| err.to_string())?;
        std::fs::create_dir_all(&at).map_err(|err| err.to_string())?;
    }
    let facts = GUIDES.iter().filter_map(|guide| {
        let head = read_inside(root, guide, HEAD_BYTES as u64)?;
        let title = heading(&head, guide);
        Some(line(
            day,
            guide,
            &format!("A guide to read first: \"{title}\"."),
        ))
    });
    let decisions = adrs(root).into_iter().filter_map(|adr| {
        let head = read_inside(root, &adr, HEAD_BYTES as u64)?;
        Some(line(day, &adr, &heading(&head, &adr)))
    });
    let files = [
        ("facts.md", "Facts", facts.collect::<Vec<_>>()),
        ("lessons.md", "Lessons", Vec::new()),
        ("decisions.md", "Decisions", decisions.collect()),
    ];
    for (file, title, lines) in files {
        let at = devpit_core::paths::resolve_new(root, &format!("{DIR}/{file}"))
            .map_err(|err| err.to_string())?;
        let mut text = format!(
            "# {title}\n\nEach entry: `- YYYY-MM-DD{SEP}source{SEP}what it says`. The source is a file, a card (#12) or a PR (PR #3).\n\n"
        );
        for line in lines {
            text.push_str(&line);
        }
        std::fs::write(at, text).map_err(|err| err.to_string())?;
    }
    Ok(true)
}

fn line(day: &str, source: &str, text: &str) -> String {
    format!(
        "- {day}{SEP}{source}{SEP}{}\n",
        text.replace(['\n', '\r'], " ")
    )
}

/// A regular file of the project, resolved inside its root and read up to
/// `limit` bytes. `None` for a link leading out, a device or a folder.
fn read_inside(root: &Path, relative: &str, limit: u64) -> Option<String> {
    let at = devpit_core::tree::resolve(root, relative).ok()?;
    if !at.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(at)
        .ok()?
        .take(limit)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The first heading of `text`, or the file's own name when it has none.
fn heading(text: &str, file: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix("# ").map(|title| title.trim().to_owned()))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| file.to_owned())
}

fn adrs(root: &Path) -> Vec<String> {
    let Ok(folder) = devpit_core::tree::resolve(root, "docs/adr") else {
        return Vec::new();
    };
    let mut found: Vec<String> = std::fs::read_dir(folder)
        .map(|dir| {
            dir.filter_map(Result::ok)
                .filter_map(|one| one.file_name().into_string().ok())
                .filter(|name| name.ends_with(".md"))
                .map(|name| format!("docs/adr/{name}"))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// What the agent wakes with: each file's newest entries and what was left
/// out. `None` while the project has no memory.
pub(crate) fn summary(root: &Path) -> Option<String> {
    let dir = root.join(DIR);
    if !dir.is_dir() {
        return None;
    }
    let mut out = format!(
        "Your memory is kept in {DIR}/: these are its newest entries; read the files for the rest.\n"
    );
    for file in FILES {
        let text = read_inside(root, &format!("{DIR}/{file}"), MEMORY_BYTES).unwrap_or_default();
        let (mut kept, mut broken) = (Vec::new(), 0);
        for found in text.lines().filter_map(entry) {
            match found {
                Ok(one) => kept.push(one),
                Err(_) => broken += 1,
            }
        }
        kept.sort_by(|a, b| b.day.cmp(&a.day));
        out.push_str(&format!("\n{file} ({} entries)\n", kept.len()));
        for one in kept.iter().take(SHOWN) {
            out.push_str(&format!(
                "- {}{SEP}{}{SEP}{}\n",
                one.day, one.source, one.text
            ));
        }
        if kept.len() > SHOWN {
            out.push_str(&format!("({} older in the file)\n", kept.len() - SHOWN));
        }
        if broken > 0 {
            let lines = if broken == 1 { "line" } else { "lines" };
            out.push_str(&format!(
                "({broken} {lines} left out: an entry needs its date and its source)\n"
            ));
        }
    }
    Some(out)
}

/// What closed cards and re-checks proposed, waiting for the person's decision.
pub(crate) const PROPOSED: &str = "proposed.md";
/// One line per closed card: its review rounds and repeated findings.
const METRICS: &str = "metrics.jsonl";
/// The default branch's commit the memory was last re-checked against.
const CHECKED: &str = "checked";
/// Between a stale entry's proposed text and the text it replaces.
const REPLACES: &str = "⟸";
/// Between a proposal and its one plain line for the person, which stays in
/// `proposed.md`: the memory keeps what it says.
const IN_SHORT: char = '¶';
/// The longest plain line a proposal shows when it has none of its own.
const PLAIN_CHARS: usize = 140;
/// Proposals and decisions write one at a time.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Keeps `proposed`, each from `source`, until the person decides each.
pub(crate) fn propose(
    root: &Path,
    day: &str,
    source: &str,
    proposed: &[String],
) -> Result<(), String> {
    propose_or_accept(root, day, source, proposed, false).map(|_| ())
}

/// [`propose`], save that with `accept` a fact resting only on files of the
/// project goes to `facts.md` at once, marked as accepted without the person.
/// Answers how many went there.
pub(crate) fn propose_or_accept(
    root: &Path,
    day: &str,
    source: &str,
    proposed: &[String],
    accept: bool,
) -> Result<usize, String> {
    let (alone, waiting): (Vec<&String>, Vec<&String>) = proposed
        .iter()
        .partition(|one| accept && accepts_alone(root, source, one));
    let text: String = waiting.iter().map(|one| line(day, source, one)).collect();
    let _held = WRITING
        .lock()
        .map_err(|_| "the memory is busy".to_owned())?;
    append(root, PROPOSED, &text)?;
    let files = Entry {
        day: String::new(),
        source: source.to_owned(),
        text: String::new(),
    }
    .files()
    .join(", ");
    for one in &alone {
        append(
            root,
            "facts.md",
            &line(day, &format!("{files}, {AUTO}"), aim(one).1),
        )?;
    }
    Ok(alone.len())
}

/// Marks, in an entry's source, a fact accepted without the person.
pub(crate) const AUTO: &str = "accepted automatically";

/// Whether proposal `said` from `source` needs no person: a new fact whose
/// source is files alone, each a file of the project now.
fn accepts_alone(root: &Path, source: &str, said: &str) -> bool {
    let one = Entry {
        day: String::new(),
        source: source.to_owned(),
        text: String::new(),
    };
    let files = one.files();
    let (file, _, replaces) = aim(said);
    file == "facts.md"
        && replaces.is_none()
        && !files.is_empty()
        && one
            .parts()
            .all(|part| part == ONBOARDING || files.contains(&part))
        && files
            .iter()
            .all(|named| devpit_core::tree::resolve(root, named).is_ok_and(|at| at.is_file()))
}

/// Takes back the fact saying `text` that was accepted without the person:
/// out of `facts.md`, and waiting for them again on the files it rests on.
pub(crate) fn undo_accepted(root: &Path, text: &str) -> Result<(), String> {
    let _held = WRITING
        .lock()
        .map_err(|_| "the memory is busy".to_owned())?;
    let is = |one: &Entry| one.auto() && one.text == text;
    let found = entries(root, "facts.md")
        .into_iter()
        .find(is)
        .ok_or("that fact is no longer one accepted automatically")?;
    rewrite(
        root,
        "facts.md",
        |line| matches!(entry(line), Some(Ok(one)) if is(&one)),
        None,
    )?;
    let said = aimed("facts.md", text, None);
    append(
        root,
        PROPOSED,
        &line(&found.day, &found.files().join(", "), &said),
    )
}

/// A proposal's text with `short`, its one plain line for the person.
pub(crate) fn summed(said: String, short: Option<&str>) -> String {
    let short = short
        .map(|short| short.replace([IN_SHORT, '⟸'], " "))
        .map(|short| short.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|short| !short.is_empty());
    match short {
        Some(short) => format!("{said} {IN_SHORT} {short}"),
        None => said,
    }
}

/// The plain line a proposal was given, when it was given one.
pub(crate) fn in_short(said: &str) -> Option<&str> {
    said.split_once(IN_SHORT)
        .map(|(_, short)| short.trim())
        .filter(|short| !short.is_empty())
}

/// The one line a proposal shows first: its own plain line, or else the
/// first sentence of what it says.
pub(crate) fn plain(said: &str) -> String {
    if let Some(short) = in_short(said) {
        return short.to_owned();
    }
    let (_, text, replaces) = aim(said);
    let text = if text.is_empty() {
        replaces.unwrap_or_default()
    } else {
        text
    };
    let first = text.split_inclusive(". ").next().unwrap_or(text).trim();
    if first.chars().count() <= PLAIN_CHARS {
        return first.to_owned();
    }
    let cut: String = first.chars().take(PLAIN_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Who proposed it, in a few words the person reads beside it.
pub(crate) fn proposer(one: &Entry) -> String {
    if one.onboarding() {
        return "onboarding".to_owned();
    }
    if aim(&one.text).2.is_some() {
        return "a re-check of its memory".to_owned();
    }
    match one.cards() {
        cards if !cards.is_empty() => format!("a session on card {}", cards.join(", ")),
        _ if !one.files().is_empty() => "an answer in Ask the project".to_owned(),
        _ => "the project agent".to_owned(),
    }
}

/// A proposal's name, from its line: the same line is the same proposal.
pub(crate) fn id_of(one: &Entry) -> String {
    // FNV-1a: stable across runs and builds, unlike the std hasher.
    let hash = line(&one.day, &one.source, &one.text)
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    format!("{:012x}", hash & 0xffff_ffff_ffff)
}

/// A proposal's text: the memory file it goes to, what it says and the stale
/// entry it replaces. A lesson's is its text alone.
pub(crate) fn aimed(file: &str, text: &str, replaces: Option<&str>) -> String {
    let text = text.replace(['⟸', IN_SHORT], " ");
    match (file, replaces, text.trim()) {
        ("lessons.md", None, text) => text.to_owned(),
        (_, None, text) => format!("{file}: {text}"),
        (_, Some(was), "") => format!("{file}: {REPLACES} {was}"),
        (_, Some(was), text) => format!("{file}: {text} {REPLACES} {was}"),
    }
}

/// A proposal's text read back as [`aimed`] wrote it, without its plain line.
pub(crate) fn aim(said: &str) -> (&'static str, &str, Option<&str>) {
    let said = said
        .split_once(IN_SHORT)
        .map_or(said, |(said, _)| said.trim_end());
    for file in FILES {
        if let Some(rest) = said
            .strip_prefix(file)
            .and_then(|rest| rest.strip_prefix(": "))
        {
            return match rest.split_once(REPLACES) {
                Some((text, was)) => (file, text.trim(), Some(was.trim())),
                None => (file, rest.trim(), None),
            };
        }
    }
    ("lessons.md", said, None)
}

/// The proposals waiting for the person, oldest first.
pub(crate) fn proposals(root: &Path) -> Vec<Entry> {
    entries(root, PROPOSED)
}

/// The newest approved lessons, at most [`SHOWN`], oldest first.
pub(crate) fn lessons(root: &Path) -> Vec<Entry> {
    let all = entries(root, "lessons.md");
    let older = all.len().saturating_sub(SHOWN);
    all.into_iter().skip(older).collect()
}

/// The decisions that name the cards they came from.
pub(crate) fn decided(root: &Path) -> Vec<Entry> {
    entries(root, "decisions.md")
        .into_iter()
        .filter(|one| !one.cards().is_empty())
        .collect()
}

/// Settles the proposal of `source` saying `text`, replacing `replaces` when it
/// re-checked a stale entry. Approved as `kept`, dated `day`, it is added to its
/// memory file, or takes the stale entry's place, which an empty `kept` removes.
/// Discarded (`kept` is `None`), it leaves no trace and the memory as it was.
pub(crate) fn decide(
    root: &Path,
    day: &str,
    source: &str,
    text: &str,
    replaces: Option<&str>,
    kept: Option<&str>,
) -> Result<(), String> {
    let _held = WRITING
        .lock()
        .map_err(|_| "the memory is busy".to_owned())?;
    let was = read_inside(root, &format!("{DIR}/{PROPOSED}"), MEMORY_BYTES).unwrap_or_default();
    let mut lines: Vec<&str> = was.lines().collect();
    let (at, file, kept_as) = lines
        .iter()
        .enumerate()
        .find_map(|(at, one)| match entry(one) {
            Some(Ok(found)) if found.source == source => {
                let (file, said, stale) = aim(&found.text);
                // The memory keeps the files a read rested on, not the read.
                let kept_as = if found.onboarding() {
                    found.files().join(", ")
                } else {
                    source.to_owned()
                };
                (said == text && stale == replaces).then_some((at, file, kept_as))
            }
            _ => None,
        })
        .ok_or("that proposal is no longer waiting")?;
    lines.remove(at);
    match (kept, replaces) {
        (Some(kept), Some(stale)) => {
            let now = (!kept.is_empty()).then(|| line(day, source, kept));
            rewrite(
                root,
                file,
                |one| matches!(entry(one), Some(Ok(found)) if found.source == source && found.text == stale),
                now.as_deref(),
            )?;
        }
        (Some(kept), None) if !kept.is_empty() => append(root, file, &line(day, &kept_as, kept))?,
        _ => {}
    }
    let rest: String = lines.iter().map(|one| format!("{one}\n")).collect();
    write(root, PROPOSED, &rest)
}

/// Replaces the first line of a memory file that `is` matches with `now`, or
/// removes it when `now` is `None`.
fn rewrite(
    root: &Path,
    file: &str,
    is: impl Fn(&str) -> bool,
    now: Option<&str>,
) -> Result<(), String> {
    let path = format!("{DIR}/{file}");
    let at = devpit_core::paths::resolve_writable(root, &path).map_err(|err| err.to_string())?;
    // Read whole or not at all: a capped read written back would cut the file.
    if std::fs::metadata(&at).map_err(|err| err.to_string())?.len() > MEMORY_BYTES {
        return Err(format!("{file} is too large to change here"));
    }
    let was = std::fs::read_to_string(&at).map_err(|err| err.to_string())?;
    let found = was
        .lines()
        .position(&is)
        .ok_or_else(|| format!("the entry it replaces is no longer in {file}: discard it"))?;
    let text: String = was
        .lines()
        .enumerate()
        .filter_map(|(at, one)| match (at == found, now) {
            (false, _) => Some(format!("{one}\n")),
            (true, Some(now)) => Some(now.to_owned()),
            (true, None) => None,
        })
        .collect();
    write(root, file, &text)
}

/// The entries resting on files the default branch changed since the memory
/// was last re-checked, each with its memory file, and the commit it stands at.
pub(crate) struct Flagged {
    pub tip: String,
    pub entries: Vec<(&'static str, Entry)>,
}

/// What a wake re-checks. `None` without memory or a default branch; nothing
/// flagged the first time, which only notes where the default branch stands.
pub(crate) fn flagged(root: &Path) -> Option<Flagged> {
    if !root.join(DIR).is_dir() {
        return None;
    }
    let tip = devpit_git::landing::default_tip(root)?;
    let since = read_inside(root, &format!("{DIR}/{CHECKED}"), 128)
        .and_then(|said| devpit_git::commit_of(root, said.trim()).ok());
    let changed: Vec<String> = since
        .and_then(|since| devpit_git::changed_between(root, &since, &tip).ok())
        .into_iter()
        .flatten()
        .map(|one| one.path)
        .collect();
    let entries = FILES
        .iter()
        .flat_map(|file| entries(root, file).into_iter().map(move |one| (*file, one)))
        .filter(|(_, one)| {
            one.files()
                .iter()
                .any(|file| changed.iter().any(|path| path == file))
        })
        .collect();
    Some(Flagged { tip, entries })
}

/// The memory is re-checked up to the default branch's commit `tip`.
pub(crate) fn checked(root: &Path, tip: &str) -> Result<(), String> {
    write(root, CHECKED, &format!("{tip}\n"))
}

/// Logs how many review rounds card `source` took and how many findings came back.
pub(crate) fn measure(
    root: &Path,
    day: &str,
    source: &str,
    rounds: usize,
    repeated: usize,
) -> Result<(), String> {
    let said = serde_json::json!({
        "day": day,
        "card": source,
        "reviewRounds": rounds,
        "repeatedFindings": repeated,
    });
    let _held = WRITING
        .lock()
        .map_err(|_| "the memory is busy".to_owned())?;
    append(root, METRICS, &format!("{said}\n"))
}

/// Marks, in a proposal's source, what the agent's own read of the project proposed.
pub(crate) const ONBOARDING: &str = "onboarding";

/// When the agent last learned the project on its own: the day, kept here.
const LEARNED: &str = "learned";

/// Approves every proposal the agent's own read of the project made, each
/// to its memory file dated `day`, its source the files alone; the rest wait.
pub(crate) fn approve_onboarding(root: &Path, day: &str) -> Result<usize, String> {
    let _held = WRITING
        .lock()
        .map_err(|_| "the memory is busy".to_owned())?;
    let (learned, rest) = onboarding_split(root);
    for one in &learned {
        let (file, said, _) = aim(&one.text);
        let source = one.files().join(", ");
        append(root, file, &line(day, &source, said))?;
    }
    write(root, PROPOSED, &rest)?;
    Ok(learned.len())
}

/// Discards the proposals of the agent's own read still waiting, before it
/// reads the project again.
pub(crate) fn drop_onboarding(root: &Path) -> Result<usize, String> {
    let _held = WRITING
        .lock()
        .map_err(|_| "the memory is busy".to_owned())?;
    let (learned, rest) = onboarding_split(root);
    if !learned.is_empty() {
        write(root, PROPOSED, &rest)?;
    }
    Ok(learned.len())
}

/// The waiting proposals of the agent's own read, and the text of every
/// other line of `proposed.md`, kept as it was.
fn onboarding_split(root: &Path) -> (Vec<Entry>, String) {
    let was = read_inside(root, &format!("{DIR}/{PROPOSED}"), MEMORY_BYTES).unwrap_or_default();
    let (mut learned, mut rest) = (Vec::new(), String::new());
    for one in was.lines() {
        match entry(one) {
            Some(Ok(found)) if found.onboarding() => learned.push(found),
            _ => rest.push_str(&format!("{one}\n")),
        }
    }
    (learned, rest)
}

/// The agent learned the project on its own on `day`.
pub(crate) fn onboarded(root: &Path, day: &str) -> Result<(), String> {
    write(root, LEARNED, &format!("{day}\n"))
}

/// The day the agent last learned the project on its own.
pub(crate) fn learned_on(root: &Path) -> Option<String> {
    read_inside(root, &format!("{DIR}/{LEARNED}"), 64)
        .map(|said| said.trim().to_owned())
        .filter(|day| dated(day))
}

/// The entries of one memory file, as they are written.
pub(crate) fn kept(root: &Path, file: &str) -> Vec<Entry> {
    entries(root, file)
}

fn entries(root: &Path, file: &str) -> Vec<Entry> {
    read_inside(root, &format!("{DIR}/{file}"), MEMORY_BYTES)
        .unwrap_or_default()
        .lines()
        .filter_map(entry)
        .filter_map(Result::ok)
        .collect()
}

/// Writes a file of the memory, resolved inside the project first.
fn write(root: &Path, file: &str, text: &str) -> Result<(), String> {
    let at = devpit_core::paths::resolve_new(root, &format!("{DIR}/{file}"))
        .map_err(|err| err.to_string())?;
    std::fs::write(at, text).map_err(|err| err.to_string())
}

/// Appends to a file of the memory, resolved inside the project first.
fn append(root: &Path, file: &str, text: &str) -> Result<(), String> {
    use std::io::Write;
    let at = devpit_core::paths::resolve_new(root, &format!("{DIR}/{file}"))
        .map_err(|err| err.to_string())?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(at)
        .and_then(|mut out| out.write_all(text.as_bytes()))
        .map_err(|err| err.to_string())
}

#[cfg(test)]
#[path = "project_memory_tests.rs"]
mod tests;
