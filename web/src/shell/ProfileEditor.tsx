import { useState } from 'react'

import type { KnownAgent } from '../gen/bindings'
import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import { AccountFields } from './AccountFields'
import type { Draft } from './profiles'
import {
  ACCOUNT_VARS,
  argsOf,
  argsText,
  hasAccountFields,
  masked,
  others,
  ready,
  secret,
  withVar,
} from './profiles'
import { ask, commands } from './live'
import { committed } from './typing'

/*
 * One profile, being written.
 *
 * Four fields and not a command box, because a command box would be a lie in
 * two directions: the board spawns a process and cannot run shell text, and a
 * person's `glm` is not a command at all — it is a `claude` with seven
 * variables in front of it.
 *
 * The program is optional and shows the base agent's own as its placeholder,
 * so the common case is a name and some variables.
 */

export function ProfileEditor({
  draft,
  agents,
  onChange,
  onSave,
  onCancel,
  onRemove,
}: {
  draft: Draft
  agents: readonly KnownAgent[]
  onChange: (next: Draft) => void
  onSave: () => void
  onCancel: () => void
  /** Absent for an agent devpit merely noticed: there is nothing of somebody's
   *  to remove, only an override to clear by emptying the fields. */
  onRemove?: () => void
}): React.JSX.Element {
  /* By name and not by row. Indexed by position, removing a row above a
     revealed one slid the reveal onto whatever took its place — a different
     token on screen that nobody asked to see. */
  useLocale()
  const [shown, setShown] = useState<string | null>(null)
  const base = agents.find((one) => one.id === draft.base)
  /* What the person types to start it, and what reading it found. Two fields
     is the whole form for most people: a name, and the command they already
     use in their terminal. The rest is under Details for anyone who wants it. */
  const [typed, setTyped] = useState(draft.command)
  const [read, setRead] = useState<{ kind: 'reading' | 'read' | 'plain' | 'failed'; say: string } | null>(null)
  const [details, setDetails] = useState(draft.env.length > 0 || draft.args.length > 0)

  const set = (over: Partial<Draft>): void => onChange({ ...draft, ...over })

  /* A command of their own is run once in their shell and read back: the
     agent it starts, its arguments, and the variables it sets. An agent's own
     program needs no reading; anything else unreadable is kept as a program. */
  const readCommand = (): void => {
    const command = typed.trim()
    if (!command || command === draft.command) return
    const own = agents.find((one) => one.launch === command)
    if (own) {
      set({ base: own.id, command: '' })
      return setRead({ kind: 'plain', say: t('profile.runsAsIs', { agent: own.label }) })
    }
    setRead({ kind: 'reading', say: t('profile.reading', { command }) })
    void ask(() => commands.agentProfileRead(command)).then((answer) => {
      if (answer.error) {
        set({ command })
        setDetails(true)
        return setRead({ kind: 'failed', say: answer.error })
      }
      const found = answer.data
      if (!found) {
        set({ command })
        return setRead({ kind: 'plain', say: t('profile.plainProgram', { command }) })
      }
      const agent = agents.find((one) => one.id === found.base)
      set({ base: found.base, command: '', args: found.args, env: found.env })
      const names = found.env.map((one) => one.name)
      setRead({
        kind: 'read',
        say: t('profile.read', {
          agent: agent?.label ?? found.base,
          args: found.args.length ? ` ${found.args.join(' ')}` : '',
          vars: names.length ? t('profile.readWith', { names: names.join(', ') }) : '',
        }),
      })
    })
  }

  return (
    <div className="oapp oapp--new">
      <label className="fld">
        <span className="fld__l">{t('profile.name')}</span>
        <input
          className="fld__b"
          autoFocus
          value={draft.label}
          placeholder={t('profile.placeholder.name')}
          aria-label={t('profile.name')}
          onChange={(event) => set({ label: event.target.value })}
          onKeyDown={(event) => committed(event) && ready(draft) && onSave()}
        />
      </label>

      <label className="fld">
        <span className="fld__l">{t('profile.command')}</span>
        <input
          className="fld__b"
          value={typed}
          spellCheck={false}
          aria-label={t('profile.command')}
          placeholder={t('profile.placeholder.command')}
          onChange={(event) => {
            setTyped(event.target.value)
            setRead(null)
          }}
          onBlur={readCommand}
          onKeyDown={(event) => committed(event) && readCommand()}
        />
      </label>
      <p className="pref__d" data-read={read?.kind}>
        {read?.say ?? t('profile.commandSaid')}
      </p>

      <button className="pdet" aria-expanded={details} onClick={() => setDetails(!details)}>
        {details ? '▾' : '▸'} {t('profile.details')}
      </button>

      {details && (
      <>
      <label className="fld">
        <span className="fld__l">{t('profile.agent')}</span>
        <select
          className="fld__b"
          value={draft.base}
          aria-label={t('profile.agent')}
          onChange={(event) => set({ base: event.target.value })}
        >
          {agents.map((one) => (
            <option key={one.id} value={one.id}>{one.label}</option>
          ))}
        </select>
      </label>

      <label className="fld">
        <span className="fld__l">{t('profile.program')}</span>
        <input
          className="fld__b"
          value={draft.command}
          spellCheck={false}
          aria-label={t('profile.program')}
          placeholder={base?.launch ?? ''}
          onChange={(event) => set({ command: event.target.value })}
        />
      </label>

      <label className="fld">
        <span className="fld__l">{t('profile.arguments')}</span>
        <input
          className="fld__b"
          value={argsText(draft.args)}
          spellCheck={false}
          aria-label={t('profile.arguments')}
          placeholder={t('profile.placeholder.args')}
          onChange={(event) => set({ args: argsOf(event.target.value) })}
        />
      </label>

      <AccountFields key={draft.id} draft={draft} set={set} />

      {/* The account fields' variables are theirs; listing them here too
          would be two places to edit one value. */}
      <span className="fld__l">{hasAccountFields(draft.base) ? t('profile.otherVariables') : t('profile.environment')}</span>
      {others(draft.env, hasAccountFields(draft.base) ? ACCOUNT_VARS : []).map(({ one, at }) => (
        <div className="oapp" key={at}>
          <input
            className="fld__b"
            value={one.name}
            spellCheck={false}
            aria-label={t('profile.variable')}
            placeholder={t('profile.placeholder.variable')}
            onChange={(event) =>
              set({ env: withVar(draft.env, at, { ...one, name: event.target.value }) })
            }
          />
          {/* Hidden by default only where the name says it is a secret: a
              masked base URL is a field nobody can check at a glance. */}
          {secret(one.name) && shown !== one.name ? (
            <button
              className="fld__b"
              aria-label={t('profile.show', { name: one.name })}
              onClick={() => setShown(one.name)}
            >
              {masked(one.value)}
            </button>
          ) : (
            <input
              className="fld__b"
              value={one.value}
              spellCheck={false}
              aria-label={one.name || t('profile.value')}
              onChange={(event) =>
                set({ env: withVar(draft.env, at, { ...one, value: event.target.value }) })
              }
            />
          )}
          <button
            className="btn"
            data-danger
            aria-label={t('profile.removeNamed', { name: one.name || t('profile.variableLower') })}
            onClick={() => set({ env: withVar(draft.env, at, null) })}
          >
            {t('profile.remove')}
          </button>
        </div>
      ))}
      <button
        className="btn"
        onClick={() => set({ env: [...draft.env, { name: '', value: '' }] })}
      >
        {t('profile.addVariable')}
      </button>
      </>
      )}

      <div className="ask__row">
        {onRemove && (
          <button className="btn btn--danger" onClick={onRemove}>
            {t('profile.remove')}
          </button>
        )}
        <button className="btn" onClick={onCancel}>{t('profile.cancel')}</button>
        <button className="btn btn--go" disabled={!ready(draft) || read?.kind === 'reading'} onClick={onSave}>
          {t('profile.save')}
        </button>
      </div>
      <p className="pref__d">
        {base?.launch ? t('profile.footnote', { launch: base.launch }) : t('profile.footnoteAgent')}
      </p>
    </div>
  )
}
