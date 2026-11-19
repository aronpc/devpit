import { useEffect, useState } from 'react'

import type { DevflowProfile, DevflowProfileFrom } from '../gen/bindings'
import { ask, commands } from './live'
import { t } from '../i18n'

/*
 * The account the project's Claude Code reviewers run as: the profile the
 * project names, else the one its orchestrators speak as, else this
 * machine's default. A session on another account still reviews as its own.
 */

const from = (source: DevflowProfileFrom): string => t(`workspace.reviewers.from.${source}`)

export function ReviewersAccount({ projectId }: { projectId: string }): React.JSX.Element | null {
  const [profile, setProfile] = useState<DevflowProfile | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    void ask(() => commands.devflowProfile(projectId)).then((answer) => setProfile(answer.data))
  }, [projectId])

  if (!profile) return null
  const choose = (id: string): void => {
    void ask(() => commands.devflowProfileSet(projectId, id || null)).then((answer) => {
      if (answer.data) setProfile(answer.data)
      setProblem(answer.error)
    })
  }

  return (
    <div className="dfblock">
      <label className="dffield">
        <span className="dffield__l">{t('workspace.reviewers.runAs')}</span>
        <select aria-label={t('workspace.reviewers.accountLabel')} value={profile.named ?? ''} onChange={(event) => choose(event.target.value)}>
          <option value="">{profile.named ? t('workspace.reviewers.notNamed') : (profile.runsAs ?? t('workspace.reviewers.defaultAccount'))}</option>
          {profile.choices.map((one) => (
            <option key={one.id} value={one.id}>
              {one.label}
            </option>
          ))}
        </select>
      </label>
      <span className="dfblock__d">
        {t('workspace.reviewers.runsFrom', { account: profile.runsAs ?? t('workspace.reviewers.defaultAccount'), from: from(profile.from) })}
        {profile.from !== 'project' && ` ${t('workspace.reviewers.otherProfile')}`}
      </span>
      {problem && <span className="dfblock__d">{problem}</span>}
    </div>
  )
}
