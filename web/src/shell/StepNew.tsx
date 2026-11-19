import { useContext, useEffect, useState } from 'react'

import type { Agent, Profile, Step } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from './live'
import { offers, onProfilesChanged } from './profiles'
import { filled, RECIPES, type Recipe } from './recipes'
import { ShellStoreContext } from './shellStore'
import { CONTEXT_KEYS, stepConfig, stepFields, type Fields } from './stepConfig'

/*
 * The form that makes a step.
 *
 * One field per kind, because the kinds share nothing: a command has a command
 * and a patience, a session has a model and an account to run it under. Until
 * this form asked for them, every step made here was saved as the one line the
 * person typed, which is not JSON, which is not what any runner reads.
 *
 * The same form edits one. The kind is not offered then: a command that became
 * an agent would be a different step, and the runs already filed under this
 * one say what it was when they ran.
 */

/* Each kind's words are `step.kind.<id>` and `step.kindHint.<id>`. */
const KINDS = [{ id: 'agent' }, { id: 'session' }, { id: 'command' }] as const

type Field = {
  /** Its words are `step.field.<key>` and `step.placeholder.<key>`. */
  key: string
  /** A step without it would not run, so Create stays out of reach. */
  needed?: boolean
  /** Prose rather than a line: a prompt, or the schema of an answer. */
  prose?: boolean
}

const FIELDS: Readonly<Record<string, readonly Field[]>> = {
  command: [
    { key: 'command', needed: true },
    { key: 'timeoutSeconds' },
  ],
  session: [{ key: 'model' }],
  agent: [
    { key: 'prompt', needed: true, prose: true },
    // A step with no ceiling is a bill nobody agreed to, so the backend
    // refuses one. Asked for here rather than discovered on the lane.
    { key: 'capUsd', needed: true },
    { key: 'model' },
    { key: 'expects', prose: true },
  ],
}

export function StepNew({
  step,
  onDone,
  onCancel,
}: {
  /** The step being edited, or nothing when one is being made. */
  step?: Step | null
  onDone: (kind: string, name: string, config: string, irreversible: boolean) => void
  onCancel: () => void
}): React.JSX.Element {
  const [kind, setKind] = useState<string>(step?.kind ?? 'command')
  const [name, setName] = useState(step?.name ?? '')
  // A config nobody can read starts the form empty rather than half filled:
  // what is on screen is then what will be saved, all of it.
  const [fields, setFields] = useState<Fields>(
    step ? (stepFields(step.kind, step.config) ?? {}) : {},
  )
  const [profiles, setProfiles] = useState<readonly Profile[]>([])
  const [agents, setAgents] = useState<readonly Agent[]>([])
  const [irreversible, setIrreversible] = useState(step?.irreversible ?? false)
  // A recipe's config the form does not show, kept by the save.
  const [base, setBase] = useState<string | undefined>(undefined)
  const [recipe, setRecipe] = useState<Recipe | null>(null)
  // Read when a recipe is picked; a form drawn outside a shell has no project.
  const shell = useContext(ShellStoreContext)
    const asked = FIELDS[kind] ?? []

  // A command runs as itself; the two kinds that spend somebody's account ask
  // which one.
  useEffect(() => {
    if (kind === 'command') return
    const load = (): void =>
      void ask(() => commands.agentProfiles()).then((answer) => setProfiles(answer.data ?? []))
    load()
    return onProfilesChanged(load)
  }, [kind])

  // Offered rather than typed: a step stores an agent by the name in its
  // frontmatter, and a free-text field over the files on disk is a typo that
  // fails when the card lands on the lane.
  useEffect(() => {
    if (kind !== 'agent') return
    void ask(() => commands.agentsList()).then((answer) => setAgents(answer.data?.agents ?? []))
  }, [kind])

  const start = async (picked: Recipe): Promise<void> => {
    const project = shell?.get().project
    const test = picked.id === 'tests' && project ? (await ask(() => commands.projectTestCommand(project.id))).data : null
    const made = filled(picked.id, test?.command ?? null)
    setRecipe(picked)
    setKind(made.kind)
    setName(made.name)
    setFields(made.fields)
    setBase(made.base)
  }

  const typed = (key: string, value: string): void =>
    setFields((was) => ({ ...was, [key]: value }))
  const missing = asked.some((one) => one.needed && !(fields[one.key] ?? '').trim())
  const injected = (fields.inject ?? '').split(',').map((one) => one.trim())
  const inject = (key: string): void =>
    typed(
      'inject',
      (injected.includes(key) ? injected.filter((one) => one !== key) : [...injected, key])
        .filter(Boolean)
        .join(', '),
    )

  return (
    <div className="lstep__pop lstep__pop--wide">
      <p className="lstep__t">{step ? t('step.this') : t('step.aNew')}</p>

      {!step && (
        <>
          <span className="fld__l">{t('step.startFrom')}</span>
          <div className="src__sw">
            {RECIPES.map((one) => (
              <button className="src__o" key={one.id} aria-checked={recipe?.id === one.id} onClick={() => void start(one)}>
                {t(`step.recipe.${one.id}`)}
              </button>
            ))}
          </div>
          {recipe && <p className="pref__d">{t('step.recipeHint', { hint: t(`step.recipeHint.${recipe.id}`) })}</p>}
        </>
      )}

      {!step && (
        <div className="src__sw">
          {KINDS.map((one) => (
            <button
              className="src__o"
              key={one.id}
              aria-checked={kind === one.id}
              onClick={() => (setKind(one.id), setBase(undefined), setRecipe(null))}
            >
              {t(`step.kind.${one.id}`)}
            </button>
          ))}
        </div>
      )}
      <p className="pref__d">{t(`step.kindHint.${kind}`)}</p>

      <label className="fld">
        <span className="fld__l">{t('step.name')}</span>
        <input
          className="fld__b"
          autoFocus
          value={name}
          placeholder={t('step.namePlaceholder')}
          onChange={(event) => setName(event.target.value)}
        />
      </label>

      {kind === 'agent' && (
        <label className="fld">
          <span className="fld__l">{t('step.agent')}</span>
          <input
            className="fld__b"
            list="stepnew-agents"
            value={fields.agent ?? ''}
            spellCheck={false}
            placeholder={t('step.agentPlaceholder')}
            onChange={(event) => typed('agent', event.target.value)}
          />
          <datalist id="stepnew-agents">
            {agents.map((one) => (
              <option key={one.name} value={one.name} />
            ))}
          </datalist>
        </label>
      )}

      {asked.map((one) => (
        <label className="fld" key={one.key}>
          <span className="fld__l">{t(`step.field.${one.key}`)}</span>
          {one.prose ? (
            <textarea
              className="lstep__ta"
              value={fields[one.key] ?? ''}
              spellCheck={false}
              placeholder={t(`step.placeholder.${one.key}`)}
              onChange={(event) => typed(one.key, event.target.value)}
            />
          ) : (
            <input
              className="fld__b"
              value={fields[one.key] ?? ''}
              spellCheck={false}
              placeholder={t(`step.placeholder.${one.key}`)}
              onChange={(event) => typed(one.key, event.target.value)}
            />
          )}
        </label>
      ))}

      {kind === 'agent' && (
        <label className="fld">
          <span className="fld__l">{t('step.tellIt')}</span>
          {CONTEXT_KEYS.map((key) => (
            <button
              className="ask__opt"
              key={key}
              role="checkbox"
              aria-checked={injected.includes(key)}
              onClick={() => inject(key)}
            >
              <span className="box">
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M20 6 9 17l-5-5" />
                </svg>
              </span>
              <span className="ask__ot">{key}</span>
            </button>
          ))}
        </label>
      )}

      {kind !== 'command' && (
        <label className="fld">
          <span className="fld__l">{t('step.account')}</span>
          <div className="src__sw">
            <button
              className="src__o"
              aria-checked={!fields.profile}
              onClick={() => typed('profile', '')}
            >
              {t('step.default')}
            </button>
            {profiles.filter(offers(fields.profile ?? null)).map((one) => (
              <button
                className="src__o"
                key={one.id}
                aria-checked={fields.profile === one.id}
                onClick={() => typed('profile', one.id)}
              >
                {one.label}
              </button>
            ))}
          </div>
        </label>
      )}

      {/* Stated where it is chosen, not where it bites. A deploy has no undo,
          so it is confirmed rather than fired by dropping a card on a lane. */}
      <button
        className="ask__opt"
        role="checkbox"
        aria-checked={irreversible}
        onClick={() => setIrreversible((was) => !was)}
      >
        <span className="box">
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
            <path d="M20 6 9 17l-5-5" />
          </svg>
        </span>
        <span>
          <span className="ask__ot">{t('step.noUndoTitle')}</span>
          <span className="ask__od">{t('step.noUndoDetail')}</span>
        </span>
      </button>

      <div className="ask__row">
        <button className="btn" onClick={onCancel}>{t('step.cancel')}</button>
        <button
          className="btn btn--go"
          disabled={!name.trim() || missing}
          onClick={() => onDone(kind, name.trim(), stepConfig(kind, fields, step?.config ?? base), irreversible)}
        >
          {step ? t('step.save') : t('step.create')}
        </button>
      </div>
    </div>
  )
}
