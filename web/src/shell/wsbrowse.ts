import type { WorkspaceEntry } from '../gen/bindings'
import { t } from '../i18n'
import { bytes } from './disk'
import { since } from './projects'

/*
 * Reading a folder of the devpit workspace: where you are, what is in it, and
 * in what order.
 *
 * Apart from the panel because all of it is answerable without a screen. The
 * ULID substitution below is the reason this exists at all — every folder
 * devpit makes is named `prj_01M24GHNGDMZCXRFWEDVK387KM`, and a breadcrumb
 * that reads it back is a breadcrumb nobody can use.
 */

export interface Crumb {
  /** What the reader sees. */
  readonly label: string
  /** Relative to the workspace root; empty is the root itself. */
  readonly path: string
}

/** The project whose id the crumbs should read as a name. */
export interface Named {
  readonly id: string
  readonly name: string
}

/** The trail from the workspace root down to `path`, root first. */
export function crumbs(path: string, project: Named | null): readonly Crumb[] {
  const trail: Crumb[] = [{ label: t('workspace.workspacePane.title'), path: '' }]
  let walked = ''
  for (const segment of path.split('/').filter(Boolean)) {
    walked = walked ? `${walked}/${segment}` : segment
    trail.push({
      label: project && segment === project.id ? project.name : segment,
      path: walked,
    })
  }
  return trail
}

/** The folder above, or null at the root — which has none. */
export function parentOf(path: string): string | null {
  if (!path) return null
  const cut = path.lastIndexOf('/')
  return cut === -1 ? '' : path.slice(0, cut)
}

export type Order = 'name' | 'size' | 'modified'

/** Only the entries whose name carries `query`. */
export function matching(
  entries: readonly WorkspaceEntry[],
  query: string,
): readonly WorkspaceEntry[] {
  const wanted = query.trim().toLowerCase()
  if (!wanted) return entries
  return entries.filter((entry) => entry.name.toLowerCase().includes(wanted))
}

/*
 * Folders first, then whichever column was asked for.
 *
 * Folders first even when sorting by size, where they would otherwise all
 * sink to the bottom: a directory reports no size because measuring one means
 * walking it, and sorting on a number we deliberately did not measure would
 * order them by a fact that is not true.
 */
export function sorted(
  entries: readonly WorkspaceEntry[],
  order: Order,
): readonly WorkspaceEntry[] {
  const by = (left: WorkspaceEntry, right: WorkspaceEntry): number => {
    if (order === 'size') return (right.bytes ?? 0) - (left.bytes ?? 0)
    if (order === 'modified') return (right.modified ?? 0) - (left.modified ?? 0)
    return left.name.localeCompare(right.name, undefined, { numeric: true })
  }
  return [...entries].sort(
    (left, right) => Number(right.isDir) - Number(left.isDir) || by(left, right),
  )
}

/*
 * The second line of a row: what it weighs, or what is in it, and when it
 * last changed.
 *
 * A folder says how many entries rather than how many bytes, and an empty one
 * says so — `0 items` and "we could not read it" are different answers, and
 * `disk.ts` already refuses to draw a zero it did not measure.
 */
export function meta(entry: WorkspaceEntry): string {
  const size = entry.isDir
    ? entry.count === 0
      ? 'empty'
      : `${entry.count ?? 0} item${entry.count === 1 ? '' : 's'}`
    : (bytes(entry.bytes ?? 0) ?? 'empty')
  const when = entry.modified ? since(entry.modified / 1000) : null
  return when ? `${size} · ${when}` : size
}

/** The absolute path, for the commands that open and reveal. */
export const fullPath = (root: string, path: string): string =>
  path ? `${root}/${path}` : root

/*
 * Where the arrow keys land.
 *
 * A function rather than arithmetic at the call site so the edges are stated
 * once: from nothing selected, Down takes the first row and Up the last, and
 * neither end wraps — a list that wraps loses the reader's place every time
 * they hold a key down.
 */
export function moved(at: number, step: number, count: number): number {
  if (count === 0) return -1
  if (at < 0) return step > 0 ? 0 : count - 1
  return Math.min(count - 1, Math.max(0, at + step))
}
