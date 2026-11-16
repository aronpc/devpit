import { useEffect, useRef, useState } from 'react'

import type { PaneName } from './paneList'
import type { Tab } from './strip'
import { ask, commands } from './live'
import { onCarried } from './window'

/*
 * The island asking for a session: devpit is brought forward by Rust, and
 * this opens it — a terminal's pane, the way a notice does, or a chat's
 * conversation, once its project is the one on screen. A launch that asked
 * for a pane with `--open-pane` comes through the same opening: asked of
 * Rust once as this mounts, because the ask happened before any document
 * was there to hear it.
 */
export function useIslandOpens(
  current: string | null,
  setProject: (projectId: string) => void,
  openPane: (paneId: string | null) => void,
  show: (kind: PaneName, tab?: Partial<Tab>) => void,
): void {
  const held = useRef({ current, setProject, openPane })
  held.current = { current, setProject, openPane }
  const [wanted, setWanted] = useState<{ projectId: string; conversationId: string } | null>(null)

  useEffect(
    () =>
      onCarried<[string | null, string]>('island:open-pane', ([projectId, paneId]) => {
        const now = held.current
        if (projectId && projectId !== now.current) now.setProject(projectId)
        now.openPane(paneId)
      }),
    [],
  )

  useEffect(
    () =>
      onCarried<[string, string | null]>('island:open-chat', ([projectId, conversationId]) => {
        const now = held.current
        if (projectId !== now.current) now.setProject(projectId)
        // An orchestrator's project is its chat; nothing more to open.
        if (conversationId) setWanted({ projectId, conversationId })
      }),
    [],
  )

  /* The launch's own ask, taken once: Rust kept it until a document was
     there to ask, and answers nothing on a reload's second ask. */
  useEffect(() => {
    void ask(() => commands.islandPendingPane()).then((answer) => {
      const pane = answer.data
      if (!pane) return
      const now = held.current
      if (pane.projectId !== now.current) now.setProject(pane.projectId)
      now.openPane(pane.paneId)
    })
  }, [])

  /* The conversation opens in its own project's strip, which is there only
     after the switch has landed. */
  useEffect(() => {
    if (!wanted || wanted.projectId !== current) return
    show('chat', { id: wanted.conversationId })
    setWanted(null)
  }, [wanted, current, show])
}
