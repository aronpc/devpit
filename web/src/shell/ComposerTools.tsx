import { open as pickFiles } from '@tauri-apps/plugin-dialog'
import { useEffect, useRef, useState } from 'react'

import { ask, commands } from './live'
import { base64Of, fellBack, formatHere, LONGEST_MS, record, refusal, systemLanguage, type Recording } from './recorder'
import type { Chat } from './useChat'
import { useShell } from './useShell'
import { letsGo, matches } from './shortcuts'
import { useKeys, useShortcut } from './useShortcut'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * The composer's two ways in besides typing: files, chosen from the system's
 * picker (Ctrl/Cmd+Shift+A), and a voice message — recorded, made into words
 * the person reads and edits before sending, and sent as the recording when
 * no engine could hear it. The microphone is clicked, held, or its key held
 * while the chat is the one in front (push to talk).
 */

const clock = (ms: number): string => {
  const seconds = Math.floor(ms / 1000)
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
}

/** A press held this long is a hold: let go, and the recording stops. */
const HOLD_MS = 600

export function ComposerTools({ chat, mine, onHeard }: { chat: Chat; mine: boolean; onHeard: (text: string) => void }): React.JSX.Element {
  const { project } = useShell()
  const box = useRef<HTMLSpanElement>(null)
  const recording = useRef<Recording | null>(null)
  const pressed = useRef(0)
  /* While the talk key is down: what lets go of it. */
  const held = useRef<(() => void) | null>(null)
  /* The microphone is being opened: a second press waits for it, and a
     composer gone meanwhile closes what it opens. */
  const opening = useRef(false)
  const gone = useRef(false)
  const [since, setSince] = useState<number | null>(null)
  const [now, setNow] = useState(0)
  const [hearing, setHearing] = useState(false)
  const [said, setSaid] = useState<string | null>(null)
  const format = formatHere()
  const keys = useKeys()

  const pick = async (): Promise<void> => {
    const picked = await pickFiles({ multiple: true })
    const paths = Array.isArray(picked) ? picked : typeof picked === 'string' ? [picked] : []
    if (paths.length > 0) chat.attach(paths)
  }

  /* From anywhere in this composer, and only there: every open chat has one. */
  useShortcut('attach', () => {
    if (!box.current?.closest('.composer')?.contains(document.activeElement)) return false
    void pick()
  })

  useEffect(() => {
    if (since === null) return
    const beat = window.setInterval(() => {
      setNow(Date.now())
      if (Date.now() - since > LONGEST_MS) void finish()
    }, 250)
    return () => window.clearInterval(beat)
  })

  /* Push to talk: held while this chat is the one in front, never from a
     terminal, and let go by its key, a key held with it, or the window losing
     focus — a key let go elsewhere never comes back as a keyup. */
  useShortcut('talk', () => {
    if (!mine || document.activeElement?.closest('.xterm')) return false
    if (held.current || recording.current || hearing) return
    if (!format) return setSaid(t('workspace.composerTools.cannotRecord'))
    const up = (event: KeyboardEvent): void => {
      if (letsGo('talk', event)) letGo()
    }
    /* The key repeats while held: none of it is typed into the composer. */
    const repeat = (event: KeyboardEvent): void => {
      if (event.repeat && matches('talk', event)) event.preventDefault()
    }
    const letGo = (): void => {
      window.removeEventListener('keyup', up, true)
      window.removeEventListener('keydown', repeat, true)
      window.removeEventListener('blur', letGo)
      held.current = null
      if (recording.current) void finish()
    }
    window.addEventListener('keyup', up, true)
    window.addEventListener('keydown', repeat, true)
    window.addEventListener('blur', letGo)
    held.current = letGo
    /* Let go before the microphone opened: the recording ends as it starts. */
    void start().then(() => {
      if (!held.current && recording.current) void finish()
    })
  })

  /* Gone with the composer, a recording does not keep the microphone. */
  useEffect(() => {
    gone.current = false
    return () => {
      gone.current = true
      recording.current?.cancel()
      recording.current = null
      held.current?.()
    }
  }, [])

  /* Another tab in front: the composer is hidden, so nothing records unseen.
     What was said so far is made words in this chat, where it was said. */
  useEffect(() => {
    if (mine) return
    if (held.current) held.current()
    else if (recording.current) void finish()
  })

  const start = async (): Promise<void> => {
    if (!format || recording.current || opening.current) return
    opening.current = true
    setSaid(null)
    try {
      const got = await record(format)
      if (gone.current) return got.cancel()
      recording.current = got
      if (got.missing) setSaid(fellBack(got.missing))
      setSince(Date.now())
      setNow(Date.now())
    } catch (error) {
      if (!gone.current) setSaid(refusal(error))
    } finally {
      opening.current = false
    }
  }

  const cancel = (): void => {
    recording.current?.cancel()
    recording.current = null
    setSince(null)
  }

  const finish = async (): Promise<void> => {
    const held = recording.current
    if (!held || !project) return
    recording.current = null
    setSince(null)
    const blob = await held.stop()
    if (!blob) return setSaid(t('workspace.composerTools.nothing'))
    setHearing(true)
    const kept = await ask(async () => commands.chatPaste(project.id, blob.type, await base64Of(blob)))
    if (!kept.data) {
      setHearing(false)
      return setSaid(kept.error ?? t('workspace.composerTools.notKept'))
    }
    const recorded = kept.data
    chat.keep(recorded)
    const heard = await ask(() => commands.chatTranscribe(project.id, recorded.name, systemLanguage()))
    setHearing(false)
    const text = heard.data?.text
    if (text) {
      /* Heard, the words are the message; the recording goes unless kept by hand. */
      chat.detach(recorded.path)
      onHeard(text)
      return
    }
    setSaid(t('workspace.composerTools.sentAsRecording', { why: heard.data?.note ?? heard.error ?? t('workspace.composerTools.notHeard') }))
  }

  const recordingNow = since !== null
  return (
    <span className="ctools" ref={box}>
      {said && (
        <span className="ctools__said" role="status">
          {said}
          <button className="ctools__x" onClick={() => setSaid(null)} aria-label={t('workspace.composerTools.dismiss')} title={t('workspace.composerTools.dismiss')}>
            ✕
          </button>
        </span>
      )}
      {recordingNow ? (
        <span className="ctools__rec" role="status" aria-label={t('workspace.composerTools.recording')}>
          <i className="ctools__dot" />
          {clock(now - since)}
          <button className="ctools__btn" onClick={cancel} title={t('workspace.composerTools.throwAway')}>
            {t('workspace.composerTools.cancel')}
          </button>
          <button className="ctools__btn ctools__btn--go" onClick={() => void finish()} title={t('workspace.composerTools.stopTitle')}>
            {t('workspace.composerTools.done')}
          </button>
        </span>
      ) : (
        <>
          <button className="ctools__icon" onClick={() => void pick()} title={t('workspace.composerTools.attachTitle', { keys: keys('attach') })} aria-label={t('workspace.composerTools.attach')}>
            <Icon size="m"><path d="m21.4 11.6-9.2 9.2a6 6 0 0 1-8.5-8.5l9.2-9.2a4 4 0 0 1 5.7 5.7l-9.2 9.2a2 2 0 0 1-2.8-2.8l8.5-8.5" /></Icon>
          </button>
          <button
            className="ctools__icon"
            disabled={!format || hearing}
            title={!format ? t('workspace.composerTools.cannotRecordTitle') : hearing ? t('workspace.composerTools.making') : t('workspace.composerTools.recordTitle', { keys: keys('talk') })}
            aria-label={t('workspace.composerTools.record')}
            onPointerDown={() => {
              pressed.current = Date.now()
              void start()
            }}
            onPointerUp={() => {
              if (Date.now() - pressed.current > HOLD_MS) void finish()
            }}
          >
            {hearing ? (
              <span className="ctools__wait" />
            ) : (
              <Icon size="m"><rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5 11a7 7 0 0 0 14 0M12 18v3" /></Icon>
            )}
          </button>
        </>
      )}
    </span>
  )
}
