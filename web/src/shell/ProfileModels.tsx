import { useEffect, useState } from 'react'

import type { ProfileModels as Kept } from '../gen/bindings'
import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
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
  useLocale()
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
      <PrefRow title={t('models.title')} said={t('models.said')}>
        <span className="pmodels">
          {all.map((model) => (
            <label key={model} className="pmodels__one">
              <input type="checkbox" checked={!off.includes(model)} disabled={!kept} onChange={() => flip(model)} aria-label={t('models.offer', { model })} />
              {model}
            </label>
          ))}
        </span>
      </PrefRow>
      {defaultField('model', t('models.defaultModel'), t('models.asCli'), t('models.defaultModelSaid'))}
      {defaultField('effort', t('models.defaultEffort'), t('models.asCli'), t('models.defaultEffortSaid'))}
      {claude && <PrefRow title={t('models.advisor')} said={t('models.advisorSaid')}>
        <input
          key={kept?.advisorModel ?? ''}
          className="voice__in"
          aria-label={t('models.advisor')}
          placeholder={t('models.none')}
          defaultValue={kept?.advisorModel ?? ''}
          disabled={!kept}
          onBlur={(event) => {
            const model = event.target.value.trim()
            if (model !== (kept?.advisorModel ?? '')) save({ ...kept, advisorModel: model || null })
          }}
        />
      </PrefRow>}
      {claude && draft && onDraft && (
        <PrefRow title={t('models.subagent')} said={t('models.subagentSaid', { variable: SUBAGENT_VAR })}>
          <input className="voice__in" aria-label={t('models.subagent')} placeholder={t('models.asSession')} value={subagent} onChange={(event) => onDraft(withSubagent(draft, event.target.value))} />
        </PrefRow>
      )}
      {error && <p className="pref__d">{error}</p>}
    </>
  )
}
