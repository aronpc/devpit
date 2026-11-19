import { useEffect, useState } from 'react'

import type { Tested } from '../gen/bindings'
import { ask, commands } from './live'
import { countsWords, leftNoReport, unlistedWords } from './tested'
import { t } from '../i18n'

/* The failed tests of an opened run, asked on their own like the findings. */

export function TestedPane({ runId }: { runId: string }): React.JSX.Element | null {
  const [tested, setTested] = useState<Tested | null>(null)

  useEffect(() => {
    let dropped = false
    void ask(() => commands.checkpointTested(runId)).then((answer) => {
      if (!dropped) setTested(answer.data)
    })
    return () => {
      dropped = true
    }
  }, [runId])

  if (!tested || leftNoReport(tested)) return null

  const unlisted = unlistedWords(tested)
  return (
    <div className="fnd">
      <p className="chk__no">{countsWords(tested)}</p>
      {tested.failures.length > 0 && (
        <ul className="fnd__l">
          {tested.failures.map((failure, at) => (
            <li className="fnd__i" key={`${failure.file}:${failure.name}:${at}`}>
              <span className="fnd__s" data-severity="blocking">
                {t('workspace.tested.failed')}
              </span>
              <code className="fnd__w">
                {failure.name}
                {failure.file && ` — ${failure.file}`}
              </code>
              {failure.message && <span className="fnd__y">{failure.message}</span>}
            </li>
          ))}
        </ul>
      )}
      {unlisted && <p className="chk__no">{unlisted}</p>}
    </div>
  )
}
