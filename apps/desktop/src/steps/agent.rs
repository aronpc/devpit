//! The agent step: one headless turn, with a schema and a ceiling.

use devpit_agentcli as agent;
use devpit_core::Store;
use devpit_rpc::Step;

use super::context::{context_of, injected, Checkout};
use super::Finished;

pub struct Starting<'a> {
    pub run_id: &'a str,
    pub session_id: &'a str,
    pub fixed: Option<&'a crate::run_start::FixedAgent>,
}

pub fn run(
    store: &Store,
    card_id: &str,
    step: &Step,
    starting: Starting<'_>,
    mut on_progress: impl FnMut(&str),
    on_start: impl FnMut(u32),
) -> Result<Finished, String> {
    let Starting {
        run_id,
        session_id,
        fixed,
    } = starting;
    let config = super::agent_config::readable(&step.config)?;

    // No ceiling, no run. Said here rather than after the money is spent.
    let Some(cap) = config.budget_usd else {
        return Err("this step declares no spending cap, so it does not run".to_owned());
    };

    let card = store
        .card(card_id)
        .map_err(|err| err.to_string())?
        .ok_or("no such card")?;

    let prompt = format!(
        "{}\n\n---\n\n# {}\n\n{}",
        config.prompt, card.title, card.body
    );

    // Only the agent this step names is passed. Handing the CLI the whole
    // catalogue would make every step's behaviour depend on files it never
    // mentions.
    let catalogue = agent::read_every_agent(&agent::seed_sources());
    let named = config
        .agent
        .as_ref()
        .and_then(|name| catalogue.agents.iter().find(|a| &a.name == name).cloned());
    if let (Some(name), None) = (&config.agent, &named) {
        return Err(format!("no agent named `{name}` on this machine"));
    }
    let argument = named
        .as_ref()
        .map(|a| agent::as_argument(std::slice::from_ref(a)));

    // The card's checkout when this step wants one, the project otherwise.
    let cwd = crate::checkout::cwd_for(store, card_id, step, &mut on_progress)?;
    // On the run's row, so continuing this session later finds its folder.
    store
        .set_run_cwd(run_id, &cwd.display().to_string())
        .map_err(|err| err.to_string())?;
    // Only a checkout of the card's own carries the card's branch. A step that
    // asked for none runs in the project, where the branch is the person's.
    let branch = crate::checkout::branch_of(step, &cwd);
    let project = store.project_of_card(card_id).ok().flatten();
    let context = context_of(
        &card,
        branch
            .as_deref()
            .map(|branch| Checkout { path: &cwd, branch }),
        project.as_deref(),
    );
    let context = injected(&context, &config.inject);

    // The hooks reach us through a settings file written next to the state,
    // so a turn tells the board what it is doing while it does it.
    // The profile's advisor model too; no profile is the CLI as installed.
    let settings = super::hook_settings_for(Some(
        config
            .profile
            .as_deref()
            .unwrap_or(devpit_agentcli::PROGRAM),
    ));

    // Which account this spends through. A named profile that has gone is an
    // error rather than a silent fall back to the default: the difference
    // between them is somebody's bill.
    let current_runner = match (&fixed, &config.profile) {
        (None, Some(id)) => Some(crate::agent_profiles::runner_for(store, id)?),
        _ => None,
    };
    let runner = fixed.map(|fixed| &fixed.runner).or(current_runner.as_ref());
    let model = fixed
        .and_then(|fixed| fixed.model.as_deref())
        .or(config.model.as_deref());

    let outcome = agent::run_turn_cancellable(
        &agent::Turn {
            prompt: &prompt,
            cwd: &cwd,
            agents: argument.as_deref(),
            schema: config.schema.as_deref(),
            budget_usd: Some(cap),
            model,
            settings: settings.as_deref(),
            session_id: Some(session_id),
            env: &context,
            runner,
        },
        |line| {
            // Only the assistant's own words. The stream also carries hook
            // chatter and rate-limit notices, and a card relaying those reads
            // as the agent talking about the machinery rather than the work.
            if let Some(text) = assistant_text(line) {
                on_progress(&text);
            }
        },
        on_start,
    )
    .map_err(|err| err.to_string())?;

    crate::run_start::record_effective_model(store, run_id, outcome.model.as_deref())
        .map_err(|err| err.to_string())?;

    // The CLI's own word for the session wins: its transcript is named by it.
    let said = outcome.session_id.as_deref().unwrap_or(session_id);
    if said != session_id {
        eprintln!("run {run_id} asked for session {session_id} and ran in {said}");
        store
            .set_run_session(run_id, said)
            .map_err(|err| err.to_string())?;
    }

    // A schema the answer does not satisfy is a failure, and the card does not
    // advance. Prose where fields were asked for is the case this catches.
    if let Some(schema) = &config.schema {
        if let Err(reason) = agent::validates(&outcome.result, schema) {
            return Ok(Finished {
                ok: false,
                output: format!(
                    "the answer did not match the schema: {reason}\n\n{}",
                    outcome.result
                ),
                cost_usd: outcome.cost_usd,
                duration_ms: outcome.duration_ms,
                exit_code: None,
            });
        }
    }

    // A review, if the answer is one. Nothing is inferred: an answer that does
    // not read as findings leaves no evidence, and the run is what it always
    // was. `reviewed` refuses a half-read review for the same reason the
    // schema check above refuses prose — a partial answer acted on as a whole
    // one is worse than no answer.
    super::review::kept_as_a_review(store, run_id, &outcome.result, &config.prompt, &cwd);

    Ok(Finished {
        ok: !outcome.is_error,
        output: outcome.result,
        cost_usd: outcome.cost_usd,
        duration_ms: outcome.duration_ms,
        exit_code: None,
    })
}

/// The words an assistant fragment carries, if it carries any.
///
/// The stream interleaves hook events, init and rate-limit notices with the
/// answer. Relaying all of it would make a card show the machinery instead of
/// the work.
fn assistant_text(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("type")?.as_str()? != "assistant" {
        return None;
    }
    let blocks = value.get("message")?.get("content")?.as_array()?;
    let text: String = blocks
        .iter()
        .filter(|block| block.get("type").and_then(|t| t.as_str()) == Some("text"))
        .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
        .collect::<Vec<_>>()
        .join("");
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
