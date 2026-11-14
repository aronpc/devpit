import { useEffect, useState } from 'react'

import type { ProfileModels as Kept } from '../gen/bindings'
import { ask, commands } from './live'
import { PrefRow } from './PrefRow'
import { type Draft, SUBAGENT_VAR, withSubagent } from './profiles'

/*
 * One profile's models in Settings → Providers: each offered or switched off,
 * the model and effort its chats start with, and for Claude Code the advisor
 * model devpit starts its sessions with and, for a profile of the person's
 * own, the subagent model its sessions run with.
 */

export function ProfileModels({ profileId, claude = true, draft, onDraft }: { profileId: string; claude?: boolean; draft?: Draft | null; onDraft?: (draft: Draft) => void }): React.JSX.Element {
  const [kept, setKept] = useState<Kept | null>(null)
  const [offered, setOffered] = useState<readonly string[]>([])
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    void ask(() => commands.profileModelsRead(profileId)).then((answer) => setKept(answer.data ?? null))
    void ask(() => commands.agentProfiles()).then((answer) => setOffered(answer.data?.find((one) => one.id === profileId)?.models ?? []))
  }, [profileId])
  const off = kept?.modelsOff ?? []
  // The default stays on: it is what a session runs without a model chosen.
  const all = [...offered, ...off.filter((one) => !offered.includes(one))].filter((one) => one !== 'default')
  const save = (next: Kept): void => {
    void ask(() => commands.profileModelsWrite(profileId, next)).then((answer) => {
      setError(answer.error)
      if (answer.data) setKept(answer.data)
    })
  }
  const flip = (model: string): void => {
    save({ ...kept, modelsOff: off.includes(model) ? off.filter((one) => one !== model) : [...off, model] })
  }
  const subagent = draft?.env.find((one) => one.name === SUBAGENT_VAR)?.value ?? ''
  /* Saved when the field is left, empty being none. */
  const defaultField = (field: 'model' | 'effort', label: string, placeholder: string, said: string): React.JSX.Element => (
    <PrefRow title={label} said={said}>
      <input
        key={kept?.[field] ?? ''}
        className="voice__in"
        aria-label={label}
        placeholder={placeholder}
        defaultValue={kept?.[field] ?? ''}
        disabled={!kept}
        onBlur={(event) => {
          const value = event.target.value.trim()
          if (value !== (kept?.[field] ?? '')) save({ ...kept, [field]: value || null })
        }}
      />
    </PrefRow>
  )
  return (
    <>
      <PrefRow title="Models" said="A model switched off leaves the model picker. Add your own in the profile's models field.">
        <span className="pmodels">
          {all.map((model) => (
            <label key={model} className="pmodels__one">
              <input type="checkbox" checked={!off.includes(model)} disabled={!kept} onChange={() => flip(model)} aria-label={`Offer ${model}`} />
              {model}
            </label>
          ))}
        </span>
      </PrefRow>
      {defaultField('model', 'Default model', 'as the CLI', "What this profile's chats run when none is picked. Empty leaves it to the CLI.")}
      {defaultField('effort', 'Default effort', 'as the CLI', 'The effort its chats start with, where the CLI has one. Empty keeps the CLI\'s own.')}
      {claude && <PrefRow title="Advisor model" said="Written as advisorModel into the settings devpit starts this profile's sessions with, never into the profile's own settings. Empty is none.">
        <input
          key={kept?.advisorModel ?? ''}
          className="voice__in"
          aria-label="Advisor model"
          placeholder="none"
          defaultValue={kept?.advisorModel ?? ''}
          disabled={!kept}
          onBlur={(event) => {
            const model = event.target.value.trim()
            if (model !== (kept?.advisorModel ?? '')) save({ ...kept, advisorModel: model || null })
          }}
        />
      </PrefRow>}
      {claude && draft && onDraft && (
        <PrefRow title="Subagent model" said={`${SUBAGENT_VAR} for this profile's sessions. Saved with the profile.`}>
          <input className="voice__in" aria-label="Subagent model" placeholder="as the session" value={subagent} onChange={(event) => onDraft(withSubagent(draft, event.target.value))} />
        </PrefRow>
      )}
      {error && <p className="pref__d">{error}</p>}
    </>
  )
}
