import { useEffect, useState } from 'react'

import type { Credentials } from '../gen/bindings'
import { ask, commands } from './live'
import type { Draft } from './profiles'
import { ACCOUNT, hasAccountFields, valueOf, withValue } from './profiles'
import { modelsOf, modelsText } from './models'
import { t } from '../i18n'

/*
 * What makes a profile another account, another endpoint, or another set of
 * models — the three things `claude2` and `glm` actually differ in.
 *
 * Each account field is a variable underneath, written into the same list the
 * plain editor shows; they are here because a person declaring `claude2`
 * should not need to know the name `CLAUDE_CONFIG_DIR`.
 */

/* What the directory says about signing in, asked for what is typed rather
   than what was saved. Unknown says nothing: on macOS the Keychain holds it. */
const SAID: Record<Credentials, string | null> = {
  saved: 'workspace.accountFields.saved',
  missing: 'workspace.accountFields.missing',
  expired: 'workspace.accountFields.expired',
  unknown: null,
}

function useCredentials(dir: string, asking: boolean): Credentials {
  const [state, setState] = useState<Credentials>('unknown')
  useEffect(() => {
    if (!asking) return
    let current = true
    void ask(() => commands.agentSignedIn(dir)).then((answer) => {
      if (current) setState(answer.data?.state ?? 'unknown')
    })
    return () => {
      current = false
    }
  }, [dir, asking])
  return state
}

export function AccountFields({
  draft,
  set,
}: {
  draft: Draft
  set: (over: Partial<Draft>) => void
}): React.JSX.Element {
  /* Typed text, parsed on the way out: parsing on every keystroke ate the
     comma that was about to separate two names. */
  const [models, setModels] = useState(modelsText(draft.models))
  const [reveal, setReveal] = useState(false)
  const value = (name: string): string => valueOf(draft.env, name)
  const put = (name: string, next: string): void => set({ env: withValue(draft.env, name, next) })
  const credentials = useCredentials(value(ACCOUNT.config), hasAccountFields(draft.base))

  return (
    <>
      {hasAccountFields(draft.base) && (
        <>
          <label className="fld">
            <span className="fld__l">{t('workspace.accountFields.config')}</span>
            <input
              className="fld__b"
              value={value(ACCOUNT.config)}
              spellCheck={false}
              aria-label={t('workspace.accountFields.config')}
              placeholder="~/.claude-work" // ui-text: ok an example path
              onChange={(event) => put(ACCOUNT.config, event.target.value)}
            />
            <span className="fld__h">
              {t('workspace.accountFields.configHint')}
              {SAID[credentials] && ` ${t(SAID[credentials])}`}
            </span>
          </label>

          <div className="fld__two">
            <label className="fld">
              <span className="fld__l">{t('workspace.accountFields.endpoint')}</span>
              <input
                className="fld__b"
                value={value(ACCOUNT.url)}
                spellCheck={false}
                aria-label={t('workspace.accountFields.endpoint')}
                placeholder="https://api.anthropic.com" // ui-text: ok an example address
                onChange={(event) => put(ACCOUNT.url, event.target.value)}
              />
            </label>
            <label className="fld">
              <span className="fld__l">{t('workspace.accountFields.token')}</span>
              <span className="fld__b fld__secret">
                <input
                  type={reveal ? 'text' : 'password'}
                  value={value(ACCOUNT.token)}
                  spellCheck={false}
                  autoComplete="off"
                  aria-label={t('workspace.accountFields.token')}
                  placeholder={t('workspace.accountFields.tokenPlaceholder')}
                  onChange={(event) => put(ACCOUNT.token, event.target.value)}
                />
                {value(ACCOUNT.token) && (
                  <button
                    type="button"
                    className="fld__eye"
                    aria-label={reveal ? t('workspace.accountFields.hideToken') : t('workspace.accountFields.showToken')}
                    onClick={() => setReveal((was) => !was)}
                  >
                    {reveal ? t('workspace.accountFields.hide') : t('workspace.accountFields.show')}
                  </button>
                )}
              </span>
            </label>
          </div>
        </>
      )}

      <label className="fld">
        <span className="fld__l">{t('workspace.accountFields.models')}</span>
        <input
          className="fld__b"
          value={models}
          spellCheck={false}
          aria-label={t('workspace.accountFields.models')}
          placeholder="glm-5.3[1m], glm-4.7" // ui-text: ok example model names
          onChange={(event) => {
            setModels(event.target.value)
            set({ models: modelsOf(event.target.value) })
          }}
        />
        <span className="fld__h">
          {t('workspace.accountFields.modelsHint')}
        </span>
      </label>
    </>
  )
}
