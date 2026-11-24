import { useCallback, useEffect, useState } from 'react'

import { open as pickFolder } from '@tauri-apps/plugin-dialog'

import type { WorktreeBase as Base } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'
import { useShell } from './useShell'
import { committed } from './typing'

/*
 * Where new worktrees are created.
 *
 * It was a constant and the constant is still the default, which is why the
 * field can be empty and mean something. The two answers it exists for:
 *
 *   a relative path   the checkouts live inside the project and travel with it
 *   an absolute path  one folder holds every project, on whichever disk
 *
 * The resolved path is shown under the field rather than explained above it.
 * "Relative or absolute" is a sentence about paths; a path is an answer.
 *
 * Refusals come from the backend and are printed as they arrive. The field
 * keeps what was typed — a box that clears itself on a refusal makes you type
 * the whole thing again to find out what was wrong with it.
 */

export function WorktreeBase(): React.JSX.Element {
  const { project } = useShell()
  const [base, setBase] = useState<Base | null>(null)
  const [draft, setDraft] = useState('')
  const [problem, setProblem] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  const load = useCallback(() => {
    void ask(() => commands.worktreeBaseRead(project?.id ?? null)).then((answer) => {
      if (!answer.data) return
      setBase(answer.data)
      setDraft(answer.data.typed)
    })
  }, [project])

  useEffect(load, [load])

  const save = (typed: string): void => {
    setDraft(typed)
    void ask(() => commands.worktreeBaseWrite(project?.id ?? null, typed)).then((answer) => {
      setProblem(answer.error)
      if (!answer.data) return
      setBase(answer.data)
      setSaved(true)
    })
  }

  const browse = async (): Promise<void> => {
    const picked = await pickFolder({ directory: true, multiple: false })
    if (typeof picked === 'string') save(picked)
  }

  return (
    <section className="wtb">
      <div className="wtb__head">
        <span className="pref__t">{t('workspace.worktreeBase.title')}</span>
        <span className="pref__d">
          {t('workspace.worktreeBase.said')}
        </span>
      </div>

      <div className="wtb__row">
        <input
          className="wtb__f"
          value={draft}
          spellCheck={false}
          placeholder={t('workspace.worktreeBase.placeholder')}
          aria-label={t('workspace.worktreeBase.title')}
          onChange={(event) => {
            setDraft(event.target.value)
            setSaved(false)
            setProblem(null)
          }}
          onBlur={(event) => event.target.value !== base?.typed && save(event.target.value)}
          onKeyDown={(event) => committed(event) && save(draft)}
        />
        <button className="btn" onClick={() => void browse()}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></svg>
          {t('workspace.worktreeBase.browse')}
        </button>
        {/* Only once the read has come back and said there is something to
            reset. `!base?.isDefault` is true while `base` is still null, so
            the button appeared before anything had been chosen. */}
        {base && !base.isDefault && (
          <button className="btn" onClick={() => save('')}>
            {t('workspace.worktreeBase.reset')}
          </button>
        )}
      </div>

      {problem ? (
        <p className="wtb__no">{problem}</p>
      ) : (
        base && (
          <p className="wtb__ex">
            {tRich('workspace.worktreeBase.next', { folder: <code>{base.example}</code> })}
            {saved && <span className="wtb__ok">{t('workspace.worktreeBase.saved')}</span>}
          </p>
        )
      )}
    </section>
  )
}
