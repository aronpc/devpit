import { useEffect, useState } from 'react'

import type { Profile } from '../gen/bindings'
import { ask, commands } from './live'
import { installationOf } from './outside'
import { added, removed } from './pills'
import { useInstallations } from './useInstallations'
import { t } from '../i18n'

/*
 * The skills picked for this turn, as pills beside the attachments, and the
 * picker that adds them.
 *
 * The picker lists the skills of the installation the chat's profile runs
 * against — `claude2` has its own — the same catalogue the Skills panel
 * shows for it. A skill that is not installed there is not offered.
 */

export function SkillPills({
  picked,
  onChange,
  profile,
}: {
  picked: readonly string[]
  onChange: (next: readonly string[]) => void
  profile: Profile | undefined
}): React.JSX.Element {
  const [open, setOpen] = useState(false)
  /* Keyed by the directory they were read from, so switching account never
     shows the last one's list for a moment. */
  const [installed, setInstalled] = useState<{ from: string | null; names: readonly string[] } | null>(null)
  const { list } = useInstallations()
  const directory = installationOf(profile, list)?.directory ?? null

  useEffect(() => {
    if (!open || installed?.from === directory) return
    void ask(() => commands.skillsList(directory)).then((answer) =>
      setInstalled({ from: directory, names: (answer.data?.skills ?? []).map((skill) => skill.name) }),
    )
  }, [open, directory, installed?.from])

  return (
    <div className="pills">
      {picked.map((skill) => (
        <button key={skill} className="chip pills__p" onClick={() => onChange(removed(picked, skill))} title={t('workspace.skillPills.remove')}>
          /{skill} ✕
        </button>
      ))}
      <button className="chip" aria-expanded={open} onClick={() => setOpen((was) => !was)}>
        {t('workspace.skillPills.add')}
      </button>
      {open && (
        <div className="pills__menu" role="listbox" aria-label={t('workspace.skillPills.label')}>
          {(installed?.from === directory ? installed.names : []).map((skill) => (
            <button
              key={skill}
              className="pills__o"
              role="option"
              aria-selected={picked.includes(skill)}
              onClick={() => {
                onChange(added(picked, skill))
                setOpen(false)
              }}
            >
              {skill}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
