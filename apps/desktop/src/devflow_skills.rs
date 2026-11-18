//! The skills of an active project, for the MCP server: their entries to
//! list, and one skill's text to read, to call or to start a stage with.
//!
//! Resolved in the project's root, never a card's worktree, so every card
//! of a project follows the same flow.

use std::path::{Path, PathBuf};

use devflow_engine::layers::Team;
use devflow_engine::skills::{self, Layer, Skill};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Each part `devpit_agentapi::skills::PARTS` names, beside its skill's
/// SKILL.md: devflow's, in devpit's words.
fn parts() -> &'static [(&'static str, &'static str, &'static str)] {
    crate::devflow_dialect::DEVPIT.assets.parts
}

/// The most of a layer's part read.
const LONGEST_PART: u64 = 64 * 1024;

/// devflow's `skills/get` for these params, when the devflow here serves it.
pub(crate) type Asking<'a> =
    &'a dyn Fn(Value) -> Option<Result<Value, crate::devflow_host::Failed>>;

pub(crate) fn respond(
    method: &str,
    root: &Path,
    stages: &[&str],
    params: &Value,
    by_devflow: Asking,
) -> Result<Value, String> {
    let personal = personal()?;
    // A team layer that applies replaces a built-in skill by its name too.
    let teams = crate::devflow::layered(root)
        .map(|layered| layered.applied)
        .unwrap_or_default();
    match method {
        "devflow_skills" => Ok(listed(root, &personal, &teams, stages)),
        _ => one(root, &personal, &teams, params, by_devflow),
    }
}

/// The person's own skills, under this devpit's home.
pub(crate) fn personal() -> Result<PathBuf, String> {
    let root = devpit_core::Store::root().map_err(|err| err.to_string())?;
    Ok(devpit_core::home::devflow_skills(&root))
}

/// Each skill offered, without its text. One whose file is refused is named
/// with why, not left out unsaid.
fn listed(root: &Path, personal: &Path, teams: &[Team], stages: &[&str]) -> Value {
    let (mut skills, mut refused) = (Vec::new(), Vec::new());
    for name in skills::offered(stages) {
        match skills::resolve_with_in(&crate::devflow_dialect::DEVPIT, root, personal, teams, name)
        {
            Ok(skill) => skills.push(entry(&skill)),
            Err(why) => refused.push(json!({ "name": name, "why": why })),
        }
    }
    json!({ "skills": skills, "refused": refused })
}

/// One skill, by `name` or `uri`, with its text: any stage's, asked by
/// name, so a stage can be run on demand where devflow.yaml leaves it off.
/// With `part`, or a part's URI, that part of it instead.
fn one(
    root: &Path,
    personal: &Path,
    teams: &[Team],
    params: &Value,
    by_devflow: Asking,
) -> Result<Value, String> {
    let asked = ["name", "uri"]
        .iter()
        .find_map(|key| params.get(key).and_then(Value::as_str))
        .ok_or("pass the skill's `name`, such as devflow-plan")?;
    let (asked, part) = match part_uri(asked) {
        Some((name, part)) => (name, Some(part)),
        None => (asked, params.get("part").and_then(Value::as_str)),
    };
    let name = skills::named(asked).ok_or_else(|| {
        format!(
            "`{asked}` is not a Devflow skill: {}",
            skills::names().collect::<Vec<_>>().join(", ")
        )
    })?;
    // devflow resolves the layers it knows; a team layer devpit fetched is
    // devpit's to read.
    if teams.is_empty() {
        let mut asked = json!({ "name": name });
        if let Some(part) = part {
            asked["part"] = json!(part);
        }
        if let Some(said) = by_devflow(asked).and_then(Result::ok) {
            if let Some(answer) = from_devflow(&said, part) {
                return Ok(answer);
            }
        }
    }
    let skill =
        skills::resolve_with_in(&crate::devflow_dialect::DEVPIT, root, personal, teams, name)?;
    if let Some(part) = part {
        return part_of(&skill, part);
    }
    let mut said = entry(&skill);
    said["text"] = json!(skill.text);
    Ok(said)
}

/// The layer devflow names by its word.
fn layer_of(word: &str) -> Option<Layer> {
    [Layer::Project, Layer::Team, Layer::Personal, Layer::BuiltIn]
        .into_iter()
        .find(|layer| layer.word() == word)
}

/// devflow's answer to `skills/get` as devpit answers a skill or a part;
/// `None` when it says too little, from a devflow before frontmatter.
fn from_devflow(said: &Value, part: Option<&str>) -> Option<Value> {
    let name = skills::named(said["name"].as_str()?)?;
    let text = said["text"].as_str()?.to_owned();
    let layer = layer_of(said["source"]["layer"].as_str()?)?;
    let file = said["source"]["file"].as_str();
    let stage = crate::devflow_dialect::DEVPIT
        .assets
        .skills
        .iter()
        .find(|(one, ..)| *one == name)
        .and_then(|(_, stage, _)| *stage);
    if let Some(part) = part {
        return Some(json!({
            "name": name,
            "part": part,
            "stage": stage,
            "layer": layer.word(),
            "file": file,
            "skill": { "resources": [resource(name, part, &text)] },
            "text": text,
        }));
    }
    let skill = Skill {
        name,
        stage,
        layer,
        file: file.map(PathBuf::from),
        frontmatter: said
            .get("frontmatter")
            .filter(|one| one.is_object())?
            .clone(),
        text,
    };
    let mut answer = entry(&skill);
    answer["text"] = json!(skill.text);
    Some(answer)
}

/// The skill's JSON, its entry listing its parts after its SKILL.md.
fn entry(skill: &Skill) -> Value {
    let mut said = skill.to_json();
    if let Some(resources) = said["skill"]["resources"].as_array_mut() {
        for (_, part, text) in parts().iter().filter(|(of, ..)| *of == skill.name) {
            resources.push(resource(skill.name, part, text));
        }
    }
    said
}

fn resource(name: &str, part: &str, text: &str) -> Value {
    json!({
        "uri": format!("skill://devflow/{name}/{part}.md"),
        "digest": format!("sha256:{:x}", Sha256::digest(text.as_bytes())),
        "size": text.len(),
    })
}

/// `skill://devflow/<skill>/<part>.md`, for a part, not a SKILL.md.
fn part_uri(uri: &str) -> Option<(&str, &str)> {
    let (name, file) = uri.strip_prefix("skill://devflow/")?.split_once('/')?;
    let part = file.strip_suffix(".md")?;
    parts()
        .iter()
        .any(|(of, one, _)| *of == name && *one == part)
        .then_some((name, part))
}

/// A part of `skill`: the layer's own beside its SKILL.md, else devflow's.
fn part_of(skill: &Skill, part: &str) -> Result<Value, String> {
    let ours: Vec<&(&str, &str, &str)> = parts()
        .iter()
        .filter(|(of, ..)| *of == skill.name)
        .collect();
    if ours.is_empty() {
        return Err(format!(
            "{} has no parts: ask for it without `part`",
            skill.name
        ));
    }
    let &&(_, part, built_in) = ours
        .iter()
        .find(|(_, one, _)| *one == part)
        .ok_or_else(|| {
            let names: Vec<&str> = ours.iter().map(|(_, one, _)| *one).collect();
            format!(
                "`{part}` is not a part of {}: {}",
                skill.name,
                names.join(", ")
            )
        })?;
    let layered = skill.file.as_ref().and_then(|file| {
        let beside = file.with_file_name(format!("{part}.md"));
        let meta = std::fs::symlink_metadata(&beside).ok()?;
        (meta.is_file() && meta.len() <= LONGEST_PART)
            .then(|| {
                std::fs::read_to_string(&beside)
                    .ok()
                    .map(|text| (beside, text))
            })
            .flatten()
    });
    let (layer, file, text) = match layered {
        Some((file, text)) => (skill.layer.word(), Some(file.display().to_string()), text),
        None => ("built-in", None, built_in.to_owned()),
    };
    Ok(json!({
        "name": skill.name,
        "part": part,
        "stage": skill.stage,
        "layer": layer,
        "file": file,
        "skill": { "resources": [resource(skill.name, part, &text)] },
        "text": text,
    }))
}

#[cfg(test)]
#[path = "devflow_skills_tests.rs"]
mod tests;
