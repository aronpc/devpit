//! One level of the file tree, with git's opinion of each entry folded in.
//!
//! Apart from `projects.rs` because it is a different question: that file owns
//! the project row, and this one owns what the tree looks like from a given
//! checkout — including the rule that a folder shows the loudest change under
//! it, which is the only real decision in here.

use std::io::ErrorKind;

use devpit_core::locale::Locale;
use devpit_core::tree;
use devpit_rpc::{ErrorCode, FileNode, GitStatus, ProjectTree, RpcError};

use crate::projects::{checkout, locate, store};

pub(crate) fn tree_error(err: devpit_core::TreeError) -> RpcError {
    match err {
        // Its own code, not a generic one: the screen says something different
        // for a path that escaped than for a folder it could not read, and
        // this process runs terminals — reaching it is reaching the machine.
        devpit_core::TreeError::Outside { .. } => RpcError::forbidden(err.to_string()),
        // A folder deleted under the tree, or one the person cannot read, is
        // the disk's state and not a bug, so it stays out of the error report.
        devpit_core::TreeError::Unreadable { ref source, .. } => match source.kind() {
            ErrorKind::NotFound => RpcError::new(ErrorCode::NotFound, err.to_string()),
            ErrorKind::PermissionDenied => RpcError::forbidden(err.to_string()),
            // The report is never translated.
            _ => RpcError::internal(err.said(Locale::En)),
        },
        devpit_core::TreeError::AlreadyExists { .. } => {
            RpcError::new(ErrorCode::Conflict, err.to_string())
        }
    }
}

/// `project.tree` — one level of the file tree, from a given worktree.
///
/// One level rather than the whole tree: a monorepo has hundreds of thousands
/// of files and the screen draws only what is expanded. `path` is empty for
/// the root.
#[tauri::command]
#[specta::specta]
pub async fn project_tree(
    project_id: String,
    worktree_id: Option<String>,
    path: String,
) -> Result<ProjectTree, RpcError> {
    crate::off_main::blocking(move || project_tree_now(project_id, worktree_id, path)).await
}

/// [`project_tree`], on the calling thread.
pub(crate) fn project_tree_now(
    project_id: String,
    worktree_id: Option<String>,
    path: String,
) -> Result<ProjectTree, RpcError> {
    let store = store()?;
    let (_, root) = locate(&store, &project_id)?;
    let root = checkout(&root, worktree_id.as_deref());

    let read = status_of(&root);

    let nodes = tree::children(&root, &path)
        .map_err(tree_error)?
        .into_iter()
        .map(|entry| FileNode {
            status: mark(&entry, &read.paths, &read.ignored),
            children: entry.is_dir.then(Vec::new),
            name: entry.name,
            path: entry.path,
        })
        .collect();

    Ok(ProjectTree { nodes })
}

/// One checkout's status, and when the read that produced it began.
type Flight = std::sync::Mutex<Option<(std::time::Instant, devpit_git::Status)>>;

static FLIGHTS: std::sync::Mutex<
    Option<std::collections::HashMap<std::path::PathBuf, std::sync::Arc<Flight>>>,
> = std::sync::Mutex::new(None);

/// The checkout's git status, worked out once for the folders read together.
///
/// A tree refreshing — the window coming back, a file saved — asks for every
/// open folder at once, and each used to run a full `git status` of its own:
/// ten open folders were eleven walks of the whole repository. Now they
/// queue on one lock per checkout, and an answer is shared only with the
/// requests that were already waiting when it *started*: the folders of one
/// refresh arrive together and take one walk, and nothing read before a
/// request was made is ever handed to it — a stage, a commit, a checkout in
/// the terminal is seen by the next read. A status that fails is said on
/// stderr and read as clean, so the tree still draws.
fn status_of(root: &std::path::Path) -> devpit_git::Status {
    let asked = std::time::Instant::now();
    let flight = {
        let Ok(mut all) = FLIGHTS.lock() else {
            return devpit_git::status(root).unwrap_or_default();
        };
        std::sync::Arc::clone(
            all.get_or_insert_with(std::collections::HashMap::new)
                .entry(root.to_path_buf())
                .or_default(),
        )
    };
    let Ok(mut held) = flight.lock() else {
        return devpit_git::status(root).unwrap_or_default();
    };
    if let Some((started, status)) = held.as_ref() {
        if *started >= asked {
            return status.clone();
        }
    }
    let started = std::time::Instant::now();
    let read = devpit_git::status(root).unwrap_or_else(|err| {
        eprintln!("git status in {}: {err}", root.display());
        devpit_git::Status::default()
    });
    *held = Some((started, read.clone()));
    read
}

/// How much a status wants to be seen, when several are collapsed into one row.
///
/// A deletion outranks a modification outranks an addition: the further left,
/// the harder it is to undo by accident. Untracked is last because it is the
/// normal state of a working tree, not news.
fn loudness(status: GitStatus) -> u8 {
    match status {
        GitStatus::Deleted => 4,
        GitStatus::Modified => 3,
        GitStatus::Added => 2,
        GitStatus::Untracked => 1,
        GitStatus::Clean => 0,
        // Quieter than clean, and that is the point: an ignored row is one
        // this tree is showing you because it is on disk, not because it has
        // anything to do with the work.
        GitStatus::Ignored => 0,
    }
}

/// Whether an ignore rule reaches this path.
///
/// Git reports the **directory** a rule names, so `target` arrives once and
/// everything under it has to be worked out from the prefix. Asking the other
/// way round — a rule per file — is what cost 1.06 seconds when it was
/// measured.
fn ignored_here(path: &str, ignored: &std::collections::BTreeSet<String>) -> bool {
    ignored
        .iter()
        .any(|rule| path == rule || path.starts_with(&format!("{rule}/")))
}

/// A directory shows the loudest thing under it.
///
/// Otherwise a change three levels down is invisible until you have opened
/// three folders looking for it, which is the opposite of what the colour is
/// for. Ranked rather than first-found: the map is ordered by path, and taking
/// its first entry would show whichever file happens to sort earliest.
fn mark(
    entry: &tree::Entry,
    status: &std::collections::BTreeMap<String, GitStatus>,
    ignored: &std::collections::BTreeSet<String>,
) -> GitStatus {
    // Before anything else: a path an ignore rule reaches has no git status of
    // its own to report, and nothing under it does either.
    if ignored_here(&entry.path, ignored) {
        return GitStatus::Ignored;
    }
    if !entry.is_dir {
        return status.get(&entry.path).copied().unwrap_or(GitStatus::Clean);
    }

    let prefix = format!("{}/", entry.path);
    status
        .iter()
        .filter(|(path, _)| path.starts_with(&prefix))
        .map(|(_, status)| *status)
        .max_by_key(|status| loudness(*status))
        .unwrap_or(GitStatus::Clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository with no ignore rules, which is what the older tests here
    /// were written against.
    fn nothing() -> std::collections::BTreeSet<String> {
        std::collections::BTreeSet::new()
    }

    fn dir(path: &str) -> tree::Entry {
        tree::Entry {
            name: path.rsplit('/').next().unwrap_or(path).to_owned(),
            path: path.to_owned(),
            is_dir: true,
        }
    }

    fn file(path: &str) -> tree::Entry {
        tree::Entry {
            is_dir: false,
            ..dir(path)
        }
    }

    #[test]
    fn a_folder_gone_or_closed_to_us_is_not_an_internal_error() {
        let unreadable = |kind: ErrorKind| devpit_core::TreeError::Unreadable {
            path: "src".into(),
            source: kind.into(),
        };

        let gone = tree_error(unreadable(ErrorKind::NotFound));
        let closed = tree_error(unreadable(ErrorKind::PermissionDenied));
        let broken = tree_error(unreadable(ErrorKind::Other));

        assert_eq!(gone.code, ErrorCode::NotFound);
        assert_eq!(closed.code, ErrorCode::Forbidden);
        assert_eq!(broken.code, ErrorCode::Internal);
    }

    #[test]
    fn a_directory_shows_its_loudest_change_not_its_first() {
        // `a.rs` sorts before `z.rs`, so first-found would report Modified and
        // the deletion under the same folder would go unseen.
        let status = std::collections::BTreeMap::from([
            ("web/a.rs".to_owned(), GitStatus::Modified),
            ("web/z.rs".to_owned(), GitStatus::Deleted),
        ]);
        assert_eq!(mark(&dir("web"), &status, &nothing()), GitStatus::Deleted);
    }

    #[test]
    fn a_directory_with_nothing_under_it_is_clean() {
        let status = std::collections::BTreeMap::from([
            // Same prefix, different folder: `webbing` must not count as `web`.
            ("webbing/a.rs".to_owned(), GitStatus::Modified),
        ]);
        assert_eq!(mark(&dir("web"), &status, &nothing()), GitStatus::Clean);
    }

    #[test]
    fn a_path_an_ignore_rule_names_is_ignored() {
        let ignored: std::collections::BTreeSet<String> = ["target".to_owned()].into();
        assert_eq!(
            mark(&dir("target"), &Default::default(), &ignored),
            GitStatus::Ignored
        );
    }

    #[test]
    fn everything_under_an_ignored_folder_is_ignored_too() {
        // Git reports the folder the rule names, once. Everything below it has
        // to be worked out from the prefix — asking per file is what cost 1.06
        // seconds when it was measured.
        let ignored: std::collections::BTreeSet<String> = ["target".to_owned()].into();
        assert_eq!(
            mark(&file("target/debug/devpit"), &Default::default(), &ignored),
            GitStatus::Ignored
        );
    }

    #[test]
    fn a_name_that_merely_starts_the_same_is_not_ignored() {
        // `target` must not swallow `targets.rs`.
        let ignored: std::collections::BTreeSet<String> = ["target".to_owned()].into();
        assert_eq!(
            mark(&file("targets.rs"), &Default::default(), &ignored),
            GitStatus::Clean
        );
    }

    #[test]
    fn an_ignored_path_reports_no_change_of_its_own() {
        // A file inside an ignored folder is untracked as far as git's own
        // listing goes. Saying so would draw `target` in the colour that means
        // "new work you have not committed".
        let mut status = std::collections::BTreeMap::new();
        status.insert("target/x".to_owned(), GitStatus::Untracked);
        let ignored: std::collections::BTreeSet<String> = ["target".to_owned()].into();
        assert_eq!(
            mark(&file("target/x"), &status, &ignored),
            GitStatus::Ignored
        );
    }
}
