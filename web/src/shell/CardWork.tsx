import { useCallback, useState } from 'react'

import type { Checkout, Run } from '../gen/bindings'
import { t } from '../i18n'
import { AgentMark } from './AgentMark'
import { ask, commands } from './live'
import { OpenIn } from './OpenIn'
import { openCardChat, openCardTerminal } from './useCardActs'
import { offered, useKnownAgents } from './useKnownAgents'
import { useOpeners } from './useOpeners'
import { useShell } from './useShell'
import { money } from './chat'

/*
 * Where this card's work happens, and how to start doing it.
 *
 * The join the board was missing. A card described a piece of work and had no
 * way to begin it: the checkout was made only as a side-effect of a step
 * running, and the terminal knew nothing about cards. Both already existed;
 * neither was reachable from here.
 *
 * "Open a terminal" makes the checkout if there is none, because a person
 * pressing it is asking for exactly that — a place to work, in this card's own
 * branch, not an error about a folder they never heard of.
 */

export function CardWork({
  cardId,
  title,
  worktree,
  runs,
  onChanged,
}: {
  cardId: string
  /** What the card's terminal tab is called: the card, not its branch. */
  title: string
  worktree: Checkout | null
  runs: readonly Run[]
  onChanged: () => void
}): React.JSX.Element {
  const { project, show, open } = useShell()
  /* Only what Settings still offers. */
  const agents = offered(useKnownAgents())
  const openers = useOpeners()
  const [busy, setBusy] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  const make = async (): Promise<void> => {
    setBusy(t('card.makingCheckout'))
    const answer = await ask(() => commands.cardCheckout(cardId))
    setBusy(null)
    setProblem(answer.error)
    if (!answer.error) onChanged()
  }

  /* The agent is carried on the tab rather than sent here: the pane does not
     exist until the window has drawn it. The launcher palette already works
     this way, and a second way in would be a second thing to keep right. */
  const openTerminalFor = useCallback(async (agentId?: string): Promise<void> => {
    if (!project) return
    setBusy(t('card.openingTerminal'))
    const refused = await openCardTerminal(project.id, cardId, title, show, agentId, open)
    setBusy(null)
    setProblem(refused)
    if (refused) return
    onChanged()
  }, [project, cardId, title, show, open, onChanged])

  /* A conversation about this card, in its checkout, with the card in it. */
  const chat = async (): Promise<void> => {
    if (!project) return
    setBusy(t('card.openingChat'))
    const refused = await openCardChat(project.id, cardId, show)
    setBusy(null)
    setProblem(refused)
  }

  return (
    <section className="cwork">
      <h2 className="cardp__h">{t('card.work')}</h2>

      {worktree ? (
        <div className="cwork__wt" data-gone={!worktree.exists}>
          <span className="cwork__b">
            <span className="cwork__t">
              {worktree.branch ?? t('card.aCheckout')}
              {worktree.dirtyFiles !== null && worktree.dirtyFiles > 0 && (
                <span className="cwork__dirty">
                  {t('card.uncommitted', { count: worktree.dirtyFiles })}
                </span>
              )}
            </span>
            <span className="cwork__p" title={worktree.path}>{worktree.path}</span>
          </span>
          {!worktree.exists && <span className="oapp__no">{t('card.notOnDisk')}</span>}
          {worktree.exists && (
            <span className="pin__acts">
              <button className="btn" onClick={() => void ask(() => commands.pathReveal(worktree.path))}>
                {t('card.reveal')}
              </button>
              <OpenIn apps={openers} path={worktree.path} />
            </span>
          )}
        </div>
      ) : (
        <p className="pref__d">
          {t('card.noCheckout')}
        </p>
      )}

      <div className="cwork__go">
        {!worktree?.exists && (
          <button className="btn" disabled={Boolean(busy)} onClick={() => void make()}>
            {t('card.makeCheckout')}
          </button>
        )}
        <button className="btn" disabled={Boolean(busy)} onClick={() => void openTerminalFor()}>
          {t('card.openTerminal')}
        </button>
        <button className="btn" disabled={Boolean(busy)} onClick={() => void chat()}>
          {t('card.chatAbout')}
        </button>
        {/* Only the agents this machine actually has. One it cannot run is a
            button that fails, and the list already knows which those are. */}
        {agents
          .filter((agent) => agent.installed)
          .map((agent) => (
            <button
              className="btn"
              key={agent.id}
              disabled={Boolean(busy)}
              onClick={() => void openTerminalFor(agent.id)}
            >
              <AgentMark agent={agent.id} />
              {agent.label}
            </button>
          ))}
      </div>

      {busy && <p className="pref__d">{busy}</p>}
      {problem && <p className="wtb__no">{problem}</p>}

      {runs.length > 0 && (
        <>
          <h2 className="cardp__h">{t('card.runs')}</h2>
          {runs.map((run) => (
            <div className="crun" key={run.id} data-state={run.state}>
              <span className="crun__b">
                <span className="crun__t">{run.stepName}</span>
                {run.output && <span className="crun__o">{run.output.slice(0, 400)}</span>}
              </span>
              <span className="crun__s">{run.state}</span>
              {/* A command step has no cost at all, so most runs would carry
                  a $0.00 that means "this kind of step does not spend". */}
              {money(run.costUsd ?? 0) && (
                <span className="crun__c">{money(run.costUsd ?? 0)}</span>
              )}
              {run.state === 'running' && (
                <button
                  className="btn"
                  data-danger
                  onClick={() => void ask(() => commands.runCancel(cardId, run.id)).then(onChanged)}
                >
                  {t('card.stop')}
                </button>
              )}
            </div>
          ))}
        </>
      )}
    </section>
  )
}
