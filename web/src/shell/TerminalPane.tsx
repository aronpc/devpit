import { useCallback, useEffect, useRef, useState } from 'react'

import type { LayoutNode, MissingSignIn } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { BlockTerm } from './BlockTerm'
import { LaunchHeld } from './LaunchHeld'
import { usePaneActions } from './paneActions'
import { PaneCorner } from './PaneCorner'
import { Split } from './Split'
import { useShortcut } from './useShortcut'
import { leaves, targetOf } from './splits'
import type { Tab } from './strip'
import { SessionCost } from './SessionCost'
import { useShellPick } from './shellStore'
import { failingWords } from './startChecks'

/*
 * One tab's terminals, arranged as its tree says.
 *
 * The tab owns the tree. It used to be one tree per project, which meant a
 * second terminal tab had to split the first one's to get a leaf of its own —
 * so three tabs were three leaves under two split nodes, drawn as three tabs.
 * A list wearing a tree's clothes. Now a tab is a tree, and splitting divides
 * what one tab shows, which is what the word means.
 */

export function TerminalPane({ tab, projectId }: { tab: Tab; projectId: string }): React.JSX.Element {
  const { attach, launched, agents } = useShellPick((shell) => ({ attach: shell.attach, launched: shell.launched, agents: shell.agentSessions }))
  const [tree, setTree] = useState<LayoutNode | null>(null)
  const [focused, setFocused] = useState('')
  const [error, setError] = useState<string | null>(null)
  /* Something went wrong beside the terminal rather than instead of it. */
  const [notice, setNotice] = useState<string | null>(null)

  /* `regrouped` is a join rewriting this tree from the strip. */
  useEffect(() => {
    if (!projectId) return
    let dropped = false
    void ask(() => commands.sessionEnsure(projectId, tab.id, null)).then((answer) => {
      if (dropped) return
      if (!answer.data) return setError(answer.error ?? 'could not open a terminal')
      setTree(answer.data.tree)
      setFocused(answer.data.focusedId)
    })
    return () => {
      dropped = true
    }
  }, [projectId, tab.id, tab.regrouped])

  const split = useCallback(
    (direction: 'horizontal' | 'vertical') => {
      if (!projectId || !focused) return
      void ask(() => commands.sessionSplit(projectId, tab.id, focused, direction, null)).then(
        (answer) => {
          if (!answer.data) return setError(answer.error ?? 'could not split')
          setTree(answer.data.tree)
          setFocused(answer.data.focusedId)
        },
      )
    },
    [projectId, tab.id, focused],
  )

  /* The shortcuts every terminal with splits uses. Only while this tab is the
     one in front: every tab stays mounted, and each used to split on a key
     meant for the visible one. */
  const shown = useShellPick((shell) => shell.active?.id === tab.id && shell.project?.id === projectId)
  useShortcut('splitRight', () => split('horizontal'), shown)
  useShortcut('splitDown', () => split('vertical'), shown)

  const settle = useCallback(
    (splitId: string, ratio: number) => {
      if (!projectId) return
      void ask(() => commands.sessionSetRatio(projectId, tab.id, splitId, ratio)).then(
        (answer) => {
          if (answer.data) setTree(answer.data.tree)
        },
      )
    },
    [projectId, tab.id],
  )

  const focus = useCallback(
    (leafId: string) => {
      setFocused((was) => {
        if (was === leafId || !projectId) return was
        void ask(() => commands.sessionFocus(projectId, tab.id, leafId))
        return leafId
      })
    },
    [projectId, tab.id],
  )

  /* The tab is told which leaves it is showing, because the strip and the
     sidebar draw this tab and neither can see the tree. Nothing ever told
     them, so a terminal with an agent open looked exactly like an empty one
     in both places. */
  useEffect(() => {
    if (tree) attach(tab.id, leaves(tree))
  }, [tree, tab.id, attach])

  /* A terminal opened to run an agent runs it once its pane exists.

     Both a ref and the tab's own state, and neither alone is enough: the
     state is what stops a later render from sending it again, and the ref is
     what stops the second of React's two development mounts from sending it
     before that state has landed. Two sends is two `claude` lines typed, the
     second one into the first one. */
  const sent = useRef(false)
  /* A start held for a sign-in: the pane it was for, and what it lacks. */
  const [held, setHeld] = useState<{ paneId: string; agent: string; missing: MissingSignIn[] } | null>(null)
  const start = useCallback(
    (paneId: string, agent: string, anyway: boolean) => {
      setHeld(null)
      void ask(() => commands.sessionLaunchAgent(projectId, paneId, agent, anyway)).then((answer) => {
        /* A notice, not the fatal error. The terminal is open and working
           whether or not the agent was started for you, and replacing a working
           terminal with a sentence takes away the one thing that still lets you
           type the command yourself. */
        if (!answer.data) return setNotice(answer.error)
        if (answer.data.line === null) return setHeld({ paneId, agent, missing: answer.data.missing })
        setNotice(failingWords(answer.data.failing))
      })
    },
    [projectId],
  )
  useEffect(() => {
    if (!projectId || !focused || !tab.launch || sent.current) return
    sent.current = true
    launched(tab.id)
    start(focused, tab.launch, false)
  }, [projectId, focused, tab.launch, tab.id, launched, start])
  /* A terminal opened to sign a CLI in types its login, for the person to finish. */
  useEffect(() => {
    const login = tab.login
    if (!projectId || !focused || !login || sent.current) return
    sent.current = true
    launched(tab.id)
    void ask(() => commands.agentLogin(projectId, focused, login)).then((answer) => answer.error && setNotice(answer.error))
  }, [projectId, focused, tab.login, tab.id, launched])

  const layoutTo = useCallback((next: LayoutNode, focusedId: string) => {
    setTree(next)
    setFocused(focusedId)
  }, [])
  const pane = usePaneActions({ tab, tree, focused, onLayout: layoutTo, onNotice: setNotice })

  if (error) return <div className="exempty__t">{error}</div>
  if (!tree) return <div className="termhost termhost--starting">{t('workspace.terminal.starting')}</div>

  return (
    <>
      <PaneCorner
        tabId={tab.id}
        what="terminal"
        onSplit={split}
        onChat={pane.toChat ?? undefined}
        onClose={pane.closePane}
        closeLabel={pane.closeLabel}
        closeArmed={pane.armed}
      >
        <SessionCost sessionId={agents[focused]?.sessionId} className="pcorner__cost scost" />
      </PaneCorner>
      {notice && (
        <button className="tnote" onClick={() => setNotice(null)} title={t('workspace.terminal.dismiss')}>
          {notice}
        </button>
      )}
      {held && (
        <LaunchHeld
          missing={held.missing}
          onAgain={() => start(held.paneId, held.agent, false)}
          onAnyway={() => start(held.paneId, held.agent, true)}
          onDismiss={() => setHeld(null)}
        />
      )}
      <Split
        node={tree}
        focused={focused}
        onFocus={focus}
        onRatio={settle}
        leaf={(leafId) => (
          <BlockTerm
            key={leafId}
            paneId={leafId}
            projectId={projectId}
            target={targetOf(tree, leafId)}
            onSplit={split}
            onClosePane={pane.closePane}
            onSeparate={pane.separate ?? undefined}
          />
        )}
      />
    </>
  )
}
