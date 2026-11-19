import { useCallback, useEffect, useState } from 'react'

import type { Skill } from '../gen/bindings'
import { ask, commands } from './live'
import { InstallationPicker } from './InstallationPicker'
import { SkillDoc } from './SkillDoc'
import { useInstallations } from './useInstallations'
import { t } from '../i18n'

/*
 * The skills this machine has.
 *
 * The eight fixed rows that used to live here named skills nobody had
 * installed. A panel that lists what is not there is worse than an empty one,
 * because an empty one is obviously empty.
 *
 * The directory is on screen for the same reason: the CLI's configuration is
 * not always `~/.claude`, and a panel listing another installation's skills is
 * indistinguishable from one listing this installation's.
 */

const ICON = (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M12 3v3M5.6 5.6l2.1 2.1M3 12h3M18 12h3M16.3 7.7l2.1-2.1" /><rect x="7" y="12" width="10" height="9" rx="2" /></svg>
)

export function SkillsPane(): React.JSX.Element {
  const installations = useInstallations()
  const [skills, setSkills] = useState<readonly Skill[]>([])
  const [directory, setDirectory] = useState('')
  const [problem, setProblem] = useState<string | null>(null)
  const [chosen, setChosen] = useState<string | null>(null)
  const [find, setFind] = useState('')
  const [loading, setLoading] = useState(true)

  /* Read on every look, not once at startup: a skill arrives by installing a
     plugin, which happens in a terminal beside this window. */
  const load = useCallback(() => {
    setLoading(true)
    void ask(() => commands.skillsList(installations.chosen)).then((answer) => {
      setSkills(answer.data?.skills ?? [])
      setDirectory(answer.data?.directory ?? '')
      setProblem(answer.error ?? answer.data?.problem ?? null)
      setLoading(false)
    })
  }, [installations.chosen])

  useEffect(load, [load])

  const wanted = find.trim().toLowerCase()
  const shown = wanted
    ? skills.filter(
        (skill) =>
          skill.name.toLowerCase().includes(wanted) ||
          skill.description.toLowerCase().includes(wanted),
      )
    : skills
  const open = shown.find((skill) => skill.name === chosen) ?? shown[0]

  return (
    <div className="sk">
      <div className="sk__list">
        <div className="sk__seek">
          <input
            className="sk__find"
            placeholder={t('workspace.skills.searchPlaceholder')}
            value={find}
            onChange={(event) => setFind(event.target.value)}
            aria-label={t('workspace.skills.search')}
          />
          <button
            className="sq26"
            onClick={() => {
              installations.reload()
              load()
            }}
            aria-label={t('workspace.skills.refresh')}
            title={t('workspace.skills.refresh')}
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M21 12a9 9 0 0 1-15.5 6.2M3 12a9 9 0 0 1 15.5-6.2M3 20v-5h5M21 4v5h-5" /></svg>
          </button>
        </div>
        <div className="sk__inst">
          <InstallationPicker installations={installations} />
        </div>
        <div className="sk__h">
          {t('workspace.skills.installed')} <span>{wanted ? t('workspace.skills.of', { shown: shown.length, all: skills.length }) : skills.length}</span>
        </div>
        <div className="sk__rows">
          {shown.map((skill) => (
            <button
              className="skrow"
              key={`${skill.source}/${skill.name}`}
              aria-current={open?.name === skill.name}
              onClick={() => setChosen(skill.name)}
            >
              <span className="skrow__ico">{ICON}</span>
              <span className="skrow__b">
                <span className="skrow__n">{skill.name}</span>
                <span className="skrow__d">{skill.description}</span>
              </span>
            </button>
          ))}
          {!loading && shown.length === 0 && (
            <div className="exempty">
              <span className="exempty__t">
                {wanted ? t('workspace.skills.noMatch') : (problem ?? t('workspace.skills.none'))}
              </span>
              <span className="exempty__d">
                {wanted ? t('workspace.skills.searched') : t('workspace.skills.arrive')}
              </span>
            </div>
          )}
        </div>
        {/* Where they were read from. Not decoration: two installations of the
            same CLI hold different sets, and nothing else on screen says which
            one this is. */}
        <div className="sk__foot" title={directory}>
          {/* Isolated for the same reason as the Files panel's path: the box
              runs right-to-left to clip the start, not the end. */}
          <bdi>{problem && shown.length > 0 ? problem : directory}</bdi>
        </div>
      </div>

      <div className="sk__doc">
        {open ? (
          <SkillDoc skill={open} directory={installations.chosen} />
        ) : (
          /* Not the problem again: the list beside this is already saying
             why it is empty, and one sentence twice reads as two faults. */
          <p className="sk__what">{loading ? t('workspace.skills.reading') : t('workspace.skills.nonePicked')}</p>
        )}
      </div>
    </div>
  )
}
