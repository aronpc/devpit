import { useState } from 'react'

import { commandLine, SESSION_COMMANDS } from './sessionCommands'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'

/*
 * A command for a session's terminal — one of Claude Code's own, or written —
 * typed there as the person once they confirm it. Nothing goes on a pick alone.
 */
export function SessionCommand({ name, onSend }: { name: string; onSend: (line: string) => void }): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const [written, setWritten] = useState('')
  const [picked, setPicked] = useState<string | null>(null)

  const close = (): void => {
    setOpen(false)
    setWritten('')
    setPicked(null)
  }

  if (!open)
    return (
      <button className="sess__btn" onClick={() => setOpen(true)} title={t('sessions.command.title', { name })}>
        {t('sessions.command.send')}
      </button>
    )

  if (picked)
    return (
      <>
        <span className="sess__warn">
          {tRich('sessions.command.confirm', { command: <code>{picked}</code> }, { name })}
        </span>
        <button className="sess__btn" onClick={() => setPicked(null)}>{t('sessions.command.back')}</button>
        <button className="sess__btn" onClick={() => (onSend(picked), close())}>{t('sessions.command.sendNow')}</button>
      </>
    )

  return (
    <div className="sess__cmds" role="group" aria-label={t('sessions.command.group', { name })}>
      {SESSION_COMMANDS.map((one) => (
        <button key={one.command} className="sess__btn" title={t(one.what)} onClick={() => setPicked(one.command)}>
          {one.command}
        </button>
      ))}
      <form className="sess__reply" onSubmit={(event) => (event.preventDefault(), setPicked(commandLine(written)))}>
        <input value={written} maxLength={4000} placeholder={t('sessions.command.write')} aria-label={t('sessions.command.for', { name })} onChange={(event) => setWritten(event.target.value)} />
      </form>
      <button className="sess__btn" onClick={close}>{t('sessions.command.cancel')}</button>
    </div>
  )
}
