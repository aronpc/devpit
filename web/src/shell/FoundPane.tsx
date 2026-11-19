import { useEffect, useState } from 'react'

import type { Found } from '../gen/bindings'
import { ask, commands } from './live'
import { leftNoReview, placeOf, severityWord, standingWords, worstFirst } from './found'
import { t } from '../i18n'

/*
 * What a review found, under the run that found it.
 *
 * Asked separately from the rest of the panel because it carries the payload:
 * a list of runs is not a place to send everything every review said.
 *
 * A review made against another revision says so and keeps its own line
 * numbers. Following them through a diff would be a guess dressed as a fact,
 * and a finding pointing confidently at the wrong line is worse than one that
 * admits its age.
 */

export function FoundPane({ runId }: { runId: string }): React.JSX.Element | null {
  const [found, setFound] = useState<Found | null>(null)

  useEffect(() => {
    let dropped = false
    void ask(() => commands.checkpointFindings(runId)).then((answer) => {
      if (!dropped) setFound(answer.data)
    })
    return () => {
      dropped = true
    }
  }, [runId])

  if (!found || leftNoReview(found)) return null

  const caveat = standingWords(found.standing, found)
  return (
    <div className="fnd">
      {found.findings.length === 0 ? (
        <p className="chk__no">{t('workspace.found.nothing')}</p>
      ) : (
        <ul className="fnd__l">
          {worstFirst(found).map((finding, at) => (
            <li className="fnd__i" key={`${finding.file}:${finding.line}:${at}`}>
              <span className="fnd__s" data-severity={finding.severity}>
                {severityWord(finding.severity)}
              </span>
              <code className="fnd__w">{placeOf(finding.file, finding.line)}</code>
              <span className="fnd__y">{finding.why}</span>
            </li>
          ))}
        </ul>
      )}
      {caveat && (
        <p className="chk__no" data-standing={found.standing}>
          {caveat}
        </p>
      )}
      {found.rubric && (
        <p className="chk__no">
          {/* Kept as it stood: a rubric edited afterwards would leave every
              past review looking like it answered the new one. */}
          {t('workspace.found.asked', { rubric: found.rubric })}
        </p>
      )}
    </div>
  )
}
