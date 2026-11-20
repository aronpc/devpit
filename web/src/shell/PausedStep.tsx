import { useEffect, useState } from 'react'

import type { Run, RunSwitchOptions } from '../gen/bindings'
import { ask, commands } from './live'

export function RunIdentity({ run }: { run: Run }): React.JSX.Element | null {
  const said = [run.profile, run.model].filter(Boolean).join(' · ')
  return said ? <span className="crun__a">{said}</span> : null
}

export function PausedStep({
  run,
  onSwitched,
}: {
  run: Run
  onSwitched: () => void
}): React.JSX.Element {
  const [options, setOptions] = useState<RunSwitchOptions | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const [switching, setSwitching] = useState(false)

  useEffect(() => {
    void ask(() => commands.runSwitchOptions(run.id)).then((answer) => {
      setProblem(answer.error)
      setOptions(answer.data)
    })
  }, [run.id])

  const switchTo = (profileId: string): void => {
    setSwitching(true)
    void ask(() => commands.runSwitch(run.id, profileId)).then((answer) => {
      setSwitching(false)
      setProblem(answer.error)
      if (answer.data) onSwitched()
    })
  }

  return (
    <div className="pstep">
      {options && <p className="pstep__why">{options.reason}</p>}
      {problem && <p className="wtb__no">{problem}</p>}
      {options?.profiles.length === 0 && (
        <p className="pstep__none">No other profile can run this step.</p>
      )}
      {options && options.profiles.length > 0 && (
        <div className="pstep__profiles">
          {options.profiles.map((profile) => (
            <button
              className="btn btn--sm"
              key={profile.id}
              aria-label={`Switch to ${profile.label}`}
              disabled={switching}
              onClick={() => switchTo(profile.id)}
            >
              Switch to {profile.label}
              {profile.model && ` · ${profile.model}`}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
