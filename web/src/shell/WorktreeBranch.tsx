import { useEffect, useState } from 'react'

import type { WorktreeBranch as Branch } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'
import { committed } from './typing'

/*
 * What a new card's branch is called. Empty is the default, shown as the
 * placeholder; the example under the field is the backend's own rendering,
 * and a refusal keeps what was typed, as the folder field above does.
 */

/* The placeholders a pattern takes, shown as written. */
const token = (name: string): React.JSX.Element => <code>{`{${name}}`}</code>

export function WorktreeBranch(): React.JSX.Element {
  const [branch, setBranch] = useState<Branch | null>(null)
  const [draft, setDraft] = useState('')
  const [problem, setProblem] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    void ask(() => commands.worktreeBranchRead()).then((answer) => {
      if (!answer.data) return
      setBranch(answer.data)
      setDraft(answer.data.typed)
    })
  }, [])

  const save = (typed: string): void => {
    setDraft(typed)
    void ask(() => commands.worktreeBranchWrite(typed)).then((answer) => {
      setProblem(answer.error)
      if (!answer.data) return
      setBranch(answer.data)
      setSaved(true)
    })
  }

  return (
    <section className="wtb">
      <div className="wtb__head">
        <span className="pref__t">{t('workspace.worktreeBranch.title')}</span>
        <span className="pref__d">
          {tRich('workspace.worktreeBranch.said', {
            type: token('type'),
            number: token('number'),
            slug: token('slug'),
            id: token('id'),
          })}
        </span>
      </div>

      <div className="wtb__row">
        <input
          className="wtb__f"
          value={draft}
          spellCheck={false}
          placeholder={branch?.default}
          aria-label={t('workspace.worktreeBranch.title')}
          onChange={(event) => {
            setDraft(event.target.value)
            setSaved(false)
            setProblem(null)
          }}
          onBlur={(event) => event.target.value !== branch?.typed && save(event.target.value)}
          onKeyDown={(event) => committed(event) && save(draft)}
        />
        {branch && branch.typed !== '' && (
          <button className="btn" onClick={() => save('')}>
            {t('workspace.worktreeBranch.reset')}
          </button>
        )}
      </div>

      {problem ? (
        <p className="wtb__no">{problem}</p>
      ) : (
        branch && (
          <p className="wtb__ex">
            {tRich('workspace.worktreeBranch.example', { branch: <code>{branch.example}</code> })}
            {saved && <span className="wtb__ok">{t('workspace.worktreeBranch.saved')}</span>}
          </p>
        )
      )}
    </section>
  )
}
