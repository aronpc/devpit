import { useMemo } from 'react'

import type { Card } from '../gen/bindings'
import { t } from '../i18n'
import { CARD, DOC } from './paletteIcons'
import { ranked } from './search'
import type { PaneName } from './paneList'
import type { Tab } from './strip'

export interface Row {
  readonly key: string
  readonly name: string
  readonly meta: string
  readonly icon: React.JSX.Element
  readonly go: () => void
}

/* Which groups the field shows, and in which order. Panes first because they
   always exist; files last because there are thousands of them. */
export function useGroups({
  panes,
  agents,
  sessions,
  conversations = [],
  cards,
  files,
  query,
  show,
}: {
  panes: Row[]
  agents: Row[]
  sessions: Row[]
  /** What was said, in every project: ranked by the search that found it. */
  conversations?: Row[]
  cards: readonly Card[]
  files: readonly string[]
  query: string
  show: (kind: PaneName, tab?: Partial<Tab>) => void
}): [string, Row[]][] {
  return useMemo(() => {
    const asRows = {
      Panes: ranked(panes, query, (row) => row.name),
      /* Above the sessions you have and below the panes you can open: an
         agent is a new thing to start, which is what the top of this list is
         for, and it is the thing people came to the plus button to do. */
      Agents: ranked(agents, query, (row) => row.name),
      Sessions: ranked(sessions, query, (row) => row.name),
      Conversations: conversations,
      Cards: ranked(cards, query, (card) => card.title).map((card) => ({
        key: `card:${card.id}`,
        name: card.title,
        meta: t('workspace.palette.card'),
        icon: CARD,
        go: () => show('board'),
      })),
      Files: ranked(files, query, (path) => path).map((path) => ({
        key: `file:${path}`,
        name: path.split('/').pop() ?? path,
        meta: path,
        icon: DOC,
        go: () => show('file', { id: `file:${path}`, path, title: path.split('/').pop() }),
      })),
    }
    /* A group with nothing in it leaves rather than showing a heading over
       nothing. */
    return Object.entries(asRows).filter(([, rows]) => rows.length > 0)
  }, [panes, agents, sessions, conversations, cards, files, query, show])
}
