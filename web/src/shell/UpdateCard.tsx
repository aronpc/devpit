import { useEffect, useState } from 'react'

import type { InstallKind, UpdateStatus, UpdateWork } from '../gen/bindings'
import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import { ask, commands } from './live'
import { useFocusIsOn } from './useHeadsDown'
import { onCarried } from './window'
import { Icon } from './Icon'

/*
 * The offer, over the window and never in front of it.
 *
 * An update is not urgent enough to take the keyboard: this is a card in the
 * corner with a close on it, not a dialog. One sentence for what is ready, one
 * for what it costs you — nothing, because the terminals are tmux sessions and
 * they do not go down with the window — and one button.
 *
 * What it never does is decide. The download happens on a click, the restart
 * happens on a click, and the states it draws are the app's, arriving on
 * `update:status`. Restarting asks what is running first: interrupting two
 * runs and a turn is a question about those three things.
 */

/** "2 runs and 1 turn", for the sentence a waiting update says. */
function inFlight(runs: number, turns: number): string {
  const said = [runs > 0 ? t('update.runs', { count: runs }) : null, turns > 0 ? t('update.turns', { count: turns }) : null].filter(
    (one): one is string => one !== null,
  )
  if (said.length === 2) return t('update.both', { runs: said[0]!, turns: said[1]! })
  return said[0] ?? t('update.workInProgress')
}

/** Where the files are for a build devpit does not install over. */
export const RELEASE_PAGE = 'https://github.com/jholhewres/devpit/releases/latest'

/** "for 3 min", for how long an update has been waiting. */
export function waitingFor(since: number | null, now: number): string {
  const minutes = Math.floor(Math.max(0, now - (since ?? now)) / 60)
  if (minutes < 1) return t('update.forLess')
  if (minutes < 60) return t('update.forMinutes', { minutes })
  return t('update.forHours', { hours: Math.floor(minutes / 60), minutes: minutes % 60 })
}

/** Whether the card has a clock to keep.
 *
 * Only a wait says how long it has been waiting, and only a wait is told
 * nothing while it waits: no event arrives between the choice and the work
 * ending, so without a tick of its own the card said "for less than a minute"
 * for as long as it was open. */
/** Asked for by Settings, which sits over the card: whatever was closed on
 *  the card is shown again, because the person went looking for it. */
export const SHOW_THE_UPDATE = 'devpit:show-the-update'

export const counting = (status: UpdateStatus | null): boolean => status?.type === 'waiting'

/** How often that clock moves. Minutes are what the card says, so a finer tick
 *  would re-render for a sentence that did not change. */
const A_TICK = 30_000

/** What an update does to the terminals, for the install it goes into.
 *
 * Kept everywhere but Windows, where the installer ends the psmux inside the
 * install to replace it (`update::terminals_survive`). */
export function terminalsAfter(kind: InstallKind | null): string {
  return t(kind === 'nsis' ? 'update.terminalsClose' : 'update.terminalsKeep')
}

/** What the card says about each state, and what its one button does. */
function offer(
  status: UpdateStatus,
  now: number,
  kind: InstallKind | null,
): {
  title: string
  said: string
  calm: string | null
  action: string | null
} | null {
  switch (status.type) {
    case 'available':
      return {
        title: t('update.available.title'),
        said: t('update.available.said', { version: status.version }),
        calm: terminalsAfter(status.kind),
        action: t('update.available.action'),
      }
    case 'downloading':
      return { title: t('update.downloading.title'), said: '', calm: null, action: null }
    case 'ready':
      return {
        title: t('update.ready.title'),
        said: t('update.ready.said', { version: status.version }),
        calm: terminalsAfter(kind),
        action: t('update.ready.action'),
      }
    case 'waiting':
      return {
        title: t('update.waiting.title'),
        said: t('update.waiting.said', { work: inFlight(status.runs, status.turns), for: waitingFor(status.since, now) }),
        calm: t('update.waiting.calm', { terminals: terminalsAfter(kind) }),
        action: t('update.waiting.action'),
      }
    case 'manualInstall':
      return {
        title: t('update.manual.title'),
        said: status.path,
        /* devpit still does not ask for root. `pkexec` hands the job to
           polkit, which puts up the system's own dialog and takes the
           password itself. */
        calm: t('update.manual.calm'),
        action: t('update.manual.action'),
      }
    case 'failed':
      return {
        title: t('update.failed.title'),
        said: status.message,
        calm: null,
        /* Recoverable means nothing was committed, so asking again is all it
           takes. Without a button here the only way back was Settings. */
        action: status.recoverable ? t('update.failed.action') : null,
      }
    default:
      return null
  }
}

export function UpdateCard(): React.JSX.Element | null {
  useLocale()
  const [status, setStatus] = useState<UpdateStatus | null>(null)
  const [later, setLater] = useState(false)
  const [copied, setCopied] = useState<string | null>(null)
  const [copyRefused, setCopyRefused] = useState(false)
  const [notes, setNotes] = useState(false)
  /* Kept apart from the status: a refused download turns the card into a
     failure, and the failure still came from a test feed. */
  const [testFeed, setTestFeed] = useState(false)
  /* Only `available` names the install, and the states after it still have to
     say what the update does to the terminals. */
  const [kind, setKind] = useState<InstallKind | null>(null)
  /* What is running, once the person has asked to restart and it turns out
     something is. Null is "nothing in the way, or nobody has asked yet". */
  const [work, setWork] = useState<UpdateWork | null>(null)
  /* The moment the card is drawing against. State rather than a read in the
     render, so the tick below is what moves it. */
  const [now, setNow] = useState(() => Date.now() / 1000)
  /* The only overlay that appears on its own, so the only one a focus has to
     hold back. It is not dismissed — it waits, and arrives when the focus
     ends. There is no distinction for a security update today, so every
     update waits. */
  const focused = useFocusIsOn()

  useEffect(() => {
    const hear = (heard: UpdateStatus): void => {
      setStatus(heard)
      if (heard.type === 'available') {
        setKind(heard.kind)
        setTestFeed(heard.testFeed)
      } else if (heard.type !== 'failed') setTestFeed(false)
      // A new state is news again: the close dismissed the state it was on.
      setLater(false)
      setNotes(false)
    }
    /* The check at startup can finish before this listens, so what it found
       is asked for once — and an event heard meanwhile is newer and wins. */
    let heard = false
    const stop = onCarried<UpdateStatus>('update:status', (said) => {
      heard = true
      hear(said)
    })
    const asked = commands.updateStatus()
    void asked.then((now) => !heard && now.type !== 'idle' && hear(now)).catch(() => undefined)
    return stop
  }, [])

  useEffect(() => {
    const show = (): void => setLater(false)
    window.addEventListener(SHOW_THE_UPDATE, show)
    return () => window.removeEventListener(SHOW_THE_UPDATE, show)
  }, [])

  useEffect(() => {
    if (!counting(status)) return
    setNow(Date.now() / 1000)
    const tick = window.setInterval(() => setNow(Date.now() / 1000), A_TICK)
    return () => window.clearInterval(tick)
  }, [status])

  const failed = (error: string): void =>
    setStatus({ type: 'failed', message: error, recoverable: true })

  const install = (): void => {
    void ask(() => commands.updateInstall()).then((answer) => {
      if (answer.error) failed(answer.error)
    })
  }

  const restart = (): void => {
    void ask(() => commands.updateRunning()).then((answer) => {
      const busy = answer.data
      if (answer.error) return failed(answer.error)
      if (busy && busy.runs.length + busy.turns.length > 0) setWork(busy)
      else install()
    })
  }

  const choose = (choice: 'whenItIsDone' | 'stopIt' | 'later'): void => {
    void ask(() => commands.updateChoose(choice)).then((answer) => {
      setWork(null)
      if (answer.error) return failed(answer.error)
      // "Stop it" is carried out by the app: it stops the work, waits for
      // it to go, and installs — the window only hears the states.
      if (answer.data) setStatus(answer.data)
    })
  }

  const copyCommand = (): void => {
    // Asked again rather than copied from the card: the file has been sitting
    // in a cache since it arrived.
    void ask(() => commands.updatePackage()).then((answer) => {
      const command = answer.data
      if (!command) return answer.error && failed(answer.error)
      setCopied(command)
      // Through a promise so a missing clipboard is a refusal too, not a throw.
      Promise.resolve()
        .then(() => navigator.clipboard.writeText(command))
        .catch(() => setCopyRefused(true))
    })
  }

  /* A refusal before the install commits leaves the offer good; the app just
     has to be asked again. */
  const checkAgain = (): void => {
    void ask(() => commands.updateCheck()).then((answer) => {
      if (answer.error) failed(answer.error)
    })
  }

  const download = (): void => {
    void ask(() => commands.updateDownload()).then((answer) => {
      if (answer.error) failed(answer.error)
    })
  }

  if (!status || later || focused) return null
  const said = offer(status, now, kind)
  if (!said) return null

  const fromATestFeed = testFeed && (status.type === 'available' || status.type === 'failed')
  /* A build nobody installs over gets the files, not a button that refuses. */
  const unmanaged = status.type === 'available' && status.kind === 'unmanaged' && !status.testFeed
  const release = status.type === 'available' ? status.notes : ''
  const manual = status.type === 'manualInstall' ? status : null
  /* Closing a downloaded or waiting update is a decision, so it is told to
     the app — hiding it only here left the app holding the update back. */
  const close = (): void => {
    setLater(true)
    if (status.type === 'ready' || status.type === 'waiting') choose('later')
  }
  /* Through polkit. A refusal or a cancel is an answer, not a failure: the
     offer stays good, and a machine without pkexec falls back to the command
     the person runs themselves — which is what this card used to be. */
  const installPackage = (): void => {
    void ask(() => commands.updateInstallPackage()).then((answer) => {
      /* Installed: the app restarts into it, or waits for the work in the way
         and then does, and either arrives here as the status it moves to.
         This used to check again — from the old version, still running, which
         found the feed newer than itself and offered the update it had just
         installed. The password had been typed and nothing seemed to happen. */
      if (answer.data === true) return
      if (answer.data === false) return
      copyCommand()
    })
  }

  const act =
    status.type === 'available'
      ? download
      : status.type === 'ready'
        ? restart
        : status.type === 'failed'
          ? checkAgain
          : installPackage

  return (
    <div className="upd" role="status" aria-label={t('update.label')}>
      <div className="upd__hd">
        <span className="upd__t">
          {said.title}
          {fromATestFeed && <span className="upd__tag">{t('update.testFeed')}</span>}
        </span>
        <button className="upd__x" aria-label={t('update.close')} title={t('update.close')} onClick={close}>
          <Icon size="s">
            <path d="M6 6l12 12M18 6 6 18" />
          </Icon>
        </button>
      </div>

      {said.said && <span className="upd__said">{said.said}</span>}
      {said.calm && <span className="upd__d">{said.calm}</span>}

      {status.type === 'downloading' && (
        <div className="upd__bar">
          <div className="upd__fill" style={{ width: `${status.percent}%` }} />
        </div>
      )}

      {manual && (
        <>
          <code className="upd__cmd">{copied ?? manual.command}</code>
          {copyRefused && <span className="upd__d">{t('update.clipboardRefused')}</span>}
          <span className="upd__d">{t('update.signed')}</span>
        </>
      )}

      {release && (
        <button className="upd__notes" onClick={() => setNotes((was) => !was)}>
          {t('update.notes')}
        </button>
      )}
      {notes && release && <span className="upd__d">{release}</span>}

      {work && (
        <span className="upd__d">
          {t('update.stillGoing', { titles: [...work.runs, ...work.turns].map((one) => one.title).join(', ') })}
        </span>
      )}
      {work && work.keeps.length > 0 && (
        <span className="upd__d">{t('update.keeps', { titles: work.keeps.map((one) => one.title).join(', ') })}</span>
      )}

      {work ? (
        <div className="upd__row">
          <button className="btn" onClick={() => choose('later')}>
            {t('update.notNow')}
          </button>
          <button className="btn" onClick={() => choose('whenItIsDone')}>
            {t('update.whenDone')}
          </button>
          <button className="btn btn--go" onClick={() => choose('stopIt')}>
            {t('update.stopRestart')}
          </button>
        </div>
      ) : status.type === 'waiting' ? (
        <div className="upd__row">
          <button className="btn" onClick={() => choose('later')}>
            {t('update.cancel')}
          </button>
          <button className="btn btn--go" onClick={() => choose('stopIt')}>
            {t('update.stopUpdate')}
          </button>
        </div>
      ) : unmanaged ? (
        <a
          className="upd__go"
          href={RELEASE_PAGE}
          /* The window cannot open a new one of its own: nothing in tauri
             answers wry's new-window request, so target="_blank" did nothing
             at all. The address stays in the href, for hovering and copying. */
          onClick={(event) => {
            event.preventDefault()
            void ask(() => commands.urlOpen(RELEASE_PAGE))
          }}
        >
          {t('update.releasePage')}
        </a>
      ) : (
        said.action && (
          <button className="upd__go" onClick={act}>
            {said.action}
          </button>
        )
      )}
    </div>
  )
}
