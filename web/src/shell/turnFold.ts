import type { Ended, Message, Part, TurnEnd } from '../gen/bindings'
import { t } from '../i18n'

/*
 * A turn in the order it happened: the agent's prose, and the work between
 * it. Once the turn is over the work folds behind the last prose, which is the
 * answer; a turn that failed stays open on its error.
 */

export type Segment =
  | { readonly kind: 'prose'; readonly at: number; readonly text: string }
  | { readonly kind: 'said'; readonly at: number; readonly content: string }
  | { readonly kind: 'work'; readonly at: number; readonly parts: readonly Part[] }

/* `parts` already joined, so `at` stays the same as more arrives. */
export function segments(parts: readonly Part[]): readonly Segment[] {
  const out: Segment[] = []
  let work: Part[] | null = null
  parts.forEach((part, at) => {
    if (part.kind === 'text' && !part.parent) {
      work = null
      out.push({ kind: 'prose', at, text: part.text })
    } else if (part.kind === 'command') {
      work = null
      out.push({ kind: 'said', at, content: part.content })
    } else if (work) {
      work.push(part)
    } else {
      work = [part]
      out.push({ kind: 'work', at, parts: work })
    }
  })
  return out
}

/* The segment the folded turn keeps showing: its last prose, if any. */
export function answerOf(segments: readonly Segment[]): number {
  return segments.findLastIndex((one) => one.kind === 'prose')
}

export type Outcome = 'done' | 'stopped' | 'failed'

/* `interrupted` is also a process that died without an end frame: it stopped
   either way, and what it said on stderr is already in the answer. */
export function outcome(ended: Ended): Outcome {
  if (ended.error) return 'failed'
  if (ended.stopReason === 'interrupted' || ended.stopReason === 'cancelled') return 'stopped'
  return ended.isError ? 'failed' : 'done'
}

/* Milliseconds since the epoch. A person's turn is written in milliseconds and
   a woken one in seconds; 0 is a message whose time was never known. */
export function millis(at: number | null | undefined): number | null {
  if (!at) return null
  return at < 1e11 ? at * 1000 : at
}

export function took(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000))
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ${seconds % 60}s`
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`
}

/* The note a compaction leaves in the thread: "Compacted at 212k tokens". */
export function compactedAt(tokens: number): string {
  return t('sessions.turn.compacted', { tokens: Math.round(tokens / 1000) })
}

const VERBS: Readonly<Record<Outcome, readonly [string, string]>> = {
  done: ['sessions.turn.workedFor', 'sessions.turn.worked'],
  stopped: ['sessions.turn.interruptedAfter', 'sessions.turn.interrupted'],
  failed: ['sessions.turn.failedAfter', 'sessions.turn.failed'],
}

/* "Worked for 1m 30s", or the word alone when the start was never known. */
export function statusOf(message: Pick<Message, 'createdAt' | 'ended'>): string | null {
  const ended = message.ended
  if (!ended) return null
  const [timed, bare] = VERBS[outcome(ended)]
  const began = millis(message.createdAt)
  const end = millis(ended.at)
  return began !== null && end !== null && end >= began ? t(timed, { time: took(end - began) }) : t(bare)
}

/* How the live end frame reads, kept on the answer like a saved one. */
export function endedNow(end: Pick<TurnEnd, 'stopReason' | 'isError'> & Partial<TurnEnd>, error: string | null = null): Ended {
  return {
    at: Date.now() / 1000,
    stopReason: end.stopReason,
    isError: end.isError,
    error,
    unheard: end.unheard ?? [],
    model: end.model ?? null,
    effort: end.effort ?? null,
    costUsd: end.costUsd ?? null,
    durationMs: end.durationMs == null ? null : Math.round(end.durationMs),
    tokens: end.context?.used ?? null,
    limit: end.limit ?? null,
  }
}
