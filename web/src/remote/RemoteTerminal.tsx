import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import { useCallback, useEffect, useRef, useState } from 'react'

import type { RemoteIn } from '../gen/bindings'
import { frame, Kind } from '../shell/paneSocket'
import { SESSION_COMMANDS } from '../shell/sessionCommands'
import { base64Of } from './socket'
import { t } from '../i18n'

/*
 * One terminal of the machine, watched. Its size here is its own: the
 * machine attaches this viewer so it changes nothing on the desk. Typing is
 * keys straight through, a key bar for what a phone keyboard lacks, and a
 * line written and sent whole.
 */

// `name` because a glyph is not a name: a screen reader would say "upwards arrow".
const KEYS: readonly { label: string; name: string; bytes: string }[] = [
  { label: 'Esc', name: 'Escape', bytes: '\x1b' }, // ui-text: ok the cap printed on the key
  { label: 'Tab', name: 'Tab', bytes: '\t' }, // ui-text: ok the cap printed on the key
  { label: '^C', name: 'Interrupt (Ctrl+C)', bytes: '\x03' },
  { label: '↑', name: 'Up', bytes: '\x1b[A' },
  { label: '↓', name: 'Down', bytes: '\x1b[B' },
  { label: '←', name: 'Left', bytes: '\x1b[D' },
  { label: '→', name: 'Right', bytes: '\x1b[C' },
  { label: 'Enter', name: 'Enter', bytes: '\r' }, // ui-text: ok the cap printed on the key
]

/* Each terminal opened gets a handle of its own, so bytes still on the way
 * for one closed a moment ago never land in the next. */
let lastHandle = 0
const words = new TextEncoder()

export function RemoteTerminal({
  project,
  pane,
  typing,
  binary,
  send,
  sendBytes,
  bytes,
}: {
  project: string
  pane: string
  typing: boolean
  /** The host reads and sends raw bytes: keys and output skip base64 and JSON. */
  binary: boolean
  send: (message: RemoteIn) => void
  sendBytes: (frame: Uint8Array<ArrayBuffer>) => void
  bytes: React.RefObject<(from: string | number, data: Uint8Array) => void>
}): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null)
  const [line, setLine] = useState('')
  const handle = useRef<number | null>(null)

  const type = useCallback(
    (data: string): void => {
      if (handle.current !== null) sendBytes(frame(handle.current, Kind.Input, words.encode(data)))
      else send({ t: 'paneInput', pane, b64: base64Of(data) })
    },
    [pane, send, sendBytes],
  )

  useEffect(() => {
    if (!host.current) return
    const term = new Terminal({ fontSize: 12, scrollback: 2000, convertEol: false, disableStdin: !typing, cursorBlink: typing })
    const fit = new FitAddon()
    term.loadAddon(fit)
    term.open(host.current)
    fit.fit()
    const mine = binary ? ++lastHandle : null
    handle.current = mine
    bytes.current = (from, data) => (from === pane || from === mine) && term.write(data)
    send({ t: 'paneOpen', project, pane, cols: term.cols, rows: term.rows, handle: mine })
    const typed = typing ? term.onData(type) : null
    return () => {
      typed?.dispose()
      handle.current = null
      send({ t: 'paneClose', pane })
      bytes.current = () => {}
      term.dispose()
    }
  }, [project, pane, typing, binary, send, bytes, type])

  const sendLine = (): void => {
    if (!line.trim()) return
    send({ t: 'panePaste', pane, text: line })
    setLine('')
  }

  return (
    <div className="rm__term">
      <div className="rm__xterm" ref={host} />
      {typing ? (
        <>
          <div className="rm__keys">
            {KEYS.map((key) => (
              <button key={key.label} className="rm__key" aria-label={key.name} title={key.name} onClick={() => type(key.bytes)}>
                {key.label}
              </button>
            ))}
          </div>
          <div className="rm__keys" aria-label={t('remote.terminal.commands')}>
            {SESSION_COMMANDS.map((one) => (
              <button key={one.command} className="rm__key" title={one.what} onClick={() => confirm(t('remote.terminal.confirmType', { command: one.command })) && send({ t: 'panePaste', pane, text: one.command })}>
                {one.command}
              </button>
            ))}
          </div>
          <div className="rm__line">
            <input value={line} placeholder={t('remote.terminal.linePlaceholder')} onChange={(event) => setLine(event.target.value)} onKeyDown={(event) => event.key === 'Enter' && !event.nativeEvent.isComposing && sendLine()} />
            <button className="rm__btn" onClick={sendLine}>
              {t('remote.terminal.send')}
            </button>
          </div>
        </>
      ) : (
        <p className="rm__dim">{t('remote.terminal.watchOnly')}</p>
      )}
    </div>
  )
}
