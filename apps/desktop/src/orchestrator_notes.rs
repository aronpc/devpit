//! An orchestrator's log, written by devpit for it.
//!
//! Its brief asks it to keep `context/sessions.md` and `context/preferences.md`
//! up to date, and it did — with `echo >>` and heredocs, a hundred times a
//! week. One call does the same: a dated line, appended, nothing else touched.

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The logs an orchestrator keeps, by the name a call gives.
const LOGS: [&str; 2] = ["sessions", "preferences"];
/// How long one entry may be, in characters.
const LONGEST: usize = 4000;

/// The file a log name stands for, inside the orchestrator's folder. Only
/// the names above: the name comes from the agent and is never a path.
pub(crate) fn log_file(folder: &Path, log: &str) -> Option<PathBuf> {
    LOGS.contains(&log)
        .then(|| folder.join("context").join(format!("{log}.md")))
}

/// One entry as it is written: a dated bullet, its later lines indented
/// under it.
pub(crate) fn entry(day: &str, text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > LONGEST {
        return Err(format!("an entry is between 1 and {LONGEST} characters"));
    }
    if text
        .chars()
        .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
    {
        return Err("an entry cannot hold control characters".to_owned());
    }
    let body = text.lines().collect::<Vec<_>>().join("\n  ");
    Ok(format!("- {day} — {body}\n"))
}

/// A devpit project, as a rules heading names it: by id or by name.
#[derive(Clone, Copy)]
pub(crate) struct ProjectName<'a> {
    pub id: &'a str,
    pub name: &'a str,
}

/// Where in the preferences an entry goes. Only the log is written
/// unasked: the rules go with every brief, so they change when asked.
#[derive(Clone, Copy)]
pub(crate) enum Section<'a> {
    Log,
    Rules,
    Project(ProjectName<'a>),
}

/// The heading `devpit_log` keeps preferences under once the rules are last.
const LOG: &str = "## Log";

/// One read-and-write of the preferences at a time, so no entry is lost.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Appends an entry to one of the orchestrator's logs; answers what it wrote.
pub(crate) fn note(folder: &Path, log: &str, text: &str, into: Section) -> Result<String, String> {
    let file =
        log_file(folder, log).ok_or_else(|| format!("the logs are {}", LOGS.join(" and ")))?;
    if log != "preferences" && !matches!(into, Section::Log) {
        return Err("`section` and `project` are for the preferences log".to_owned());
    }
    let day = devpit_agentcli::spend_scan::day_of(devpit_core::reports::now() as i64);
    let line = entry(&day, text)?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if log != "preferences" {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .and_then(|mut opened| opened.write_all(line.as_bytes()))
            .map_err(|err| err.to_string())?;
        return Ok(line);
    }
    let _held = WRITING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.to_string()),
    };
    let after = placed(&before, &line, into);
    let fresh = file.with_extension("md.writing");
    std::fs::write(&fresh, after)
        .and_then(|()| std::fs::rename(&fresh, &file))
        .map_err(|err| err.to_string())?;
    // In view from the orchestrator's next turn. The entry is written either
    // way, and the next opening writes the copy again.
    let _ = crate::orchestrator_view::keep(folder);
    Ok(line)
}

fn is_heading(line: &str) -> bool {
    line.starts_with("## ") || line.starts_with("# ")
}

/// The preferences with `line` in its section: at the end of the first one
/// `into` names, or of a new one at the end of the file. The log takes the
/// end of the file as before, unless that is inside some rules.
fn placed(text: &str, line: &str, into: Section) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let wanted = |heading: &str| match into {
        Section::Log => heading.trim().eq_ignore_ascii_case(LOG),
        Section::Rules => rules_heading(heading) == Some(None),
        Section::Project(project) => {
            matches!(rules_heading(heading), Some(Some(label)) if names(label, project))
        }
    };
    if let Some(at) = lines.iter().position(|one| is_heading(one) && wanted(one)) {
        return inserted(&lines, at, line);
    }
    let last_is_rules = lines
        .iter()
        .rev()
        .find(|one| is_heading(one))
        .is_some_and(|heading| rules_heading(heading).is_some());
    if matches!(into, Section::Log) && !last_is_rules {
        let mut kept = text.to_owned();
        if !kept.is_empty() && !kept.ends_with('\n') {
            kept.push('\n');
        }
        return kept + line;
    }
    let heading = match into {
        Section::Log => LOG.to_owned(),
        Section::Rules => "## Rules for sessions".to_owned(),
        Section::Project(project) => format!("## Rules for sessions: {}", project.name),
    };
    let mut out = text.trim_end().to_owned();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    format!("{out}{heading}\n\n{line}")
}

/// `line` after the last line of the section headed at `at`.
fn inserted(lines: &[&str], at: usize, line: &str) -> String {
    let end = lines[at + 1..]
        .iter()
        .position(|one| is_heading(one))
        .map_or(lines.len(), |found| at + 1 + found);
    let mut last = end;
    while last > at + 1 && lines[last - 1].trim().is_empty() {
        last -= 1;
    }
    let mut out: Vec<&str> = lines[..last].to_vec();
    if last == at + 1 {
        out.push("");
    }
    out.extend(line.lines());
    if last == end && end < lines.len() {
        out.push("");
    }
    out.extend(&lines[last..]);
    out.join("\n") + "\n"
}

/// The heading in `context/preferences.md` whose lines go with every brief.
const RULES: &str = "## rules for sessions";

/// Whether `line` heads rules that go to sessions, every project's or one's.
pub(crate) fn is_rules_heading(line: &str) -> bool {
    rules_heading(line).is_some()
}

/// A rules heading: `Some(None)` for every session's, `Some(Some(label))`
/// for `## Rules for sessions: <label>`, the projects it names.
fn rules_heading(line: &str) -> Option<Option<&str>> {
    let line = line.trim();
    if !line.get(..RULES.len())?.eq_ignore_ascii_case(RULES) {
        return None;
    }
    let rest = line[RULES.len()..].trim_start();
    if rest.is_empty() {
        return Some(None);
    }
    let label = rest.strip_prefix(':')?.trim();
    (!label.is_empty()).then_some(Some(label))
}

/// Whether a heading's label names the project: its id or name, alone or in
/// a list separated by commas.
fn names(label: &str, project: ProjectName) -> bool {
    let one = |said: &str| {
        let said = said.trim();
        said == project.id || said.eq_ignore_ascii_case(project.name)
    };
    one(label) || label.split(',').any(one)
}

/// The bodies of the sections whose heading `wanted` takes, each up to the
/// next heading, the empty ones left out.
fn sections_in(text: &str, wanted: impl Fn(&str) -> bool) -> Vec<String> {
    let mut found: Vec<Vec<&str>> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if is_heading(line) {
            inside = wanted(line);
            if inside {
                found.push(Vec::new());
            }
        } else if inside {
            if let Some(body) = found.last_mut() {
                body.push(line);
            }
        }
    }
    found
        .into_iter()
        .map(|body| body.join("\n").trim().to_owned())
        .filter(|body| !body.is_empty())
        .map(|body| body.chars().take(LONGEST).collect())
        .collect()
}

/// The person's standing rules for every session the orchestrator starts:
/// the section under `## Rules for sessions` in its preferences, up to the
/// next heading. Nothing, when there is none.
pub(crate) fn rules_in(text: &str) -> Option<String> {
    let mut lines = text
        .lines()
        .skip_while(|line| rules_heading(line) != Some(None));
    lines.next()?;
    let body: Vec<&str> = lines.take_while(|line| !is_heading(line)).collect();
    let body = body.join("\n");
    let body = body.trim();
    (!body.is_empty()).then(|| body.chars().take(LONGEST).collect())
}

/// One project's own rules: every `## Rules for sessions: <label>` section
/// whose label names it, one after the other.
fn project_rules_in(text: &str, project: ProjectName) -> Option<String> {
    let bodies = sections_in(
        text,
        |heading| matches!(rules_heading(heading), Some(Some(label)) if names(label, project)),
    );
    (!bodies.is_empty()).then(|| bodies.join("\n\n"))
}

/// The longest brief a session is started with, rules included.
const LONGEST_BRIEF: usize = 8000;

/// Whom a session reports to: the orchestrator's own name, which outlives a
/// restart of its chat where the one the CLI makes up does not.
pub(crate) fn report_to(folder: &Path, by_bus: bool) -> Option<String> {
    let name = crate::orchestrator::address(folder)?;
    Some(report_line(&name, !by_bus))
}

/// How a session reaches `name`: Claude Code's own SendMessage, or devpit's
/// tool for a session on a CLI without it.
pub(crate) fn report_line(name: &str, send_message: bool) -> String {
    let how = match send_message {
        true => "SendMessage to that name",
        false => "message it with devpit_session_message, `to` left out,",
    };
    format!(
        "The orchestrator that started you is `{name}`: {how} to report back. It keeps the name when its chat restarts."
    )
}

/// A session's brief, with whom to report to, the person's standing rules
/// and those of the session's own project after it — cut to what still
/// fits, so they never make a brief too long to start.
/// A session that reports `by_bus` is one devpit keeps, with no SendMessage
/// to the orchestrator.
pub(crate) fn briefed(
    folder: &Path,
    prompt: &str,
    project: Option<ProjectName>,
    by_bus: bool,
) -> String {
    if prompt.trim().is_empty() {
        return prompt.to_owned();
    }
    let preferences = log_file(folder, "preferences")
        .and_then(|file| std::fs::read_to_string(file).ok())
        .unwrap_or_default();
    let mut brief = prompt.trim_end().to_owned();
    let added = [
        ("Reporting back".to_owned(), report_to(folder, by_bus)),
        ("Rules for sessions".to_owned(), rules_in(&preferences)),
        (
            format!("Rules for sessions: {}", project.map_or("", |one| one.name)),
            project.and_then(|one| project_rules_in(&preferences, one)),
        ),
    ];
    for (heading, body) in added {
        let Some(body) = body else { continue };
        let used = brief.chars().count() + heading.chars().count() + 64;
        let Some(room) = LONGEST_BRIEF.checked_sub(used).filter(|room| *room > 0) else {
            break;
        };
        let body: String = body.chars().take(room).collect();
        brief = format!("{brief}\n\n## {heading}\n\n{body}");
    }
    if brief == prompt.trim_end() {
        prompt.to_owned()
    } else {
        brief
    }
}

#[cfg(test)]
#[path = "orchestrator_notes_tests.rs"]
mod tests;
