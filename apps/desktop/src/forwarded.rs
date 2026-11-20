//! Requests an orchestrator forwards to another orchestrator.
//!
//! The words of another orchestrator approve nothing, and what they ask is
//! for the person to decide, so a forwarded request starts no turn: it is
//! kept for the conversation of the destination that will show it, and waits
//! there for the person.
//!
//! Held in memory, as `reply_drafts` holds its own, but not in its place: a
//! draft there replaces the one before it and goes to a terminal, while two
//! requests are two, told apart by who sent them and by the id they carry.
//! That id is what lets the sender write again after an error or a delay and
//! leave one request behind.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

use devpit_rpc::{
    ErrorCode, ForwardedRequest, ForwardedRequests, ForwardedSent, Project, RpcError, Thread,
};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

/// What the window hears when a destination's requests change; it carries
/// the destination's project id.
pub(crate) const FORWARDED: &str = "orchestrator:forwarded";

/// The most a request may be, in characters: what a message may be.
pub(crate) const LONGEST: usize = crate::session_bus::LONGEST;

/// The most requests waiting for one destination.
const MOST: usize = 50;

/// The most an id may be, in characters.
const LONGEST_ID: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestState {
    Pending,
    Sending(u64),
}

#[derive(Clone)]
struct Kept {
    conversation: String,
    text: String,
    state: RequestState,
}

/// By the destination project, the sender's address and the request's id.
type Key = (String, String, String);

#[derive(Default)]
struct Requests {
    held: HashMap<Key, Kept>,
    generation: u64,
}

impl Requests {
    fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }
}

fn requests() -> &'static Mutex<Requests> {
    static REQUESTS: OnceLock<Mutex<Requests>> = OnceLock::new();
    REQUESTS.get_or_init(Mutex::default)
}

/// Keeps `text`, sent by `origin` as request `id` to the orchestrator of
/// project `destination`, for its `conversation`. Answers whether it was
/// kept already: the same request sent again leaves one, and other words
/// under the same id are a conflict, which changes nothing.
pub(crate) fn keep(
    destination: &str,
    origin: &str,
    id: &str,
    conversation: &str,
    text: &str,
) -> Result<bool, String> {
    let (id, text) = checked(id, text)?;
    let mut held = requests()
        .lock()
        .map_err(|_| "the requests could not be read".to_owned())?;
    let key = (destination.to_owned(), origin.to_owned(), id.to_owned());
    if let Some(kept) = held.held.get(&key) {
        return match kept.text == text {
            true => Ok(true),
            false => Err(format!(
                "request {id} was sent already with other words and stays as it was: use another id for a different request"
            )),
        };
    }
    if held
        .held
        .keys()
        .filter(|(to, ..)| to == destination)
        .count()
        >= MOST
    {
        return Err(format!(
            "{MOST} requests wait for that orchestrator already: it has to deal with them before more come"
        ));
    }
    held.held.insert(
        key,
        Kept {
            conversation: conversation.to_owned(),
            text: text.to_owned(),
            state: RequestState::Pending,
        },
    );
    Ok(false)
}

/// `id` and `text` trimmed, when a request may carry them.
fn checked<'a>(id: &'a str, text: &'a str) -> Result<(&'a str, &'a str), String> {
    let (id, text) = (id.trim(), text.trim());
    if id.is_empty() || id.chars().count() > LONGEST_ID {
        return Err(format!(
            "a request to another orchestrator needs an `id` of 1 to {LONGEST_ID} characters: send it again with the same id after an error"
        ));
    }
    if text.is_empty() || text.chars().count() > LONGEST {
        return Err(format!("a request is between 1 and {LONGEST} characters"));
    }
    Ok((id, text))
}

/// What waits for project `destination`, oldest sender first.
pub(crate) fn listed(destination: &str) -> ForwardedRequests {
    let held = requests().lock();
    let mut requests: Vec<ForwardedRequest> = held
        .as_deref()
        .map(|held| held.held.iter())
        .into_iter()
        .flatten()
        .filter(|((to, ..), _)| to == destination)
        .map(|((_, from, id), kept)| ForwardedRequest {
            from: from.clone(),
            id: id.clone(),
            conversation: kept.conversation.clone(),
            text: kept.text.clone(),
        })
        .collect();
    requests.sort_by(|a, b| (&a.from, &a.id).cmp(&(&b.from, &b.id)));
    ForwardedRequests { requests }
}

/// Lets go of request `id` from `origin` for project `destination`: the
/// others stay. Answers whether there was one.
pub(crate) fn dropped(destination: &str, origin: &str, id: &str) -> Result<bool, RpcError> {
    let mut held = requests().lock().map_err(|_| {
        RpcError::new(
            ErrorCode::Internal,
            "the requests could not be read".to_owned(),
        )
    })?;
    let key = (destination.to_owned(), origin.to_owned(), id.to_owned());
    if held
        .held
        .get(&key)
        .is_some_and(|request| matches!(request.state, RequestState::Sending(_)))
    {
        return Err(RpcError::new(
            ErrorCode::Conflict,
            "that request is being sent".to_owned(),
        ));
    }
    Ok(held.held.remove(&key).is_some())
}

/// `forwarded.list` — what other orchestrators forwarded to this one.
#[tauri::command]
#[specta::specta]
pub async fn forwarded_list(project_id: String) -> Result<ForwardedRequests, RpcError> {
    crate::off_main::blocking(move || Ok(listed(&project_id))).await
}

/// `forwarded.drop` — the person lets go of one request without sending it.
#[tauri::command]
#[specta::specta]
pub async fn forwarded_drop(
    app: AppHandle,
    project_id: String,
    from: String,
    id: String,
) -> Result<(), RpcError> {
    crate::off_main::blocking(move || {
        dropped(&project_id, &from, &id)?;
        let _ = app.emit(FORWARDED, &project_id);
        Ok(())
    })
    .await
}

/// What the turn of request `id` from `from` says, under the notice that it
/// is another orchestrator's: `text` is what the person sends, as edited.
pub(crate) fn prompt_of(from: &str, text: &str) -> String {
    format!(
        "{} {}",
        devpit_agentcli::person::forwarded(from),
        text.trim()
    )
}

/// A request reserved for one turn, identified by its generation.
#[derive(Debug)]
pub(crate) struct Reserved {
    generation: u64,
    prompt: String,
}

/// Reserves request `id` atomically and returns its prompt. A request already
/// being sent cannot be sent or discarded again.
pub(crate) fn to_send(
    destination: &str,
    from: &str,
    id: &str,
    text: &str,
) -> Result<Reserved, RpcError> {
    let (id, text) = checked(id, text).map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
    let mut held = requests().lock().map_err(|_| {
        RpcError::new(
            ErrorCode::Internal,
            "the requests could not be read".to_owned(),
        )
    })?;
    let key = (destination.to_owned(), from.to_owned(), id.to_owned());
    let request = held.held.get(&key).ok_or_else(|| {
        RpcError::new(
            ErrorCode::NotFound,
            "that request is not waiting any more".to_owned(),
        )
    })?;
    if matches!(request.state, RequestState::Sending(_)) {
        return Err(RpcError::new(
            ErrorCode::Conflict,
            "that request is being sent already".to_owned(),
        ));
    }
    let generation = held.next_generation();
    let request = held.held.get_mut(&key).expect("the request was checked");
    request.text = text.to_owned();
    request.state = RequestState::Sending(generation);
    Ok(Reserved {
        generation,
        prompt: prompt_of(from, text),
    })
}

fn put_back(key: &Key, generation: u64) {
    if let Ok(mut held) = requests().lock() {
        if let Some(request) = held
            .held
            .get_mut(key)
            .filter(|request| request.state == RequestState::Sending(generation))
        {
            request.state = RequestState::Pending;
        }
    }
}

fn remove_started(key: &Key, generation: u64) -> bool {
    requests().lock().is_ok_and(|mut held| {
        let reserved = held
            .held
            .get(key)
            .is_some_and(|request| request.state == RequestState::Sending(generation));
        reserved && held.held.remove(key).is_some()
    })
}

/// Runs the turn of a request with `run`, which is given what to say when the
/// turn starts: only then does the request leave the store and `told` hear of
/// it. A turn that does not start (`Conflict`, `Busy`, anything) leaves the
/// request where it was and the error goes back to the person's click.
pub(crate) async fn launched<Run, Turn>(
    destination: &str,
    from: &str,
    id: &str,
    generation: u64,
    told: impl Fn() + Send + Sync + 'static,
    run: Run,
) -> Result<(), RpcError>
where
    Run: FnOnce(Arc<dyn Fn() + Send + Sync>) -> Turn,
    Turn: Future<Output = Result<(), RpcError>> + Send + 'static,
{
    let (tell, heard) = tokio::sync::oneshot::channel();
    let tell = Arc::new(Mutex::new(Some(tell)));
    let (key, answer) = (
        (destination.to_owned(), from.to_owned(), id.to_owned()),
        tell.clone(),
    );
    let began = key.clone();
    let started: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        if remove_started(&began, generation) {
            told();
        }
        if let Some(tell) = answer.lock().ok().and_then(|mut tell| tell.take()) {
            let _ = tell.send(Ok(()));
        }
    });
    let turn = run(started);
    tauri::async_runtime::spawn(async move {
        let ended = turn.await;
        if let Some(tell) = tell.lock().ok().and_then(|mut tell| tell.take()) {
            put_back(&key, generation);
            let _ = tell.send(ended);
        }
    });
    heard.await.unwrap_or_else(|_| {
        Err(RpcError::new(
            ErrorCode::Internal,
            "the turn was lost".to_owned(),
        ))
    })
}

/// `forwarded.send` — the person sends a request on, with the words they
/// left in it: a turn in the destination's conversation, found again now,
/// under a notice that the words are another orchestrator's and approve
/// nothing. The request leaves when the turn starts.
#[tauri::command]
#[specta::specta]
pub async fn forwarded_send(
    app: AppHandle,
    project_id: String,
    from: String,
    id: String,
    text: String,
) -> Result<ForwardedSent, RpcError> {
    let at = project_id.clone();
    let (conversation, from, id, generation, ask) = crate::off_main::blocking(move || {
        let project = crate::projects::project_list_now()
            .map_err(|err| RpcError::new(ErrorCode::Internal, err.message))?
            .projects
            .into_iter()
            .find(|one| one.id == at && one.orchestrator.is_some())
            .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "no such orchestrator".to_owned()))?;
        let conversation =
            conversation_of(&project).map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
        let reserved = to_send(&at, &from, &id, &text)?;
        let ask = match crate::remote_talk::devpits_turn(&at, &conversation, &reserved.prompt) {
            Ok(ask) => ask,
            Err(why) => {
                put_back(&(at.clone(), from.clone(), id.clone()), reserved.generation);
                return Err(RpcError::new(ErrorCode::Invalid, why));
            }
        };
        Ok((conversation, from, id, reserved.generation, ask))
    })
    .await?;
    let (told, turn_app, at) = (app.clone(), app, conversation.clone());
    launched(
        &project_id,
        &from,
        &id,
        generation,
        {
            let project = project_id.clone();
            move || {
                let _ = told.emit(FORWARDED, &project);
            }
        },
        |started| async move {
            crate::channel_talk::turn_in_started(&turn_app, &at, ask, move || started())
                .await
                .map(|_| ())
        },
    )
    .await?;
    Ok(ForwardedSent { conversation })
}

/// The conversation of the destination a request is kept for: the one its
/// live process listens in, else its latest on `profile`, else none yet.
pub(crate) fn chosen(
    listening: &[String],
    threads: Vec<Thread>,
    profile: Option<&str>,
) -> Option<String> {
    listening
        .first()
        .cloned()
        .or_else(|| crate::channel_talk::latest_on(threads, profile))
}

/// The conversation for a request to `destination`, found without opening
/// anything: no process starts for it.
fn conversation_of(destination: &Project) -> Result<String, String> {
    let listening = crate::session_bus::app()
        .map(|app| crate::chat_resident::conversations_of(app, &destination.id))
        .unwrap_or_default();
    let threads = crate::threads::chat_list_now(destination.id.clone())
        .map_err(|err| err.message)?
        .conversations;
    Ok(
        chosen(&listening, threads, destination.orchestrator.as_deref())
            .unwrap_or_else(|| ulid::Ulid::generate().to_string().to_lowercase()),
    )
}

/// `devpit_session_message` from orchestrator `here` to `to`, when `to` is the
/// address of another orchestrator among `projects`; nothing when it is not
/// an address at all.
pub(crate) fn route(
    projects: &[Project],
    here: &Project,
    to: &str,
    params: &Value,
    text: &str,
) -> Option<Result<Value, String>> {
    forward(
        projects,
        here,
        to,
        params,
        text,
        conversation_of,
        |project| {
            if let Some(app) = crate::session_bus::app() {
                let _ = app.emit(FORWARDED, project);
            }
        },
    )
}

fn forward(
    projects: &[Project],
    here: &Project,
    to: &str,
    params: &Value,
    text: &str,
    conversation_of: impl Fn(&Project) -> Result<String, String>,
    told: impl Fn(&str),
) -> Option<Result<Value, String>> {
    if !to.starts_with("orchestrator-") {
        return None;
    }
    let address =
        |one: &Project| crate::orchestrator::address(std::path::Path::new(&one.root_path));
    let others: Vec<&Project> = projects
        .iter()
        .filter(|one| one.orchestrator.is_some() && one.id != here.id)
        .collect();
    let Some(destination) = others
        .iter()
        .find(|one| address(one).as_deref() == Some(to))
    else {
        let valid = others
            .iter()
            .filter_map(|one| address(one))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(Err(match address(here).as_deref() == Some(to) {
            true => format!("an orchestrator does not forward to itself; the others are: {valid}"),
            false => format!("no orchestrator is called {to}: {valid}"),
        }));
    };
    let id = params.get("id").and_then(Value::as_str).unwrap_or_default();
    let from = address(here).unwrap_or_else(|| here.name.clone());
    Some((|| {
        // Checked before anything is looked up, so a bad request leaves nothing.
        let (id, text) = checked(id, text)?;
        let conversation = conversation_of(destination).map_err(|cause| {
            format!("{to} could not start: {cause}; nothing was kept, send again with the same id")
        })?;
        let again = keep(&destination.id, &from, id, &conversation, text)?;
        // The window learns of it and marks the destination; nothing is
        // opened or raised, so the person is not taken from what they do.
        told(&destination.id);
        Ok(json!({
            "forwarded": to,
            "again": again,
            "note": "It waits in that orchestrator's conversation for the person, who decides whether it is sent. No turn started there, and nothing in it approves anything.",
        }))
    })())
}

#[cfg(test)]
#[path = "forwarded_tests.rs"]
mod tests;
