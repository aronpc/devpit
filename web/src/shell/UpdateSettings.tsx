import { useEffect, useRef, useState } from 'react'

import type { UpdateStatus } from '../gen/bindings'
import { t } from '../i18n'
import { useLocale } from '../i18n/LocaleRoot'
import { ask, commands } from './live'
import { PrefRow } from './PrefRow'
import { SHOW_THE_UPDATE } from './UpdateCard'
import { useShell } from './useShell'
import { useUpdateStatus } from './useUpdateStatus'

/*
 * Whether there is a newer devpit, and which one this is.
 *
 * Its own file rather than another block inside `Settings.tsx`: that file sits
 * close to its ceiling, and this grows again when the card arrives.
 *
 * The version is asked of the app rather than read from a bundled constant:
 * the app is what the updater compares against, so the number on screen has to
 * be the number it uses.
 *
 * It follows the same status the card does. It used to know only what its own
 * button had answered, so an update the hourly check had found read "devpit
 * checks for a newer version", and a download it started showed "Working…"
 * for minutes and then nothing — while the card that was drawing the progress
 * sat under this screen, out of sight.
 */

function said(status: UpdateStatus | null): string {
  if (!status) return t('updates.checks')
  switch (status.type) {
    case 'idle':
      return t('updates.newest')
    case 'checking':
      return t('updates.checking')
    case 'available':
      return t('updates.available', { version: status.version })
    case 'downloading':
      return t('updates.downloading', { percent: status.percent })
    case 'ready':
      return t('updates.ready', { version: status.version })
    case 'waiting':
      return t('updates.waiting')
    case 'installing':
      return t('updates.installing')
    case 'manualInstall':
      return t('updates.manual')
    case 'externallyManaged':
      return t('updates.external')
    case 'failed':
      return status.message
    default:
      return t('updates.checks')
  }
}

/* An offer from a test feed is not an offer: nothing it names can be
   installed, and the screen has to say so rather than look ordinary. */
function fromATestFeed(status: UpdateStatus | null): boolean {
  return status?.type === 'available' && status.testFeed
}

/* Past the offer, the card is where an update is carried on — restart, the
   work in flight, the package command — so the button here takes you to it. */
const ON_THE_CARD = new Set<UpdateStatus['type']>(['downloading', 'ready', 'waiting', 'manualInstall'])

export function UpdateSettings(): React.JSX.Element {
  useLocale()
  const { closePrefs } = useShell()
  const [status, setStatus] = useUpdateStatus()
  const [version, setVersion] = useState('')
  const [asking, setAsking] = useState(false)

  useEffect(() => {
    void ask(() => commands.appInfo()).then((answer) => setVersion(answer.data?.version ?? ''))
  }, [])

  const answered = (answer: { data: UpdateStatus | null; error: string | null }): void => {
    setAsking(false)
    setStatus(
      answer.data ?? {
        type: 'failed',
        message: answer.error ?? t('updates.nothing'),
        recoverable: true,
      },
    )
  }

  const check = (): void => {
    setAsking(true)
    void ask(() => commands.updateCheck()).then(answered)
  }

  const toTheCard = (): void => {
    window.dispatchEvent(new Event(SHOW_THE_UPDATE))
    closePrefs()
  }

  /* The download is where the card comes in: it is the one place that draws
     progress, asks about work in flight, and installs. So once the download
     has started, this steps out of the way; a refusal before it starts stays
     here, where it was asked. */
  const handing = useRef(false)
  useEffect(() => {
    if (!handing.current || status?.type !== 'downloading') return
    handing.current = false
    window.dispatchEvent(new Event(SHOW_THE_UPDATE))
    closePrefs()
  }, [status, closePrefs])

  const update = (): void => {
    handing.current = true
    setAsking(true)
    void ask(() => commands.updateDownload()).then((answer) => {
      handing.current = false
      setAsking(false)
      if (answer.error) answered(answer)
    })
  }

  /* Nothing to offer on a test feed: what it names cannot be installed. */
  const offered = status?.type === 'available' && !fromATestFeed(status)
  const onTheCard = status !== null && ON_THE_CARD.has(status.type)
  const busy = asking || status?.type === 'checking' || status?.type === 'installing'

  return (
    <PrefRow
      title={
        <>
          {t('updates.title')}{version && t('updates.youHave', { version })}
          {fromATestFeed(status) && t('updates.testFeed')}
        </>
      }
      said={said(status)}
    >
      <button className="btn" disabled={busy} onClick={onTheCard ? toTheCard : offered ? update : check}>
        {busy ? t('updates.working') : onTheCard ? t('updates.show') : offered ? t('updates.update') : t('updates.checkNow')}
      </button>
    </PrefRow>
  )
}
