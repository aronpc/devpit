import { useEffect, useRef } from 'react'

import type { IslandAsked, Question } from '../gen/bindings'
import { receiptSubject } from './Turn'

/*
 * What a question the agent waits on reads as, and what a key does to it.
 *
 * Read from `touch`, the backend's reading of the tool's input — the same one
 * the island draws — so the chat and the island never disagree about it.
 */

/** One list of labels per question, in the order asked. */
export type Chosen = readonly (readonly string[])[]

/** The questions an `AskUserQuestion` carries, when it carries any. */
export const questionsOf = (question: Pick<Question, 'touch'>): readonly IslandAsked[] | null =>
  question.touch?.kind === 'ask' && question.touch.questions.length > 0 ? question.touch.questions : null

/** What a request is about, for its first line: the command, the file, the address.
    A command whole: a line after the first is what it would hide. */
export function subjectOf(question: Pick<Question, 'input' | 'touch'>): string {
  const touch = question.touch
  if (touch?.kind === 'run') return touch.command
  if (touch && 'path' in touch) return touch.path
  return receiptSubject(question.input)
}

/* Whether "Always in this chat" can be offered: not for a request its CLI
   says asks every time, whose name says nothing of what runs. */
export function alwaysOffered(input: string): boolean {
  try {
    return (JSON.parse(input) as { always?: unknown }).always !== false
  } catch {
    return true
  }
}

/* What a request asks beyond the call itself — write access under another
   root, the network, more permissions — said on the card, never folded away. */
export function grantsOf(input: string): readonly string[] {
  let grants: unknown
  try {
    grants = (JSON.parse(input) as { grants?: unknown }).grants
  } catch {
    return []
  }
  if (!grants || typeof grants !== 'object') return []
  const { grantRoot, networkApprovalContext: net, additionalPermissions: more } = grants as Record<string, unknown>
  const said: string[] = []
  if (typeof grantRoot === 'string') said.push(`Write access under ${grantRoot} for the rest of this session`)
  if (net && typeof net === 'object') {
    const { host, protocol } = net as { host?: unknown; protocol?: unknown }
    said.push(`Network access to ${typeof host === 'string' ? host : 'a host'}${typeof protocol === 'string' ? ` (${protocol})` : ''}`)
  }
  if (more != null) said.push(`More permissions: ${JSON.stringify(more)}`)
  return said
}

/** What a key does on a request's card: Enter allows only on the card itself, where no button would take it too. */
export const allowKey = (key: string, onCard: boolean): 'allow' | 'deny' | null =>
  key === 'Escape' ? 'deny' : key === 'Enter' && onCard ? 'allow' : null

export type Keyed = { readonly pick: number } | { readonly move: number } | 'refuse' | null

/** What a key does on a question with `count` choices, the arrows going on from the one at `at`. */
export function keyOn(key: string, count: number, at: number | null): Keyed {
  if (key === 'Escape') return 'refuse'
  if (/^[1-9]$/.test(key)) return Number(key) <= count ? { pick: Number(key) - 1 } : null
  if (count === 0) return null
  if (key === 'ArrowDown' || key === 'ArrowRight') return { move: at === null ? 0 : (at + 1) % count }
  if (key === 'ArrowUp' || key === 'ArrowLeft') return { move: at === null ? count - 1 : (at - 1 + count) % count }
  return null
}

/** Ticked or not, the choice at `n`; kept in the order asked. */
export const toggled = (ticked: readonly number[], n: number): number[] =>
  ticked.includes(n) ? ticked.filter((one) => one !== n) : [...ticked, n].sort((one, other) => one - other)

/* Words half written keep the keyboard: a digit meant for them would answer. */
const writing = (active: Element | null): boolean =>
  (active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement) && active.value !== ''

/** A card that takes the keyboard when it appears, unless someone is writing. */
export function useFocused<T extends HTMLElement>(): React.RefObject<T | null> {
  const card = useRef<T>(null)
  useEffect(() => {
    if (!writing(document.activeElement)) card.current?.focus()
  }, [])
  return card
}
