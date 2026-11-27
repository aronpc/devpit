import { useState, useSyncExternalStore } from 'react'

import { Menu, MenuItem } from './Menu'
import { menuPoint } from './menuRules'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * Which details a conversation's header and its "where" line show, chosen
 * from a right-click on either. Kept per viewer; all show until hidden.
 */

export const DETAILS = [
  { id: 'card', label: 'sessions.details.card' },
  { id: 'context', label: 'sessions.details.context' },
  { id: 'cost', label: 'sessions.details.cost' },
  { id: 'project', label: 'sessions.details.project' },
  { id: 'machine', label: 'sessions.details.machine' },
  { id: 'branch', label: 'sessions.details.branch' },
] as const

export type Detail = (typeof DETAILS)[number]['id']

const KEY = 'devpit.chatDetails'

function read(): ReadonlySet<Detail> {
  try {
    const kept: unknown = JSON.parse(localStorage.getItem(KEY) ?? '[]')
    return new Set(Array.isArray(kept) ? kept.filter((one): one is Detail => DETAILS.some((detail) => detail.id === one)) : [])
  } catch {
    return new Set()
  }
}

let hidden: ReadonlySet<Detail> | null = null
const told = new Set<() => void>()

const hiddenNow = (): ReadonlySet<Detail> => (hidden ??= read())

export function toggleDetail(id: Detail): void {
  const next = new Set(hiddenNow())
  if (next.has(id)) next.delete(id)
  else next.add(id)
  hidden = next
  try {
    localStorage.setItem(KEY, JSON.stringify([...next]))
  } catch {
    // Refused: the choice holds until the window closes.
  }
  for (const listener of told) listener()
}

/** Forgets the window's copy, so the next read is from storage. For tests. */
export const rereadDetails = (): void => {
  hidden = null
}

function subscribe(listener: () => void): () => void {
  told.add(listener)
  return () => told.delete(listener)
}

/** Whether a detail is shown, read again when one is toggled. */
export function useDetails(): (id: Detail) => boolean {
  const now = useSyncExternalStore(subscribe, hiddenNow)
  return (id) => !now.has(id)
}

const TICK = (
  <Icon size="s"><path d="m5 13 4 4L19 7" /></Icon>
)

/* A right-click on the header or on this line picks which details they show. */
export function useDetailsMenu(): { open: (event: React.MouseEvent) => void; menu: React.JSX.Element | null } {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null)
  const shows = useDetails()
  const open = (event: React.MouseEvent): void => {
    /* A text field keeps the system's cut, copy and paste. */
    if ((event.target as Element).closest('input, textarea, [contenteditable="true"]')) return
    event.preventDefault()
    setAt(menuPoint(event))
  }
  const menu = at && (
    <Menu at={at} label={t('sessions.details.shown')} onClose={() => setAt(null)}>
      {DETAILS.map((one) => (
        <MenuItem key={one.id} label={t(one.label)} glyph={shows(one.id) ? TICK : undefined} onPick={() => toggleDetail(one.id)} />
      ))}
    </Menu>
  )
  return { open, menu }
}
