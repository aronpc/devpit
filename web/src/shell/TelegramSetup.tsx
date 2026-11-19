import { useEffect, useState } from 'react'

import type { Privacy, TelegramStatus } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'

const PRIVACY: [Privacy, string][] = [
  ['minimal', 'workspace.telegram.privacyMinimal'],
  ['titles', 'workspace.telegram.privacyTitles'],
  ['full', 'workspace.telegram.privacyFull'],
]

/* devpit's bot carries names at most: a whole answer is the person's own bot's. */
const THROUGH_DEVPIT = PRIVACY.filter(([level]) => level !== 'full')

/*
 * The person's own Telegram bot: its token from @BotFather, then a code sent
 * to it from the chat devpit should use. Sent from this machine, with no
 * server in between — and so only while it is on. What the person writes
 * there goes to one orchestrator, the one chosen here.
 *
 * Or devpit's own bot, on devpit.app, offered only when devpit.app says it
 * has one: a single-use code opens the chat with it.
 */
export function TelegramSetup({ onLinked }: { onLinked: () => void }): React.JSX.Element {
  const [status, setStatus] = useState<TelegramStatus | null>(null)
  const [token, setToken] = useState('')
  const [said, setSaid] = useState<string | null>(null)

  useEffect(() => {
    void ask(() => commands.telegramStatus()).then((answer) => setStatus(answer.data))
  }, [])

  /* While a code waits, look again until the chat is linked. */
  useEffect(() => {
    if (!status?.code || status.linked) return
    const timer = window.setInterval(() => {
      void ask(() => commands.telegramStatus()).then((answer) => {
        setStatus(answer.data)
        if (answer.data?.linked) onLinked()
      })
    }, 2000)
    return () => window.clearInterval(timer)
  }, [status?.code, status?.linked, onLinked])

  const link = (): void => {
    setSaid(null)
    void ask(() => commands.telegramLink(token)).then((answer) => {
      if (answer.error) return setSaid(answer.error)
      setToken('')
      setStatus(answer.data)
    })
  }

  const useDevpit = (): void => {
    setSaid(null)
    void ask(() => commands.telegramUseDevpit()).then((answer) => {
      if (answer.error) return setSaid(answer.error)
      setStatus(answer.data)
      if (answer.data?.link) void ask(() => commands.urlOpen(answer.data?.link ?? ''))
    })
  }

  const saved = (answer: { data: TelegramStatus | null; error: string | null }): void => {
    if (answer.data) setStatus(answer.data)
    setSaid(answer.error)
  }

  const chosen = status?.orchestrators.some((one) => one.project === status.orchestrator) ?? false

  if (status?.linked)
    return (
      <div className="pref">
        <span className="pref__body">
          <span className="pref__t">
            {t('workspace.telegram.linkedTitle', { bot: status.bot ?? '' })}
            {status.throughDevpit && ` — ${t('workspace.telegram.devpitBot')}`}
          </span>
          {status.throughDevpit ? (
            <span className="pref__d">
              {t('workspace.telegram.throughDevpit')}
            </span>
          ) : (
            <span className="pref__d">
              {t('workspace.telegram.direct')}
            </span>
          )}
          <label className="voice__row">
            {t('workspace.telegram.says')}
            <select aria-label={t('workspace.telegram.saysLabel')} value={status.privacy} onChange={(event) => void ask(() => commands.telegramPrivacySet(event.target.value as Privacy)).then(saved)}>
              {(status.throughDevpit ? THROUGH_DEVPIT : PRIVACY).map(([level, label]) => (
                <option key={level} value={level}>
                  {t(label)}
                </option>
              ))}
            </select>
          </label>
          {status.orchestrators.length === 1 && <span className="pref__d">{t('workspace.telegram.talksToOnly', { name: status.orchestrators[0].name })}</span>}
          {status.orchestrators.length > 1 && (
            <label className="voice__row">
              {t('workspace.telegram.talksTo')}
              <select
                aria-label={t('workspace.telegram.talksToLabel')}
                value={chosen ? (status.orchestrator ?? '') : ''}
                onChange={(event) => void ask(() => commands.telegramOrchestratorSet(event.target.value || null)).then(saved)}
              >
                {!chosen && <option value="">{t('workspace.telegram.choose')}</option>}
                {status.orchestrators.map((one) => (
                  <option key={one.project} value={one.project}>
                    {one.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          {said && <span className="pref__d voice__bad">{said}</span>}
        </span>
        <button className="btn" onClick={() => void ask(() => commands.telegramUnlink()).then((answer) => (setStatus(answer.data), onLinked()))}>
          {t('workspace.telegram.unlink')}
        </button>
      </div>
    )

  return (
    <div className="pref">
      <span className="pref__body">
        <span className="pref__t">{t('workspace.telegram.name')}</span>
        <span className="pref__d">{t('workspace.telegram.makeBot')}</span>
        {status?.code ? (
          <span className="pref__d">
            {tRich('workspace.telegram.sendCode', { cmd: <code>/start {status.code}</code> /* // ui-text: ok a command */ }, { bot: status.bot ?? '' })}
            {status.link && (
              <>
                {' '}
                &mdash;{' '}
                <button className="plusl" onClick={() => void ask(() => commands.urlOpen(status.link ?? ''))}>
                  {t('workspace.telegram.openIt')}
                </button>
              </>
            )}
            {t('workspace.telegram.codeHolds')}
          </span>
        ) : (
          <>
            <span className="voice__row">
              <input type="password" autoComplete="off" placeholder="123456:ABC…" /* // ui-text: ok the shape of a token */ value={token} onChange={(event) => setToken(event.target.value)} />
              <button className="btn" disabled={!token.trim()} onClick={link}>
                {t('workspace.telegram.link')}
              </button>
            </span>
            {status?.devpitBot && (
              <span className="voice__row">
                <button className="btn" onClick={useDevpit}>
                  {t('workspace.telegram.useDevpit')}
                </button>
                <span className="pref__d">{t('workspace.telegram.devpitHint')}</span>
              </span>
            )}
          </>
        )}
        {said && <span className="pref__d voice__bad">{said}</span>}
      </span>
    </div>
  )
}
