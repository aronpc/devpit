import type { Part } from '../gen/bindings'
import { t } from '../i18n'

/*
 * The work a conversation left running beside it.
 *
 * Read from the `task` parts the driver emits — one per change, measured on
 * Claude Code 2.1.270 as `task_started`, `task_progress`, `task_updated` and
 * `task_notification` — so the latest part for a task id is its state.
 */

export interface BackgroundTask {
  readonly id: string
  /** `local_bash` for a command, `local_agent` for a subagent. */
  readonly kind: string | null
  readonly description: string
  readonly status: string
  readonly summary: string | null
}

/* Still going: started, or reporting progress. Anything else is an ending. */
export const running = (task: BackgroundTask): boolean =>
  task.status === 'started' || task.status === 'running'

/* Every task the parts mention, in the order they first appeared, each at its
   latest state. A later part without a description keeps the earlier one: the
   progress and ending lines do not always repeat it. */
export function tasksOf(parts: readonly Part[]): readonly BackgroundTask[] {
  const byId = new Map<string, BackgroundTask>()
  for (const part of parts) {
    if (part.kind !== 'task') continue
    const was = byId.get(part.task_id)
    byId.set(part.task_id, {
      id: part.task_id,
      kind: part.task_kind ?? was?.kind ?? null,
      description: part.description ?? was?.description ?? part.task_id,
      status: part.status,
      summary: part.summary ?? was?.summary ?? null,
    })
  }
  return [...byId.values()]
}

/* What a task is, in a word: the CLI's own word when it is not one of these. */
export function taskLabel(task: BackgroundTask): string {
  if (task.kind === 'local_agent') return t('sessions.task.subagent')
  if (task.kind === 'local_bash') return t('sessions.task.shell')
  return task.kind ? task.kind.replace(/^local_/, '').replace(/^./, (first) => first.toUpperCase()) : t('sessions.task.task')
}

export interface Wakeup {
  readonly id: string
  readonly reason: string
  /** When it fires, as the CLI said it: `15:47:00`, or `in 5m 0s`. */
  readonly when: string
}

/* The wakeup a turn left set. Each `ScheduleWakeup` replaces the one before,
   so it is the last call the CLI did not refuse. */
export function wakeupOf(parts: readonly Part[]): Wakeup | null {
  const refused = new Set(parts.flatMap((part) => (part.kind === 'tool_result' && part.is_error ? [part.call_id] : [])))
  const call = parts.findLast((part) => part.kind === 'tool_call' && part.name === 'ScheduleWakeup' && !part.parent && !refused.has(part.id))
  if (call?.kind !== 'tool_call') return null
  const answer = parts.find((part) => part.kind === 'tool_result' && part.call_id === call.id)
  const at = answer?.kind === 'tool_result' ? /scheduled for (\d{1,2}:\d{2}(?::\d{2})?)/.exec(answer.output)?.[1] : undefined
  let input: { reason?: unknown; delaySeconds?: unknown } = {}
  try {
    input = JSON.parse(call.input) as typeof input
  } catch {
    // Not JSON: the row still says a wakeup is set.
  }
  const delay = typeof input.delaySeconds === 'number' ? t('sessions.wakeup.in', { time: elapsed(0, input.delaySeconds * 1000) }) : ''
  return { id: call.id, reason: typeof input.reason === 'string' ? input.reason : '', when: at ?? delay }
}

/* How long something has been running, in the words the composer uses. */
export function elapsed(since: number, now: number): string {
  const seconds = Math.max(0, Math.floor((now - since) / 1000))
  return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${seconds % 60}s`
}
