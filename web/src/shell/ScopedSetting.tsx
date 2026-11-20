import { useEffect, useState } from 'react'

import type { Project, ProjectSettings, Scoped } from '../gen/bindings'
import { modeName, modes } from './chat'
import { t } from '../i18n'
import { ask, commands } from './live'
import { PrefRow } from './PrefRow'

/*
 * A setting a project can choose for itself: "Applying settings for" picks
 * every project (this machine's value) or one of them. A project's own value
 * wins over its repository's .devpit/settings.json, which wins over the
 * machine's; with none set, as devpit always did.
 */

type Field = 'model' | 'permissionMode' | 'branchPattern'

const TITLE: Record<Field, string> = {
  model: 'workspace.scoped.model',
  permissionMode: 'workspace.scoped.permission',
  branchPattern: 'workspace.scoped.branch',
}

const NONE: Record<Field, string> = {
  model: 'workspace.scoped.noneModel',
  permissionMode: '',
  branchPattern: 'workspace.scoped.noneBranch',
}

/** What a scope gets for `field` without a value of its own, and from where. */
function inherited(shown: ProjectSettings, field: Field, project: boolean): string {
  const repo = project ? shown.repo[field] : null
  if (repo) return t('workspace.scoped.fromRepo', { value: named(field, repo) })
  const machine = project ? shown.machine[field] : null
  if (machine) return t('workspace.scoped.fromAll', { value: named(field, machine) })
  return field === 'permissionMode' ? modes()[0].label : t(NONE[field])
}

function named(field: Field, value: string): string {
  return field === 'permissionMode' ? modeName(value) : value
}

export function ScopedSetting({ field }: { field: Field }): React.JSX.Element {
  const [projects, setProjects] = useState<readonly Project[]>([])
  const [scope, setScope] = useState('')
  const [shown, setShown] = useState<ProjectSettings | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    void ask(() => commands.projectList(null)).then((answer) => setProjects((answer.data?.projects ?? []).filter((one) => !one.orchestrator)))
  }, [])
  useEffect(() => {
    void ask(() => commands.projectSettingsRead(scope || null)).then((answer) => {
      setError(answer.error)
      setShown(answer.data ?? null)
    })
  }, [scope])

  const project = scope !== ''
  const mine: Scoped = shown ? (project ? shown.own : shown.machine) : {}
  const value = mine[field] ?? ''
  /* One setting at a time: another page's change to another one stays. */
  const save = (next: string): void => {
    void ask(() => commands.projectSettingsWrite(scope || null, field, next.trim() || null)).then((answer) => {
      setError(answer.error)
      if (answer.data) setShown(answer.data)
    })
  }
  // The machine's branch name is the one Settings → Worktrees always had.
  const editable = Boolean(shown) && (project || field !== 'branchPattern')
  const fallback = shown ? inherited(shown, field, project) : ''
  return (
    <PrefRow
      title={t(TITLE[field])}
      said={
        <>
          {t(project ? 'workspace.scoped.projectOnly' : 'workspace.scoped.everyProject', { fallback })}
          {shown?.repoProblem && ` ${t('workspace.scoped.leftOut', { problem: shown.repoProblem })}`}
          {error && ` ${error}`}
        </>
      }
    >
      <select aria-label={t('workspace.scoped.applying', { setting: t(TITLE[field]) })} value={scope} onChange={(event) => setScope(event.target.value)}>
        <option value="">{t('workspace.scoped.all')}</option>
        {projects.map((one) => (
          <option key={one.id} value={one.id}>
            {one.name}
          </option>
        ))}
      </select>
      {field === 'permissionMode' ? (
        <select aria-label={t(TITLE[field])} value={value} disabled={!editable} onChange={(event) => save(event.target.value)}>
          <option value="">{t('workspace.scoped.as', { fallback })}</option>
          {modes().map((mode) => (
            <option key={mode.id} value={mode.id}>
              {mode.label}
            </option>
          ))}
        </select>
      ) : (
        editable && ( // ui-text: ok a condition the walk misreads as words
          <input key={`${scope}:${value}`} className="voice__in" aria-label={t(TITLE[field])} placeholder={fallback} defaultValue={value} onBlur={(event) => event.target.value.trim() !== value && save(event.target.value)} />
        )
      )}
    </PrefRow>
  )
}
