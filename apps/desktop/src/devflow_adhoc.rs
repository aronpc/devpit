//! `devflow_review` without a card: a pull request, a range or a worktree
//! of the project reviewed on demand, wherever Devflow is on in Settings —
//! with the project's devflow.yaml when it has one, else the person's layer.
//!
//! A pull request or a range is read in a detached worktree under devpit's
//! folder for the project, removed once the review is: nothing the person
//! has checked out moves. The run is kept under `reviews/<pr-or-range>/` and
//! attached; no card is commented.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use devflow_engine::adhoc::{self, Standalone, Subject};
use devflow_engine::evidence::Card;
use devflow_engine::review::{self, Asked, Given, Reach};
use devflow_engine::runs::{self, Runs};
use devflow_engine::{layers, Config, State};
use devpit_core::locale::Locale;
use devpit_rpc::{Declared, Project};

use crate::devflow_dialect::dialect_in;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::AppHandle;

/// What the agent's wait keeps for attaching and answering once the reviewers stop.
const SPARE: Duration = Duration::from_secs(30);

/// What `gh pr view` says of a pull request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Pull {
    pub base_ref_name: String,
    pub head_ref_oid: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
}

/// How a pull request is read, which a test answers without GitHub.
pub(crate) type Ask<'a> = &'a dyn Fn(&Path, u64) -> Result<Pull, String>;

pub(crate) fn respond(
    locale: Locale,
    _app: Option<&AppHandle>,
    project: &Project,
    state: &State,
    asked: Standalone,
    profile: Option<&Declared>,
) -> Result<Value, String> {
    let program = crate::devflow_review::program_for(profile);
    let reach = |agent| crate::devflow_review::reach_as(agent, profile, program.as_deref());
    respond_with(locale, project, state, asked, &reach, &pull, profile)
}

/// A pull request of the project's GitHub repository; other hosts are not read.
fn pull(root: &Path, number: u64) -> Result<Pull, String> {
    let number = number.to_string();
    let fields = "baseRefName,headRefOid,title,body";
    let said = crate::island_checks::gh(root, &["pr", "view", &number, "--json", fields])
        .ok_or("gh could not read that pull request: a review of a pr needs gh signed in to the project's GitHub repository")?;
    serde_json::from_str(&said).map_err(|err| format!("gh answered something else: {err}"))
}

/// What the review runs on: the project's devflow.yaml over its layers, else
/// the person's own layer over the defaults.
pub(crate) fn config_of(state: &State) -> Result<Config, String> {
    match state {
        State::Valid(_, config) => Ok((**config).clone()),
        State::Missing => {
            layers::personal_only(&crate::devflow::home()).map_err(|invalid| invalid.to_string())
        }
        State::Invalid(..) => {
            Err("devflow.yaml does not read: devpit_devflow_status says why".to_owned())
        }
    }
}

/// `path` when it is the project's root or one of its git worktrees.
fn worktree_of(root: &Path, path: &str) -> Result<PathBuf, String> {
    let refused = || format!("{path} is not a worktree of this project");
    let at = Path::new(path).canonicalize().map_err(|_| refused())?;
    let listed = devpit_git::worktrees(root, &[]).map_err(|err| err.to_string())?;
    listed
        .iter()
        .filter_map(|tree| Path::new(&tree.path).canonicalize().ok())
        .find(|tree| *tree == at)
        .ok_or_else(refused)
}

/// A detached worktree, removed when the review is done with it.
struct Detached<'a> {
    root: &'a Path,
    at: PathBuf,
}

impl Drop for Detached<'_> {
    fn drop(&mut self) {
        devpit_git::remove_detached(self.root, &self.at);
    }
}

/// What is reviewed, resolved: BASE, HEAD, a title, the criteria, and the
/// worktree it is read in when it is the person's.
struct Resolved {
    base: String,
    head: String,
    title: String,
    criteria: Vec<String>,
    tree: Option<PathBuf>,
}

fn resolved(root: &Path, asked: &Standalone, ask: Ask) -> Result<Resolved, String> {
    let said = |err: devpit_git::GitError| err.to_string();
    match &asked.subject {
        Subject::Pr(number) => {
            let pull = ask(root, *number)?;
            if !devpit_git::plain_branch(&pull.base_ref_name) {
                return Err("the pull request's base is not a plain branch".to_owned());
            }
            if devpit_git::commit_of(root, &pull.head_ref_oid).is_err() {
                devpit_git::fetch_pull(root, *number).map_err(said)?;
            }
            let head = devpit_git::commit_of(root, &pull.head_ref_oid).map_err(said)?;
            let base =
                devpit_git::merge_base(root, &format!("origin/{}", pull.base_ref_name), &head)
                    .or_else(|_| devpit_git::merge_base(root, &pull.base_ref_name, &head))
                    .map_err(said)?;
            Ok(Resolved {
                base,
                head,
                title: format!("PR #{number}: {}", pull.title),
                criteria: adhoc::pr_criteria(&asked.criteria, &pull.title, &pull.body),
                tree: None,
            })
        }
        Subject::Range { base, head } => {
            let head = devpit_git::commit_of(root, head).map_err(said)?;
            Ok(Resolved {
                base: devpit_git::merge_base(root, base, &head).map_err(said)?,
                title: format!("{base}..{}", &head[..12]),
                head,
                criteria: asked.criteria.clone(),
                tree: None,
            })
        }
        Subject::Path { path, base } => {
            let tree = worktree_of(root, path)?;
            Ok(Resolved {
                base: devpit_git::review_base(&tree, base.as_deref()).map_err(said)?,
                head: devpit_git::head_of(&tree).map_err(said)?,
                title: format!("the worktree {}", tree.display()),
                criteria: asked.criteria.clone(),
                tree: Some(tree),
            })
        }
    }
}

pub(crate) fn respond_with(
    locale: Locale,
    project: &Project,
    state: &State,
    asked: Standalone,
    reach: Reach,
    ask: Ask,
    profile: Option<&Declared>,
) -> Result<Value, String> {
    let config = config_of(state)?;
    let env = crate::devflow_profile::env_of(profile);
    let root = Path::new(&project.root_path);
    let store = crate::projects::store().map_err(|err| err.message)?;
    let home = crate::projects::project_home(&project.id).map_err(|err| err.message)?;
    // A hand-back reads only the run's record: a pull request is not asked again.
    let resolved = match (&asked.subject, &asked.run) {
        (Subject::Pr(_), Some(_)) => None,
        _ => Some(resolved(root, &asked, ask)?),
    };
    let label = adhoc::label(
        &asked.subject,
        resolved.as_ref().map_or("", |at| at.base.as_str()),
        resolved.as_ref().map_or("", |at| at.head.as_str()),
    );
    let at = SystemTime::now();
    // Read where the person's checkout cannot move: the worktree they named,
    // or a detached one of devpit's own.
    let mut detached = None;
    let reading = match &resolved {
        Some(Resolved {
            tree: Some(tree), ..
        }) => tree.clone(),
        Some(resolved) if asked.run.is_none() => {
            let tree = home
                .review_trees()
                .join(format!("{label}-{}", runs::stamp(at)));
            devpit_git::add_detached(root, &tree, &resolved.head).map_err(|err| err.to_string())?;
            detached = Some(Detached {
                root,
                at: tree.clone(),
            });
            tree
        }
        _ => root.to_path_buf(),
    };
    let git = match (&resolved, &asked.run) {
        (Some(resolved), None) => Some(
            devpit_git::review_diff(&reading, &resolved.base, &resolved.head)
                .map_err(|err| err.to_string())?,
        ),
        _ => None,
    };
    let criteria = resolved
        .as_ref()
        .map_or_else(Vec::new, |at| at.criteria.clone());
    let body = adhoc::body_of(&criteria);
    let title = resolved
        .as_ref()
        .map_or_else(|| label.clone(), |at| at.title.clone());
    let runs = Runs {
        home: home.devflow_reviews(),
        checkout: reading.clone(),
        dialect: dialect_in(locale),
    };
    let secrets = crate::kept_out::what_devpit_gave(&store);
    let redact = |text: &str| crate::kept_out::kept_out(text, &secrets);
    let teams = crate::devflow::layered(root)
        .map(|layered| layered.applied)
        .unwrap_or_default();
    let personal = devpit_core::Store::root()
        .map(|home| devpit_core::home::devflow_reviewers(&home))
        .ok();
    let lines_at = |commit: &str, file: &str| {
        devpit_git::lines_at(root, commit, file, crate::devflow_review::MOST_COUNTED)
            .map_err(|err| err.to_string())
    };
    let as_card = Asked {
        card: label.clone(),
        base: None,
        run: asked.run.clone(),
        reports: asked.reports.clone(),
        previous: None,
        verified: Vec::new(),
    };
    let done = review::reviewed(&Given {
        root: &reading,
        runs: &runs,
        config: &config,
        asked: &as_card,
        card: Card {
            id: &label,
            title: &title,
            body: &body,
        },
        git: match (&resolved, &git) {
            (Some(resolved), Some(read)) => Some(review::Git {
                base: &resolved.base,
                head: &resolved.head,
                files: &read.files,
                diff: &read.diff,
            }),
            _ => None,
        },
        brief: None,
        evidence: None,
        env: &env,
        at,
        budget: devpit_agentapi::client::wait_for("devflow_review").saturating_sub(SPARE),
        reach,
        redact: &redact,
        jev: None,
        personas: review::Personas {
            dialect: dialect_in(locale),
            personal: personal.as_deref(),
            teams: &teams,
        },
        previous: None,
        lines_at: &lines_at,
        retro: asked.retro,
    });
    let read_detached = detached
        .as_ref()
        .and(resolved.as_ref())
        .map(|at| at.head.clone());
    drop(detached);
    let done = done?;

    let artifacts = home.artifacts();
    let folder = format!("devflow/reviews/{label}");
    let under = format!("{folder}/{}", done.stamp);
    let attached = runs::attach(Path::new(&done.folder), &done.files(), &artifacts, &under);
    runs::prune(&home.devflow_reviews().join(&label), runs::KEPT);
    runs::prune(&artifacts.join(&folder), runs::KEPT);
    let mut answer = review::summary(&done);
    if let Some(fields) = answer.as_object_mut() {
        fields.remove("card");
    }
    answer["reviewed"] = json!(label);
    answer["next"] = json!(adhoc::next_in(
        dialect_in(locale),
        &asked.subject,
        &label,
        &done.stamp,
        &done.pending,
        read_detached.as_deref(),
        done.retro
    ));
    answer["attached"] =
        json!({ "under": under, "files": attached.names.len(), "left_out": attached.left_out });
    Ok(answer)
}

#[cfg(test)]
#[path = "devflow_adhoc_tests.rs"]
pub(crate) mod tests;
