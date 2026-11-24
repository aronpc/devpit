import type { FoundIn, Message } from '../gen/bindings'
import { t } from '../i18n'

/*
 * Finding a word in a conversation, all of it.
 *
 * The backend says which messages hold the word and how many times; the
 * window may hold only the last page of them. Going to a match reads the
 * pages above until its message is there, then draws it and paints the word
 * where it is drawn.
 */

/* One match: the message it is in, and which of that message's matches it is. */
export type Occurrence = { readonly messageId: string; readonly nth: number }

export function occurrences(hits: readonly FoundIn[]): Occurrence[] {
  return hits.flatMap((hit) => Array.from({ length: hit.count }, (_, nth) => ({ messageId: hit.messageId, nth })))
}

/* "3 of 17": the match shown, counted from the top, of all of them. */
export function counter(at: number, all: number): string {
  return all === 0 ? t('sessions.find.none') : t('sessions.find.of', { at: at + 1, all })
}

/* The next match from `at`, round the ends either way. */
export function stepped(at: number, all: number, back: boolean): number {
  if (all === 0) return 0
  return (at + (back ? all - 1 : 1)) % all
}

export type Page = { readonly messages: readonly Message[]; readonly older: boolean }

/* Pages a search may read to reach a match: far more than any conversation has. */
const MOST_PAGES = 400

/* The pages above `first` read until one holds `id`: what goes in above what
   is drawn, oldest first, whether older are left, and whether `id` was reached. */
export async function pagesUntil(
  id: string,
  first: string,
  read: (before: string) => Promise<Page | null>,
): Promise<{ messages: Message[]; older: boolean; found: boolean }> {
  const got: (readonly Message[])[] = []
  let before = first
  let older = true
  for (let pages = 0; pages < MOST_PAGES; pages += 1) {
    const page = await read(before)
    if (!page || page.messages.length === 0) break
    got.unshift(page.messages)
    older = page.older
    if (page.messages.some((one) => one.id === id)) return { messages: got.flat(), older, found: true }
    if (!page.older) break
    before = page.messages[0]!.id
  }
  return { messages: got.flat(), older, found: false }
}

/* Where prose is drawn in a row: what the backend searched, and nothing else.
   A subagent's text is drawn inside the work, and is not searched. */
const PROSE = '.md, .said__t'
const WORK = '.acts'

/* Each place `needle` is drawn in the prose under `root`, ignoring case, in order. */
export function rangesIn(root: ParentNode, needle: string): Range[] {
  const want = needle.trim().toLowerCase()
  if (!want) return []
  const found: Range[] = []
  for (const prose of Array.from(root.querySelectorAll(PROSE))) {
    if (prose.parentElement?.closest(PROSE) || prose.closest(WORK)) continue
    const walk = document.createTreeWalker(prose, NodeFilter.SHOW_TEXT)
    for (let node = walk.nextNode(); node; node = walk.nextNode()) {
      const text = (node.nodeValue ?? '').toLowerCase()
      for (let at = text.indexOf(want); at >= 0; at = text.indexOf(want, at + want.length)) {
        const range = document.createRange()
        range.setStart(node, at)
        range.setEnd(node, Math.min(at + want.length, node.nodeValue?.length ?? 0))
        found.push(range)
      }
    }
  }
  return found
}

/* Painted with the CSS Custom Highlight API: nothing in the DOM React draws is touched. */
type Highlights = { set: (name: string, value: unknown) => void; delete: (name: string) => void }
const highlights = (): Highlights | null =>
  typeof CSS !== 'undefined' && 'highlights' in CSS && typeof Highlight !== 'undefined' ? (CSS as unknown as { highlights: Highlights }).highlights : null

export const canPaint = (): boolean => highlights() !== null

/* Paints every match drawn under `root`, and the current one apart. */
export function paint(root: ParentNode, needle: string, current: Range | null): void {
  const all = highlights()
  if (!all) return
  all.set('chat-find', new Highlight(...rangesIn(root, needle)))
  if (current) all.set('chat-find-now', new Highlight(current))
  else all.delete('chat-find-now')
}

export function unpaint(): void {
  highlights()?.delete('chat-find')
  highlights()?.delete('chat-find-now')
}
