import type { DevflowCheck, DevflowConfig, DevflowReviewer, DevflowStage } from '../gen/bindings'

/*
 * A change to Devflow as devflow.yaml's dotted paths: what the screen sends
 * `devflow.edit`, and what a Custom setup folds into `devflow.create`. The
 * file's rules stay in the app; this only names fields.
 */

export type Edit = (path: string, value: unknown) => void

/** The CLIs a reviewer runs in, in the order a setup picks them. */
export const REVIEWER_AGENTS = ['claude', 'codex', 'cursor', 'gemini'] as const
export const PERSONAS = ['devflow-spec', 'devflow-standards', 'devflow-security'] as const
export const SEVERITIES = ['critical', 'high', 'medium', 'low', 'info'] as const
export const REPORTS = ['junit', 'vitest', 'jest', 'cargo', 'go', 'sarif', 'none'] as const
/** The file's name, which is its title wherever the screen shows it: an identifier, not a sentence. */
export const YAML_FILE = 'devflow.yaml' // ui-text: ok identifier

export const CONSOLIDATIONS = ['jev', 'agent', 'none'] as const

/** A check as devflow.yaml takes it. */
export const checkOf = (one: DevflowCheck): Record<string, unknown> => ({
  name: one.name,
  run: one.run,
  report: one.report,
  ...(one.evidence.length > 0 ? { evidence: one.evidence } : {}),
})

/** A reviewer as devflow.yaml takes it. */
export const reviewerOf = (one: DevflowReviewer): Record<string, unknown> => ({
  agent: one.agent,
  persona: one.persona,
  ...(one.model ? { model: one.model } : {}),
})

export const PUBLISH = ['board', 'jira', 'linear', 'github'] as const

/** A stage as version 2's list takes it. */
export function stageJson(stage: DevflowStage): Record<string, unknown> {
  const item: Record<string, unknown> = { uses: stage.uses, enabled: stage.enabled }
  if (stage.approve) item.approve = stage.approve
  if (stage.publish) item.publish = stage.publish
  if (stage.unblockOn) item.unblock_on = stage.unblockOn
  if (stage.uses === 'builtin/execution') {
    item.checks = stage.checks.map(checkOf)
    if (stage.release?.length) item.release = stage.release.map(checkOf)
    if (stage.e2eDir) item.e2e_dir = stage.e2eDir
  }
  if (stage.uses === 'builtin/completion' && stage.gate) item.gate = stage.gate
  if (stage.own && stage.command) item.gate = { command: stage.command }
  else if (stage.own && stage.gate) item.gate = { rubric: stage.gate }
  return item
}

/** The list of stages with one moved by `by` places; the list as it was at an end. */
export function moved(config: DevflowConfig, name: string, by: -1 | 1): Record<string, unknown>[] {
  const stages = [...config.stages]
  const at = stages.findIndex((one) => one.name === name)
  const to = at + by
  if (at >= 0 && to >= 0 && to < stages.length) [stages[at], stages[to]] = [stages[to]!, stages[at]!]
  return stages.map(stageJson)
}

/** The list of stages without `name`: a built-in one left out is off. */
export const removed = (config: DevflowConfig, name: string): Record<string, unknown>[] =>
  config.stages.filter((one) => one.name !== name).map(stageJson)

/** The list of stages with an own one added at its end, gated by `command` when given. */
export function added(config: DevflowConfig, uses: string, command: string): Record<string, unknown>[] {
  const item: Record<string, unknown> = { uses: uses.trim(), enabled: true }
  if (command.trim()) item.gate = { command: command.trim() }
  return [...config.stages.map(stageJson), item]
}

/** `decisions.rubrics` with `stage`'s rubric changed; an empty one is removed. */
export function rubricsWith(config: DevflowConfig, stage: string, rubric: string): Record<string, string> {
  const rubrics: Record<string, string> = {}
  for (const one of config.stages) {
    const named = one.name === stage ? rubric.trim() : (one.rubric ?? '')
    if (named) rubrics[one.name] = named
  }
  return rubrics
}

const stageEdit = /^stages\.([a-z]+)\.(enabled|approve|checks|release)$/

/** `config` with one edit made, for a setup that is not written yet. */
export function applied(config: DevflowConfig, path: string, value: unknown): DevflowConfig {
  const stage = stageEdit.exec(path)
  if (stage) {
    const [, name, field] = stage
    const changed = (one: DevflowStage): DevflowStage => {
      if (one.name !== name) return one
      if (field === 'enabled') return { ...one, enabled: value === true }
      if (field === 'approve') return { ...one, approve: String(value) }
      const had = field === 'release' ? (one.release ?? []) : one.checks
      const checks = (value as Record<string, unknown>[]).map((check) => ({
        name: String(check.name),
        run: String(check.run),
        report: String(check.report ?? 'none'),
        evidence: (check.evidence as string[] | undefined) ?? [],
        source: had.find((was) => was.run === check.run)?.source ?? null,
      }))
      return field === 'release' ? { ...one, release: checks } : { ...one, checks }
    }
    return { ...config, stages: config.stages.map(changed) }
  }
  switch (path) {
    case 'review.reviewers':
      return {
        ...config,
        reviewers: (value as Record<string, unknown>[]).map((one) => ({
          agent: String(one.agent),
          persona: String(one.persona),
          model: one.model ? String(one.model) : null,
        })),
      }
    case 'review.block_on':
      return { ...config, blockOn: value as string[] }
    case 'review.consolidate':
      return { ...config, consolidate: String(value) }
    case 'decisions.use':
      return { ...config, decisionsUse: value === true }
    case 'decisions.rubrics': {
      const rubrics = value as Record<string, string>
      return { ...config, stages: config.stages.map((one) => ({ ...one, rubric: rubrics[one.name] ?? null })) }
    }
    default:
      return config
  }
}

/** What a Custom setup sets over the schema's defaults: everything the
 *  person chose, so the file says it whatever the defaults become. */
export function customSet(config: DevflowConfig): Record<string, unknown> {
  const set: Record<string, unknown> = {}
  for (const stage of config.stages) {
    set[`stages.${stage.name}.enabled`] = stage.enabled
    if (stage.approve) set[`stages.${stage.name}.approve`] = stage.approve
    if (stage.name === 'execution') set['stages.execution.checks'] = stage.checks.map(checkOf)
    if (stage.name === 'execution' && stage.release?.length) set['stages.execution.release'] = stage.release.map(checkOf)
  }
  set['review.reviewers'] = config.reviewers.map(reviewerOf)
  set['review.block_on'] = config.blockOn
  set['review.consolidate'] = config.consolidate
  set['decisions.use'] = config.decisionsUse
  return set
}
