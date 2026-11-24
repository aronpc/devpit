import type { Commit } from '../gen/bindings'
import { t } from '../i18n'
import { formatDate } from '../i18n/format'

/*
 * The rules behind the History tab, apart from the drawing.
 */

/** A conventional commit's kind, when its subject declares one. */
export interface Subject {
  readonly kind: string | null
  readonly scope: string | null
  /** A `!` after the kind: a breaking change. */
  readonly breaking: boolean
  readonly text: string
}

/* `feat(rail)!: text` — the prefix a lot of repositories write, this one
   included. Anything else is a subject with no kind, shown as written. */
const CONVENTIONAL = /^([a-z]+)(?:\(([^)]+)\))?(!)?:\s+(.+)$/i

export function subjectOf(subject: string): Subject {
  const match = CONVENTIONAL.exec(subject)
  if (!match) return { kind: null, scope: null, breaking: false, text: subject }
  return { kind: match[1]!.toLowerCase(), scope: match[2] ?? null, breaking: match[3] === '!', text: match[4]! }
}

/** The heading a commit sits under: today, yesterday, a weekday this week, or its date. */
export function dayOf(atSeconds: number | null, now: Date): string {
  if (atSeconds === null) return t('workspace.history.undated')
  const at = new Date(atSeconds * 1000)
  const midnight = (day: Date): number => new Date(day.getFullYear(), day.getMonth(), day.getDate()).getTime()
  const days = Math.round((midnight(now) - midnight(at)) / 86_400_000)
  if (days <= 0) return t('workspace.history.today')
  if (days === 1) return t('workspace.history.yesterday')
  if (days < 7) return formatDate(at, { weekday: 'long' })
  return formatDate(at, {
    month: 'short',
    day: 'numeric',
    ...(at.getFullYear() === now.getFullYear() ? {} : { year: 'numeric' }),
  })
}

/** Commits under their day's heading, in the order they came. */
export function byDay(commits: readonly Commit[], now: Date): readonly { day: string; commits: readonly Commit[] }[] {
  const out: { day: string; commits: Commit[] }[] = []
  for (const commit of commits) {
    const day = dayOf(commit.committedAt, now)
    const last = out[out.length - 1]
    if (last?.day === day) last.commits.push(commit)
    else out.push({ day, commits: [commit] })
  }
  return out
}

/* An older page appended, without the ones already shown. Pages are asked by
   offset, and a commit made between two asks shifts every offset by one — the
   page after it repeated the last commit of the page before. */
export function appended(was: readonly Commit[], page: readonly Commit[]): readonly Commit[] {
  const seen = new Set(was.map((one) => one.sha))
  return [...was, ...page.filter((one) => !seen.has(one.sha))]
}

/** Whether a commit matches what was typed in the filter: subject, author or sha. */
export function matches(commit: Commit, query: string): boolean {
  const wanted = query.trim().toLowerCase()
  if (!wanted) return true
  return `${commit.subject} ${commit.author} ${commit.sha}`.toLowerCase().includes(wanted)
}

/** How long ago, short: `5m`, `3h`, `2d`, `4w`. */
export function since(atSeconds: number | null, nowMs: number): string {
  if (atSeconds === null) return ''
  const minutes = Math.max(0, Math.floor((nowMs - atSeconds * 1000) / 60_000))
  if (minutes < 1) return 'now'
  if (minutes < 60) return `${minutes}m`
  if (minutes < 1_440) return `${Math.floor(minutes / 60)}h`
  if (minutes < 10_080) return `${Math.floor(minutes / 1_440)}d`
  if (minutes < 43_200) return `${Math.floor(minutes / 10_080)}w`
  return `${Math.floor(minutes / 43_200)}mo`
}
