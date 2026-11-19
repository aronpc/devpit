import { useEffect, useState } from 'react'

import type { IslandSession } from '../gen/bindings'
import { t } from '../i18n'
import { ask, commands } from '../shell/live'
import { abandoned, committed } from '../shell/typing'

/*
 * A few words to a session, from the island: a follow-up, a "go on", what it
 * asked for in its own words. Typed into its terminal as the person — the
 * same way the Sessions panel replies — and never into a shell.
 *
 * The island takes the keyboard only while this field has it, and gives it
 * back the moment it does not: a window above everything that took the
 * keyboard on a hover would swallow what the person was typing elsewhere.
 */
export function Reply({ session }: { session: IslandSession }): React.JSX.Element | null {
  const [text, setText] = useState('')
  const [said, setSaid] = useState<string | null>(null)
  const [sending, setSending] = useState(false)
  /* What the orchestrator drafted for it, waiting on the person's click. */
  const [draft, setDraft] = useState<string | null>(null)
  useEffect(() => {
    void ask(() => commands.islandDraft(session.sessionId)).then((answer) => setDraft(answer.data))
  }, [session.sessionId, session.at])
  if (!session.paneId) return null

  const sendDraft = (): void => {
    setSending(true)
    void ask(() => commands.islandDraftSend(session.sessionId)).then((answer) => {
      setSending(false)
      setSaid(answer.error ?? t('island.reply.draftSent'))
      if (!answer.error) setDraft(null)
    })
  }

  const send = (): void => {
    const words = text.trim()
    if (!words || sending) return
    setSending(true)
    void ask(() => commands.islandReply(session.sessionId, words)).then((answer) => {
      setSending(false)
      setSaid(answer.error ?? t('island.reply.sent'))
      if (!answer.error) setText('')
    })
  }

  return (
    <>
      {draft && (
        <div className="isl-draft">
          <span className="isl-draft__t">{t('island.reply.drafted', { draft })}</span>
          <button className="isl-draft__go" disabled={sending} onClick={sendDraft}>
            {t('island.reply.send')}
          </button>
        </div>
      )}
      <form className="isl-reply" onSubmit={(event) => (event.preventDefault(), send())}>
        <input
          className="isl-reply__in"
          value={text}
          maxLength={4000}
          placeholder={t('island.reply.placeholder')}
          aria-label={t('island.reply.label')}
          disabled={sending}
          onChange={(event) => setText(event.target.value)}
          onFocus={() => void ask(() => commands.islandTyping(true))}
          onBlur={() => void ask(() => commands.islandTyping(false))}
          onKeyDown={(event) => {
            if (committed(event)) {
              event.preventDefault()
              send()
            } else if (abandoned(event)) {
              event.currentTarget.blur()
            }
          }}
        />
      </form>
      {said && <p className="isl-reply__said">{said}</p>}
    </>
  )
}
