import { useEffect, useState } from 'react'

import type { HeadsDown, Notice, ProjectRun } from '../gen/bindings'
import { t } from '../i18n'
import { minutesIn, whatYouDid } from './headsDown'
import { ask, commands } from './live'

/*
 * What the focus held, shown on the way out.
 *
 * This is the other half of the promise: nothing that arrived while the door
 * was shut is dropped, and it is not shown as a wall of rings the moment the
 * focus ends either — it is one list, grouped by the project it came from,
 * because three things from one subsystem read as one thing and three things
 * from three read as three.
 *
 * Read through `focus.waiting` rather than from the bell's own list: the bell
 * holds a page of the newest, and a focus is exactly the case where what
 * matters fell off the end of it.
 *
 * Nothing here is marked read by being shown. Acting on one is what reads it.
 */

/** The order somebody coming back wants: what is waiting on them, then what
 *  broke, then what is ready, then what is only telling them something. */
const RANK: Readonly<Record<string, number>> = { agent: 0, irreversible: 1, run: 2 }
const rankOf = (kind: string): number => RANK[kind] ?? 9

export function FocusSummary({
  ended,
  onOpenCard,
  onClose,
}: {
  ended: HeadsDown
  onOpenCard: (cardId: string) => void
  onClose: () => void
}): React.JSX.Element | null {
  const [waiting, setWaiting] = useState<Notice[]>([])
  const [more, setMore] = useState(false)
  const [read, setRead] = useState(false)
  /* The other half: what this project did while the door was shut. Read with
     the same instant the door was shut at, which RunsQuery already takes. */
  const [mine, setMine] = useState<ProjectRun[]>([])

  useEffect(() => {
    if (ended.since === null) return
    let dropped = false
    void (async () => {
      /* Walked forward one page at a time, so a focus that lasted a day still
         shows every one exactly once. Bounded, because a summary nobody can
         scroll is a summary nobody reads. */
      const all: Notice[] = []
      let after: string | undefined
      let over = false
      for (let page = 0; page < 5; page += 1) {
        const answer = await ask(() =>
          commands.focusWaiting(ended.projectId, ended.since as number, after ?? null),
        )
        const heard = answer.data
        if (!heard || heard.notices.length === 0) break
        all.push(...heard.notices)
        after = heard.notices[heard.notices.length - 1]!.id
        over = heard.more
        if (!heard.more) break
      }
      const ran = await ask(() =>
        commands.runsList({
          projectId: ended.projectId,
          stepId: null,
          state: null,
          since: ended.since as number,
          until: null,
          after: null,
        }),
      )

      if (dropped) return
      setWaiting(all)
      setMore(over)
      setMine(ran.data?.runs ?? [])
      setRead(true)
    })()
    return () => {
      dropped = true
    }
  }, [ended])

  if (!read) return null

  const byProject = new Map<string, Notice[]>()
  for (const one of [...waiting].sort(
    (a, b) => rankOf(a.kind) - rankOf(b.kind) || (a.createdAt ?? 0) - (b.createdAt ?? 0),
  )) {
    const group = byProject.get(one.projectId ?? '') ?? []
    group.push(one)
    byProject.set(one.projectId ?? '', group)
  }

  const did = whatYouDid(mine)
  const spent =
    did.costUsd === null
      ? null
      : did.uncosted > 0
        ? t('workspace.focus.spentPartial', { amount: `$${did.costUsd.toFixed(2)}` })
        : `$${did.costUsd.toFixed(2)}`

  return (
    <div className="fsum" role="dialog" aria-label={t('workspace.focus.ended')}>
      <div className="fsum__box">
        {/* What you did comes first: it is the thing the focus was for. */}
        <p className="fsum__did">
          {t('workspace.focus.minutes', { count: minutesIn(ended, Date.now() / 1000) })}
          {mine.length > 0
            ? ` · ${t(did.failed > 0 ? 'workspace.focus.finishedFailed' : 'workspace.focus.finished', { ok: did.ok, failed: did.failed })}`
            : ` · ${t('workspace.focus.nothingRan')}`}
          {spent ? ` · ${spent}` : ''}
        </p>

        <div className="fsum__top">
          <span className="fsum__t">
            {waiting.length === 0
              ? t('workspace.focus.noneArrived')
              : t(more ? 'workspace.focus.arrivedMore' : 'workspace.focus.arrived', { count: waiting.length })}
          </span>
          <button className="conv__act" onClick={onClose}>
            {t('workspace.focus.done')}
          </button>
        </div>

        {[...byProject.entries()].map(([projectId, group]) => (
          <div className="fsum__grp" key={projectId || 'none'}>
            <span className="fsum__p">{projectId || t('workspace.focus.noProject')}</span>
            {group.map((one) => (
              <button
                className="fsum__one"
                key={one.id}
                data-kind={one.kind}
                onClick={() => {
                  if (one.cardId) onOpenCard(one.cardId)
                }}
              >
                <span className="fsum__line">{one.title}</span>
                {one.detail && <span className="fsum__d">{one.detail}</span>}
              </button>
            ))}
          </div>
        ))}
      </div>
    </div>
  )
}
