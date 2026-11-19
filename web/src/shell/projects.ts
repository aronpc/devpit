import type { Project } from '../gen/bindings'
import { formatRelative } from '../i18n/format'

/* What removing a project does to the list and to where you are standing.
   A function, so the test calls the rule instead of restating it. */
export interface Open {
  readonly projects: readonly Project[]
  readonly current: string | null
}

export function forgotten(open: Open, id: string): Open {
  const projects = open.projects.filter((other) => other.id !== id)
  return {
    projects,
    /* Removing the ground you stand on has to land somewhere, or the window
       keeps naming a project that is no longer listed. */
    current: open.current === id ? (projects[0]?.id ?? null) : open.current,
  }
}

/* A list a rename or a forget answers reads no checkout's status, so the
   project on screen keeps the checkouts it had. */
export function adopted(was: Open, next: readonly Project[]): readonly Project[] {
  const shown = was.projects.find((project) => project.id === was.current)
  return next.map((project) => (shown && project.id === shown.id ? { ...project, worktrees: shown.worktrees } : project))
}

export const found = (open: Open): Project | null =>
  open.projects.find((project) => project.id === open.current) ?? null

/*
 * When a project was last opened, against the reader's own clock.
 *
 * The column was always there and always empty: `last_opened_at` orders the
 * list, so the one row at the top is the one you used last and nothing said
 * whether that was an hour or a year ago.
 *
 * Relative rather than a date, because that is the question being asked — and
 * through `Intl` rather than a table of English words, so a reader whose
 * machine is not in English does not get one column that is.
 */
const SCALES: readonly (readonly [Intl.RelativeTimeFormatUnit, number])[] = [
  ['year', 365 * 24 * 3600],
  ['month', 30 * 24 * 3600],
  ['week', 7 * 24 * 3600],
  ['day', 24 * 3600],
  ['hour', 3600],
  ['minute', 60],
]

export function since(seconds: number | null, now = Date.now(), locale?: string): string {
  if (!seconds) return 'never opened'

  const elapsed = Math.round(now / 1000 - seconds)
  /* A clock that disagrees with the file is not news worth a row saying
     "in 3 hours", so anything inside the minute is simply now. */
  if (elapsed < 60) return 'just now'

  for (const [unit, size] of SCALES) {
    if (elapsed >= size) return formatRelative(-Math.floor(elapsed / size), unit, { numeric: 'auto' }, locale)
  }
  return 'just now'
}

/*
 * The remote, short enough to be a line in a list.
 *
 * `origin_url` was read when the project was registered and then never shown,
 * so two clones of one repository looked like two unrelated folders that
 * happened to share a name. The scheme, the credentials and the `.git` are
 * noise here — `github.com/owner/name` is the part that identifies it.
 */
export function remote(origin: string | null): string | null {
  const raw = origin?.trim()
  if (!raw) return null
  const bare = raw
    .replace(/^[a-z+]+:\/\//i, '')
    .replace(/^git@/i, '')
    .replace(/^[^/@]+@/, '')
    .replace(/\.git$/i, '')
    .replace(/\/+$/, '')
  /* `git@host:org/repo` uses a colon where a URL uses a slash. */
  return bare.replace(':', '/') || null
}
