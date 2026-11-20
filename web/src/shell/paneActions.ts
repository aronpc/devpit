import { useCallback, useEffect, useState } from 'react'

import type { LayoutNode } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { profileForInstallation } from './outside'
import { leaves } from './splits'
import type { Tab } from './strip'
import { useShell } from './useShell'

/*
 * What the corner of a terminal does to the pane in front.
 *
 * Close used to close the tab, so in a tab split into three every pane went
 * with the one you meant. Now a pane that is one of several closes alone, and
 * only the last one closes the tab.
 */

export interface PaneActions {
  closePane: () => void
  closeLabel: string
  /* The close is waiting for a second press before it stops what runs. */
  armed: boolean
  /* Goes on with the pane's agent conversation in a chat, when its session is known. */
  toChat: (() => void) | null
  /* Moves the pane in front to a tab of its own; null when it is the only one. */
  separate: (() => void) | null
}

export function usePaneActions({
  tab,
  tree,
  focused,
  onLayout,
  onNotice,
}: {
  tab: Tab
  tree: LayoutNode | null
  focused: string
  onLayout: (tree: LayoutNode, focusedId: string) => void
  onNotice: (text: string) => void
}): PaneActions {
  const { project, running, close, closeNow, show, agentSessions } = useShell()
  const [armedFor, setArmedFor] = useState<string | null>(null)
  const several = tree ? leaves(tree).length > 1 : false
  const busy = running.find((one) => one.paneId === focused && (one.agent !== null || one.busy))
  const armed = armedFor === focused && Boolean(busy)

  /* A second press long after the first is a new intention, not a confirmation. */
  useEffect(() => {
    if (!armedFor) return
    const timer = window.setTimeout(() => setArmedFor(null), 3000)
    return () => window.clearTimeout(timer)
  }, [armedFor])

  const closeLeaf = useCallback(async (): Promise<boolean> => {
    if (!project) return false
    const answer = await ask(() => commands.sessionCloseLeaf(project.id, tab.id, focused))
    if (!answer.data) {
      onNotice(answer.error ?? 'could not close that pane')
      return false
    }
    onLayout(answer.data.tree, answer.data.focusedId)
    return true
  }, [project, tab.id, focused, onLayout, onNotice])

  const closePane = useCallback(() => {
    /* The last pane is the tab, and closing a tab already asks about what it stops. */
    if (!several) return close(tab.id)
    if (busy && !armed) return setArmedFor(focused)
    setArmedFor(null)
    void closeLeaf()
  }, [several, close, tab.id, busy, armed, focused, closeLeaf])

  /* The tab id is minted here so the tree is filed under it before the tab
     opens and asks for it — the other order would give the tab a new shell. */
  const separate = useCallback(() => {
    if (!project) return
    const id = crypto.randomUUID()
    void ask(() => commands.sessionSeparateLeaf(project.id, tab.id, focused, id)).then((answer) => {
      if (!answer.data) return onNotice(answer.error ?? 'could not move that pane')
      onLayout(answer.data.tree, answer.data.focusedId)
      show('term', { id })
    })
  }, [project, tab.id, focused, onLayout, onNotice, show])

  const known = agentSessions[focused]
  const toChat = useCallback(() => {
    if (!project || !known) return
    void (async () => {
      const [installs, found] = await Promise.all([
        ask(() => commands.cliInstallations()),
        ask(() => commands.agentProfiles()),
      ])
      const profile = profileForInstallation(known.installation, installs.data ?? [], found.data ?? [])
      if (!profile) return onNotice(`No profile runs the installation this agent uses: ${known.installation}`)
      const adopted = await ask(() => commands.chatAdopt(project.id, known.sessionId, profile.id, null, tab.cardId ?? null))
      if (!adopted.data) return onNotice(adopted.error ?? 'could not open the chat')
      /* The agent here stops before the chat speaks: two CLIs resuming one
         session would each write their own continuation of it. */
      if (several) {
        if (!(await closeLeaf())) return
      } else {
        closeNow(tab.id)
      }
      show('chat', { id: adopted.data })
    })()
  }, [project, known, onNotice, several, closeLeaf, closeNow, tab.id, tab.cardId, show])

  const closeLabel = armed && busy ? t('workspace.pane.pressAgain', { label: busy.label }) : several ? t('workspace.terminal.closePane') : t('workspace.pane.closeTerminal')

  return { closePane, closeLabel, armed, toChat: known ? toChat : null, separate: several ? separate : null }
}
