import { useEffect, useState } from 'react'

import type { WouldRun } from '../gen/bindings'
import { ask, commands } from './live'
import { wouldRunWords } from './wouldRun'
import { t } from '../i18n'

/*
 * What a check would do, before anybody runs it.
 *
 * A check is somebody else's command against your machine and your checkout.
 * A button that says only "Run" asks for trust this screen has not earned —
 * the same word can be `pnpm test` and can be a deploy — so the command, the
 * directory, the environment devpit declares and the timeout are said first.
 *
 * Nothing here starts anything. Running is `card.play`, which runs the lane
 * where the card stands and does not move it.
 */

export function WouldRunCard({
  cardId,
  stepId,
}: {
  cardId: string
  stepId: string
}): React.JSX.Element {
  const [would, setWould] = useState<WouldRun | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    let dropped = false
    void ask(() => commands.checkpointPreview(cardId, stepId)).then((answer) => {
      if (dropped) return
      setWould(answer.data)
      setProblem(answer.error)
    })
    return () => {
      dropped = true
    }
  }, [cardId, stepId])

  if (problem) {
    return (
      <p className="chk__no" role="alert">
        {problem}
      </p>
    )
  }
  if (!would) return <p className="chk__no">{t('workspace.wouldRunCard.reading')}</p>

  const said = wouldRunWords(would)
  return (
    <dl className="chk__ran">
      <dt>{t('workspace.wouldRunCard.wouldRun')}</dt>
      <dd>{would.command ? <code>{would.command}</code> : said.noCommand}</dd>
      <dt>{t('workspace.wouldRunCard.in')}</dt>
      <dd>{would.inDirectory ? <code>{would.inDirectory}</code> : said.noDirectory}</dd>
      <dt>{t('workspace.wouldRunCard.stopsAfter')}</dt>
      <dd>{said.timeout}</dd>
      <dt>{t('workspace.wouldRunCard.with')}</dt>
      {/* Names only. What is in them is not something a preview shows. */}
      <dd>{would.declaredEnv.join(', ')}</dd>
      {would.irreversible && (
        <>
          <dt>{t('workspace.wouldRunCard.careful')}</dt>
          <dd>{said.irreversible}</dd>
        </>
      )}
    </dl>
  )
}
