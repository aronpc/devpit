import { t } from '../i18n'
import type { IslandChange, IslandQuestion, IslandSession, IslandStep, Question, Touch } from '../gen/bindings'

/*
 * What the island draws, worked out from what it heard.
 *
 * Pure on purpose: the island is a glance, and a glance that is wrong about
 * who waits on you is worse than no island at all — so the rules that decide
 * it are functions the tests call.
 */

export type Sessions = Readonly<Record<string, IslandSession>>

/**
 * A question waiting on the person, wherever it came from: a chat that holds
 * its tools, or a terminal session about to ask.
 */
export interface Ask {
  readonly id: string
  readonly sessionId: string
  readonly tool: string
  readonly input: string
  readonly from: 'chat' | 'terminal'
  /** Where it comes from, by name, when the session itself is not on the island. */
  readonly project: string | null
  /** The terminal offered a rule to keep, so "always" can be answered. */
  readonly keepable: boolean
  /** What a chat's question touches, as the backend read it. */
  readonly touch?: Touch | null
}

export const fromChat = (question: Question): Ask => ({ ...question, from: 'chat', keepable: true, project: null })
export const fromTerminal = (question: IslandQuestion): Ask => ({ ...question, from: 'terminal' })

/** The sessions after one change. */
export function applied(sessions: Sessions, change: IslandChange): Sessions {
  if (change.was === 'gone') {
    if (!(change.sessionId in sessions)) return sessions
    const { [change.sessionId]: _gone, ...rest } = sessions
    return rest
  }
  return { ...sessions, [change.session.sessionId]: change.session }
}

/** When it last changed: a number the contract allows to be null. */
const atOf = (session: IslandSession): number => session.at ?? 0

/** Waiting first, then working, then the rest; the most recent first within each. */
export function ordered(sessions: Sessions): IslandSession[] {
  const rank = (one: IslandSession): number =>
    one.state === 'waiting' ? 0 : one.state === 'working' ? 1 : one.state === 'failed' ? 2 : 3
  return Object.values(sessions).sort((one, other) => rank(one) - rank(other) || atOf(other) - atOf(one))
}

/** What the mascot shows. Drawn from the face up, so every state has one. */
export type Mood = 'idle' | 'thinking' | 'working' | 'waiting' | 'asking' | 'done' | 'failed' | 'sleeping'

/** How long a finished turn is celebrated, in milliseconds. */
export const CHEERS_FOR = 6_000
/** How long the island sits with nothing alive before it dozes off. */
export const DOZES_AFTER = 10 * 60_000

/** One session's mood. */
export function moodOf(session: IslandSession, now: number, asking = false): Mood {
  if (asking) return 'asking'
  switch (session.state) {
    case 'waiting':
      return 'waiting'
    case 'working':
      return session.steps.length === 0 ? 'thinking' : 'working'
    case 'failed':
      return now - atOf(session) < CHEERS_FOR ? 'failed' : 'idle'
    case 'done':
      return now - atOf(session) < CHEERS_FOR ? 'done' : 'idle'
    default:
      return 'idle'
  }
}

/** The island's own mood: the most urgent of its sessions'. */
export function moodOfAll(sessions: readonly IslandSession[], questions: readonly Ask[], now: number): Mood {
  if (questions.length > 0) return 'asking'
  const moods = sessions.map((one) => moodOf(one, now))
  for (const mood of ['waiting', 'failed', 'working', 'thinking', 'done'] as const) {
    if (moods.includes(mood)) return mood
  }
  const latest = Math.max(0, ...sessions.map(atOf))
  return now - latest > DOZES_AFTER ? 'sleeping' : 'idle'
}

/** Whether anything waits on the person, which keeps the pill up. */
export const holds = (sessions: readonly IslandSession[], questions: readonly Ask[]): boolean =>
  questions.length > 0 || sessions.some((one) => one.state === 'waiting')

/** A tool's verb (its catalog key under `island.verb.`), as a step reads: `Edit invoice.ts`, `Run npm test`. */
export function verbOf(tool: string): string {
  const mcp = /^mcp__[^_]+(?:_[^_]+)*__(.+)$/.exec(tool)
  if (mcp) return mcp[1].replace(/[_-]+/g, ' ')
  const verb = VERBS[tool]
  return verb ? t(`island.verb.${verb}`) : tool
}

const VERBS: Readonly<Record<string, string>> = {
  Read: 'Read',
  Edit: 'Edit',
  MultiEdit: 'Edit',
  Write: 'Write',
  NotebookEdit: 'Edit',
  Bash: 'Run',
  Grep: 'Search',
  Glob: 'Find',
  WebFetch: 'Fetch',
  WebSearch: 'SearchWeb',
  Agent: 'Delegate',
  Task: 'Delegate',
  TodoWrite: 'Plan',
  Skill: 'UseSkill',
  AskUserQuestion: 'Ask',
  ExitPlanMode: 'ProposePlan',
}

/** One step, in words. */
export const stepWords = (step: IslandStep): string => (step.target ? `${verbOf(step.tool)} ${step.target}` : verbOf(step.tool))

/** What a session is called on the island: its card, else its project, else its folder. */
export const nameOf = (session: IslandSession): string => session.card ?? session.project ?? t('island.agent')

/**
 * Every session's name, told apart: two sessions in one project with no card
 * would otherwise be two chips that say the same thing. The older keeps the
 * plain name; the others are numbered in the order they came.
 */
export function labelled(sessions: readonly IslandSession[]): ReadonlyMap<string, string> {
  const byAge = [...sessions].sort((one, other) => atOf(one) - atOf(other))
  const seen = new Map<string, number>()
  const labels = new Map<string, string>()
  for (const session of byAge) {
    const name = nameOf(session)
    const count = (seen.get(name) ?? 0) + 1
    seen.set(name, count)
    labels.set(session.sessionId, count === 1 ? name : `${name} ${count}`)
  }
  return labels
}

/** The line under a session's name: what it is doing now. */
export function nowWords(session: IslandSession, now: number): string {
  const mood = moodOf(session, now)
  const last = session.steps.at(-1)
  if (mood === 'waiting') return t('island.now.waiting')
  if (mood === 'thinking') return t('island.now.thinking')
  if (mood === 'working' && last) return stepWords(last)
  if (session.state === 'failed') return session.said ?? t('island.now.failed')
  if (session.state === 'done') return session.said ?? t('island.now.done')
  return t('island.now.idle')
}

/** The pill's count: who waits, else who works. */
export function headline(sessions: readonly IslandSession[], questions: readonly Ask[]): string {
  const waiting = sessions.filter((one) => one.state === 'waiting').length + questions.length
  if (waiting > 0) return t('island.headline.waiting', { count: waiting })
  const working = sessions.filter((one) => one.state === 'working').length
  if (working > 0) return t('island.headline.working', { count: working })
  return sessions.length === 0 ? t('island.headline.quiet') : t('island.headline.allDone')
}

/** What a question asks to do, in words: the tool and what it would run on. */
export function askedWords(question: Pick<Ask, 'tool' | 'input'>): string {
  let input: Record<string, unknown> = {}
  try {
    const parsed: unknown = JSON.parse(question.input)
    if (parsed && typeof parsed === 'object') input = parsed as Record<string, unknown>
  } catch {
    /* Not JSON: the tool's name says enough. */
  }
  const said = ['command', 'file_path', 'path', 'url', 'pattern']
    .map((field) => input[field])
    .find((value): value is string => typeof value === 'string' && value.trim() !== '')
  return said ? `${verbOf(question.tool)} ${said.split('\n')[0]}` : verbOf(question.tool)
}
