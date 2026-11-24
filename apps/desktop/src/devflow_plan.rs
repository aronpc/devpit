//! Devflow's briefing and plan, on the board: the brief as an epic card, and
//! the plan as the epic's cards, each with its criteria and the cards it
//! waits on.
//!
//! A plan is a preview before any card exists. It waits on the epic, where
//! the person can move, cut or edit it and create the cards with a click; an
//! agent creates them with `apply` only after the person's yes, and then
//! only the plan the person last saw.

use std::path::Path;
use std::time::SystemTime;

use devflow_engine::work::WorkItems;

use devflow_engine::brief::{self, Brief};
use devflow_engine::jev::{self, Said};
use devflow_engine::plan::{self, Slice, PREFIX};
use devflow_engine::Config;
use devpit_core::locale::Locale;
use devpit_core::{NewCard, Store};
use devpit_rpc::{DevflowPlan, DevflowPlanCreated, ErrorCode, PlannedCard, Project, RpcError};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::cards::YOU;
use crate::devflow_dialect::dialect_in;
use crate::devflow_jev::Jev;

/// The window reads the board again.
const BOARD_CHANGED: &str = "board:changed";

/// Whose yes made a plan's cards.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Yes {
    /// The person's click on the board.
    Board,
    /// An agent's `apply`, after the person's yes in its conversation.
    Agent,
    /// The orchestrator's `apply`, passing on the yes the person gave it.
    Relayed,
}

impl Yes {
    fn word(self) -> &'static str {
        match self {
            Yes::Board => "board",
            Yes::Agent => "agent",
            Yes::Relayed => "orchestrator",
        }
    }
}

/// `devflow_brief` and `devflow_plan`, asked by the agent standing in `project`.
pub(crate) fn respond(
    locale: Locale,
    app: Option<&AppHandle>,
    method: &str,
    project: &Project,
    params: &Value,
) -> Result<Value, String> {
    let (config, jev) = judging(project, text(params, "epic").as_deref());
    // A plan for a tracker devpit cannot write yet is refused before anything is made.
    if let (Some(config), "devflow_plan") = (&config, method) {
        plan::published_in(dialect_in(locale), config)?;
    }
    let mut answer = respond_judged(
        locale,
        app,
        method,
        project,
        params,
        config.as_ref(),
        jev.as_ref(),
    )?;
    if let (Some(config), "devflow_plan") = (&config, method) {
        answer["decisions"] = crate::devflow_jev::decisions_of(config, jev.as_ref());
    }
    Ok(answer)
}

/// The project's devflow.yaml, and Jev where it uses Decisions.
fn judging(project: &Project, epic: Option<&str>) -> (Option<Config>, Option<Jev>) {
    let root = Path::new(&project.root_path);
    let config = match crate::devflow::read(root) {
        devflow_engine::State::Valid(_, config) => Some(*config),
        _ => None,
    };
    let jev = config
        .as_ref()
        .and_then(|config| Jev::of(project, config, root, epic, &[]));
    (config, jev)
}

/// [`respond`], with Jev where the project uses it: the brief judged before
/// planning, the plan in its preview and before its cards are created.
pub(crate) fn respond_judged(
    locale: Locale,
    app: Option<&AppHandle>,
    method: &str,
    project: &Project,
    params: &Value,
    config: Option<&Config>,
    jev: Option<&Jev>,
) -> Result<Value, String> {
    let store = crate::projects::store().map_err(|err| err.message)?;
    let answer = match method {
        "devflow_brief" => {
            let context_on = config.is_none_or(|config| config.stages.on("context"));
            briefed(&store, &project.id, params, context_on)?
        }
        _ if crate::devflow_plan_edit::asks(params) => crate::devflow_plan_edit::respond(
            &store,
            &project.id,
            params,
            approval(params)?.as_deref(),
        )?,
        _ => planned(locale, &store, &project.id, params, config.zip(jev))?,
    };
    if let Some(app) = app {
        let _ = app.emit(BOARD_CHANGED, &project.id);
    }
    Ok(answer)
}

fn text(params: &Value, name: &str) -> Option<String> {
    params
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// The card, when it is on this project's board.
fn own_card(store: &Store, project_id: &str, id: &str) -> Result<devpit_core::CardRow, String> {
    let theirs = store
        .project_id_of_card(id)
        .map_err(|err| err.to_string())?;
    store
        .card(id)
        .map_err(|err| err.to_string())?
        .filter(|_| theirs.as_deref() == Some(project_id))
        .ok_or_else(|| format!("no card `{id}` on this project's board"))
}

/// The cards on the board each slice waits on outside its plan, by id:
/// refused when one names no card of this project.
fn outside_cards(
    store: &Store,
    project_id: &str,
    slices: &[Slice],
) -> Result<Vec<Vec<String>>, String> {
    let card = |on: &str| -> Option<String> {
        let id = match on.strip_prefix('#') {
            Some(number) => store
                .card_numbered(project_id, number.parse().ok()?)
                .ok()??,
            None => on.to_owned(),
        };
        own_card(store, project_id, &id).ok().map(|card| card.id)
    };
    slices
        .iter()
        .enumerate()
        .map(|(at, slice)| {
            plan::outside(slices, slice)
                .into_iter()
                .map(|on| {
                    card(on).ok_or_else(|| {
                        format!(
                            "card {} ({}): depends_on `{on}` is no card on this project's board",
                            at + 1,
                            slice.key
                        )
                    })
                })
                .collect()
        })
        .collect()
}

/// Where new work waits: the lane whose role is backlog, else the first.
fn waiting_lane(store: &Store, project_id: &str) -> Result<String, String> {
    store
        .ensure_board(project_id)
        .map_err(|err| err.to_string())?;
    let columns = store.columns(project_id).map_err(|err| err.to_string())?;
    let backlog = columns.iter().find(|column| {
        devpit_core::store::column_roles::role_of(column.role.as_deref(), &column.name)
            == Some("backlog")
    });
    backlog
        .or(columns.first())
        .map(|column| column.id.clone())
        .ok_or_else(|| "this board has no columns".to_owned())
}

/// The stage a brief's note sends the agent to next.
pub(crate) fn after_brief(context_written: bool, context_on: bool) -> &'static str {
    match (context_written, context_on) {
        (true, _) => {
            "Written on the board. The next stage is the plan (devflow-plan), when the person \
             asks for it."
        }
        (false, true) => {
            "Written on the board. The next stage is the context (devflow-context), when the \
             person asks for it."
        }
        (false, false) => {
            "Written on the board. The context stage is off in devflow.yaml, so the next stage \
             is the plan (devflow-plan), when the person asks for it."
        }
    }
}

fn briefed(
    store: &Store,
    project_id: &str,
    params: &Value,
    context_on: bool,
) -> Result<Value, String> {
    let given: Brief = serde_json::from_value(params.clone()).map_err(|why| {
        format!("a brief is {{ title, goal, for_whom, success, out_of_scope, constraints, decisions, context, card }}: {why}")
    })?;
    if !given.has_fields() && given.context.is_none() && given.title.is_none() {
        return Err("nothing to write: pass the brief's fields, or `context`".to_owned());
    }
    let card = text(params, "card")
        .map(|id| own_card(store, project_id, &id))
        .transpose()?;
    let title = match (&given.title, &card) {
        (Some(title), _) if !title.trim().is_empty() => brief::epic_title(title),
        (_, Some(card)) => card.title.clone(),
        _ => {
            return Err(
                "a new epic needs a `title`; pass `card` to write onto one that exists".to_owned(),
            )
        }
    };
    let body = brief::written(card.as_ref().map_or("", |card| &card.body), &given);
    if body.len() > brief::LONGEST {
        return Err(format!(
            "the brief is past {} bytes; say it shorter",
            brief::LONGEST
        ));
    }
    let (id, created) = match card {
        Some(card) => {
            store
                .update_card(&card.id, &title, &body)
                .map_err(|err| err.to_string())?;
            (card.id, false)
        }
        None => {
            let board = crate::devflow_work::Board { store, project_id };
            let epic = devflow_engine::work::New {
                title: title.clone(),
                kind: devflow_engine::work::Kind::Epic,
                parent: None,
                depends_on: Vec::new(),
                body,
            };
            (board.create(&epic, SystemTime::now())?.id, true)
        }
    };
    Ok(json!({
        "cardId": id,
        "title": title,
        "created": created,
        "note": after_brief(given.context.is_some(), context_on),
    }))
}

/// A plan asked with no epic, merge or cut: run on demand, a preview only.
pub(crate) fn on_demand(params: &Value) -> bool {
    text(params, "epic").is_none() && !crate::devflow_plan_edit::asks(params)
}

/// The plan's preview with no epic to wait on: its cards read and checked
/// as an epic's are, then created nowhere and kept nowhere.
pub(crate) fn previewed(locale: Locale, params: &Value) -> Result<Value, String> {
    if params.get("apply").and_then(Value::as_bool) == Some(true) {
        return Err("cards are created under their epic: pass `epic`".to_owned());
    }
    let slices = plan::read_in(
        dialect_in(locale),
        params.get("cards").unwrap_or(&Value::Null),
    )?;
    Ok(json!({
        "applied": false,
        "epic": Value::Null,
        "preview": preview_of(&slices),
        "note": "Nothing was created, and nothing is kept: with no epic, this plan is the preview \
                 alone. To make its cards, brief the work as an epic (devflow-briefing) and plan \
                 it with `epic`.",
    }))
}

fn preview_of(slices: &[Slice]) -> Vec<Value> {
    slices
        .iter()
        .map(|slice| {
            json!({
                "key": slice.key,
                "title": plan::title(slice),
                "blockedBy": slice.depends_on,
            })
        })
        .collect()
}

/// Jev's word on a stage's work, by the rubric devflow.yaml gives it.
fn judge(
    judged: Option<(&Config, &Jev)>,
    stage: &str,
    state: impl FnOnce() -> Value,
) -> Option<Said> {
    let (config, jev) = judged?;
    let rubric = jev::rubric_for(config, stage)?;
    Some(jev.judge(&rubric, &state()))
}

/// Why Jev holds `stage`'s work: a plan only when devflow.yaml opted its
/// rubric in, else its answer is advice.
fn held(judged: Option<(&Config, &Jev)>, stage: &str, said: Option<&Said>) -> Option<String> {
    let (config, _) = judged?;
    jev::holds_in(config, stage, said?)
}

fn jev_said(brief: Option<&Said>, plan: Option<&Said>) -> Value {
    json!({
        "brief": brief.map(Said::to_json),
        "plan": plan.map(Said::to_json),
    })
}

fn planned(
    locale: Locale,
    store: &Store,
    project_id: &str,
    params: &Value,
    judged: Option<(&Config, &Jev)>,
) -> Result<Value, String> {
    let epic = text(params, "epic").ok_or("which epic? pass its card id as `epic`")?;
    let epic_card = own_card(store, project_id, &epic)?;
    let apply = params.get("apply").and_then(Value::as_bool) == Some(true);
    let kept = store.plan(&epic).map_err(|err| err.to_string())?;
    let relayed = approval(params)?;
    let slices = match params.get("cards") {
        // An apply that names no cards is the yes to the preview the person saw.
        None | Some(Value::Null) if apply => kept
            .as_ref()
            .map(|row| slices_of(&row.cards))
            .ok_or("there is no preview on this epic to apply: send the cards first")?,
        cards => plan::read_in(dialect_in(locale), cards.unwrap_or(&Value::Null))?,
    };
    outside_cards(store, project_id, &slices)?;
    // Before planning: whether the brief can be planned from.
    let brief = judge(judged, "briefing", || {
        jev::brief_state(
            &epic_card.title,
            &brief::read(&epic_card.body).unwrap_or_default(),
        )
    });
    if let Some(held) = held(judged, "briefing", brief.as_ref()) {
        return Err(format!(
            "{held}: write the brief's goal, out of scope and success with devpit_devflow_brief \
             (devflow-briefing), then plan again"
        ));
    }
    if let (true, Some(row)) = (apply, &kept) {
        if let Some(created) = &row.created {
            return Ok(said_created(&epic, created, false));
        }
        let shown = slices_of(&row.cards);
        // What the person saw last is what their yes was to: their own edits
        // on the board, or this very preview.
        if row.edited || shown == slices {
            // Before the cards are made: whether they are ready to be.
            let plan = judge(judged, "plan", || jev::plan_state(&shown));
            if let Some(held) = held(judged, "plan", plan.as_ref()) {
                return Err(format!(
                    "{held}: nothing was created. Make each card a vertical slice with testable \
                     criteria and the cards it waits on, then show the plan again; the person \
                     can still create it on the board"
                ));
            }
            let yes = if relayed.is_some() {
                Yes::Relayed
            } else {
                Yes::Agent
            };
            let made = create(store, project_id, &epic, &shown, yes, relayed.as_deref())
                .map_err(|err| err.message)?;
            let mut said = said_created(&epic, &made, true);
            if judged.is_some() {
                said["jev"] = jev_said(brief.as_ref(), plan.as_ref());
            }
            return Ok(said);
        }
    }
    keep(store, &epic, &slices, false)?;
    // Asked of the preview too, so a plan enforce would hold is fixed before the yes.
    let plan = judge(judged, "plan", || jev::plan_state(&slices));
    let warning = held(judged, "plan", plan.as_ref()).map(|held| {
        format!("{held}: applied as it is, nothing would be created; fix the cards first")
    });
    let advice = warning
        .is_none()
        .then(|| plan.as_ref().and_then(jev::failing))
        .flatten()
        .map(|failing| format!("{failing}: advice, the plan is not held by it"));
    let note = if apply {
        "Nothing was created: the person has not seen these cards, so they are now the preview. \
         Show them, and apply once the person says yes to them."
    } else {
        "Nothing was created. The plan waits on the epic, on the board, where the person can \
         move, cut or edit it and create the cards with a click. Or, after their explicit yes \
         here, call again with the same cards and apply: true."
    };
    let mut said = json!({
        "applied": false,
        "epic": epic,
        "preview": preview_of(&slices),
        "note": note,
    });
    if judged.is_some() {
        said["jev"] = jev_said(brief.as_ref(), plan.as_ref());
    }
    if let Some(warning) = warning {
        said["warning"] = json!(warning);
    }
    if let Some(advice) = advice {
        said["advice"] = json!(advice);
    }
    Ok(said)
}

/// The person's words an orchestrator passes on with its `apply`.
fn approval(params: &Value) -> Result<Option<String>, String> {
    let Some(given) = params.get("approval").filter(|given| !given.is_null()) else {
        return Ok(None);
    };
    let quote = text(given, "quote").ok_or(
        "approval needs the person's own words as `quote`, the message where they said yes",
    )?;
    match text(given, "by").as_deref() {
        None | Some("orchestrator") => Ok(Some(quote)),
        Some(other) => Err(format!(
            "approval is passed on by the orchestrator, not `{other}`: in your own \
             conversation, apply after the person's yes"
        )),
    }
}

/// Why an orchestrator's `apply` for another project cannot go on: it
/// stands in no conversation with the person about that plan, so the yes it
/// passes on is quoted.
pub(crate) fn relayed_refusal(method: &str, params: &Value) -> Option<String> {
    let apply = params.get("apply").and_then(Value::as_bool) == Some(true);
    (method == "devflow_plan" && apply && !matches!(approval(params), Ok(Some(_)))).then(|| {
        "nothing was created: an orchestrator applies another project's plan with \
         approval: { by: \"orchestrator\", quote: \"<the person's yes, in their words>\" }"
            .to_owned()
    })
}

fn slices_of(cards: &str) -> Vec<Slice> {
    serde_json::from_str(cards).unwrap_or_default()
}

fn keep(store: &Store, epic: &str, slices: &[Slice], edited: bool) -> Result<(), String> {
    let cards = serde_json::to_string(slices).map_err(|err| err.to_string())?;
    match store
        .keep_plan(epic, &cards, edited)
        .map_err(|err| err.to_string())?
    {
        true => Ok(()),
        false => Err("the plan is too large to keep; split the epic".to_owned()),
    }
}

/// Creates the epic's waiting plan, and says on the epic whose yes made it.
/// The plan as it was created, as `created` keeps it.
fn create(
    store: &Store,
    project_id: &str,
    epic: &str,
    slices: &[Slice],
    yes: Yes,
    quote: Option<&str>,
) -> Result<String, RpcError> {
    let lane =
        waiting_lane(store, project_id).map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
    let titles: Vec<String> = slices.iter().map(plan::title).collect();
    let bodies: Vec<String> = slices.iter().map(plan::body).collect();
    let waits = plan::waits_on(slices);
    let outside = outside_cards(store, project_id, slices)
        .map_err(|why| RpcError::new(ErrorCode::Invalid, why))?;
    let cards: Vec<NewCard<'_>> = (0..slices.len())
        .map(|at| NewCard {
            title: &titles[at],
            body: &bodies[at],
            waits_on: &waits[at],
            waits_on_cards: &outside[at],
        })
        .collect();
    let record = |ids: &[String]| created_record(yes, slices, ids);
    // None when another yes got there first: its cards are the answer.
    if store.create_plan(epic, &lane, &cards, record)?.is_some() {
        let whose = match yes {
            Yes::Board => "on the person's click on the board".to_owned(),
            Yes::Agent => "after the person's yes in the conversation".to_owned(),
            Yes::Relayed => format!(
                "after the person's yes, passed on by the orchestrator: «{}»",
                quote.unwrap_or_default()
            ),
        };
        let note = format!("{PREFIX}Plan created {whose}: {} cards.", slices.len());
        let _ = store.add_comment(epic, if yes == Yes::Board { YOU } else { "agent" }, &note);
    }
    store
        .plan(epic)?
        .and_then(|row| row.created)
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "the plan is gone"))
}

/// What a created plan keeps: whose yes, and each card with its key.
fn created_record(yes: Yes, slices: &[Slice], ids: &[String]) -> String {
    json!({
        "by": yes.word(),
        "cards": slices.iter().zip(ids).map(|(slice, id)| json!({
            "key": slice.key,
            "cardId": id,
            "title": plan::title(slice),
            "blockedBy": slice.depends_on,
        })).collect::<Vec<_>>(),
    })
    .to_string()
}

/// The answer once the cards exist: made by this call, or earlier.
fn said_created(epic: &str, created: &str, now: bool) -> Value {
    let record: Value = serde_json::from_str(created).unwrap_or_default();
    let cards = record.get("cards").cloned().unwrap_or_else(|| json!([]));
    let on_board = record.get("by").and_then(Value::as_str) == Some(Yes::Board.word());
    let starts: Vec<Value> = cards
        .as_array()
        .into_iter()
        .flatten()
        .filter(|card| card["blockedBy"].as_array().is_none_or(Vec::is_empty))
        .map(|card| card["cardId"].clone())
        .collect();
    let note = match (now, on_board) {
        (_, true) => "The person already approved this plan on the board: these are its cards.",
        (true, false) => {
            "Created under the epic. The cards in startsNow wait on nothing and can start now."
        }
        (false, false) => "This plan was already applied: these are its cards.",
    };
    json!({
        "applied": true,
        "approvedOnBoard": on_board,
        "epic": epic,
        "cards": cards,
        "startsNow": starts,
        "note": note,
    })
}

/// The plan waiting on an epic, as its card shows it.
pub(crate) fn waiting(store: &Store, epic: &str) -> Option<DevflowPlan> {
    let row = store.plan(epic).ok().flatten()?;
    if row.created.is_some() {
        return None;
    }
    Some(DevflowPlan {
        epic_id: row.epic_id,
        cards: slices_of(&row.cards)
            .into_iter()
            .map(planned_card)
            .collect(),
        edited: row.edited,
        planned_at: row.planned_at as f64,
    })
}

fn planned_card(slice: Slice) -> PlannedCard {
    PlannedCard {
        key: slice.key,
        title: slice.title,
        build: slice.build,
        criteria: slice.criteria,
        files: slice.files,
        verify: slice.verify,
        depends_on: slice.depends_on,
        abuse: slice.abuse,
        boundary: slice.boundary,
    }
}

fn slice_of(card: PlannedCard) -> Slice {
    Slice {
        key: card.key,
        title: card.title,
        build: card.build,
        criteria: card.criteria,
        files: card.files,
        verify: card.verify,
        depends_on: card.depends_on,
        abuse: card.abuse,
        boundary: card.boundary,
    }
}

/// The epic, when the window names one of this project's with a plan waiting.
fn epic_in(store: &Store, project_id: &str, epic_id: &str) -> Result<(), RpcError> {
    own_card(store, project_id, epic_id).map_err(|why| RpcError::new(ErrorCode::NotFound, why))?;
    waiting(store, epic_id)
        .map(|_| ())
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "no plan waits on that card"))
}

/// The person's edit of a waiting plan: what a yes then creates.
pub(crate) fn kept_on_board(
    store: &Store,
    project_id: &str,
    epic_id: &str,
    cards: Vec<PlannedCard>,
) -> Result<DevflowPlan, RpcError> {
    epic_in(store, project_id, epic_id)?;
    let invalid = |why: String| RpcError::new(ErrorCode::Invalid, why);
    let slices: Vec<Slice> = cards.into_iter().map(slice_of).collect();
    let cards = serde_json::to_value(slices).map_err(|err| invalid(err.to_string()))?;
    let slices = plan::read_in(dialect_in(Locale::current()), &cards).map_err(invalid)?;
    outside_cards(store, project_id, &slices).map_err(invalid)?;
    keep(store, epic_id, &slices, true).map_err(invalid)?;
    waiting(store, epic_id).ok_or_else(|| RpcError::new(ErrorCode::NotFound, "the plan is gone"))
}

/// The person's click: the waiting plan becomes the epic's cards, once Jev
/// judged it as it would an agent's apply.
pub(crate) fn created_on_board(
    store: &Store,
    project_id: &str,
    epic_id: &str,
    judged: Option<(&Config, &Jev)>,
) -> Result<DevflowPlanCreated, RpcError> {
    epic_in(store, project_id, epic_id)?;
    let row = store
        .plan(epic_id)?
        .ok_or_else(|| RpcError::new(ErrorCode::NotFound, "no plan waits on that card"))?;
    let slices = slices_of(&row.cards);
    let plan = judge(judged, "plan", || jev::plan_state(&slices));
    if let Some(held) = held(judged, "plan", plan.as_ref()) {
        return Err(RpcError::new(
            ErrorCode::Invalid,
            format!(
                "{held}: nothing was created. Make each card a vertical slice with testable \
                 criteria and the cards it waits on, then create it again"
            ),
        ));
    }
    let created = create(store, project_id, epic_id, &slices, Yes::Board, None)?;
    let record: Value = serde_json::from_str(&created).unwrap_or_default();
    Ok(DevflowPlanCreated {
        card_ids: record["cards"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|card| card["cardId"].as_str().map(str::to_owned))
            .collect(),
    })
}

/// `devflow.plan_keep` — the person moved, cut or edited a waiting plan on
/// the board.
#[tauri::command]
#[specta::specta]
pub async fn devflow_plan_keep(
    project_id: String,
    epic_id: String,
    cards: Vec<PlannedCard>,
) -> Result<DevflowPlan, RpcError> {
    crate::off_main::blocking(move || {
        kept_on_board(&crate::board::store()?, &project_id, &epic_id, cards)
    })
    .await
}

/// `devflow.plan_create` — the person's click: the epic's waiting plan
/// becomes its cards.
#[tauri::command]
#[specta::specta]
pub async fn devflow_plan_create(
    app: AppHandle,
    project_id: String,
    epic_id: String,
) -> Result<DevflowPlanCreated, RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::board::store()?;
        let project = crate::devflow_screen::project_here(&store, &project_id)?;
        let (config, jev) = judging(&project, Some(&epic_id));
        let created = created_on_board(
            &store,
            &project_id,
            &epic_id,
            config.as_ref().zip(jev.as_ref()),
        )?;
        let _ = app.emit(BOARD_CHANGED, &project_id);
        Ok(created)
    })
    .await
}

/// `devflow.plan_drop` — the person dropped a waiting plan: no card is made.
#[tauri::command]
#[specta::specta]
pub async fn devflow_plan_drop(project_id: String, epic_id: String) -> Result<(), RpcError> {
    crate::off_main::blocking(move || {
        let store = crate::board::store()?;
        epic_in(&store, &project_id, &epic_id)?;
        store.drop_plan(&epic_id)?;
        Ok(())
    })
    .await
}

#[cfg(test)]
#[path = "devflow_plan_tests.rs"]
mod tests;
