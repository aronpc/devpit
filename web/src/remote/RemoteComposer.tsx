import { useEffect, useRef, useState } from 'react'

import type { Attachment, RemoteIn, RemoteOut } from '../gen/bindings'
import { withFiles, withHeard } from '../shell/frames'
import { RemoteMic } from './RemoteMic'
import { photoRefused, pieces, said, shrunk, uploadId, voiceRefused, type Talking } from './talk'
import { t } from '../i18n'

/*
 * What the person writes from the phone: typed, dictated or with a photo.
 * Dictation lands in the field for them to read before it goes; a photo is
 * kept on the machine and the message names it by its path there.
 */

interface Sending {
  readonly upload: string
  readonly what: 'photo' | 'voice'
}

export function RemoteComposer({
  talking,
  typing,
  voice,
  busy,
  send,
  hears,
}: {
  talking: Talking
  typing: boolean
  /** The machine has a voice engine set up. */
  voice: boolean
  /** A turn is on its way, and the next waits for it. */
  busy: boolean
  send: (message: RemoteIn) => void
  /** Where the machine's word on a photo or a recording reaches this. */
  hears: React.MutableRefObject<(heard: RemoteOut) => void>
}): React.JSX.Element {
  const [text, setText] = useState('')
  const [files, setFiles] = useState<readonly Attachment[]>([])
  const [sending, setSending] = useState<Sending | null>(null)
  const [note, setNote] = useState<string | null>(null)
  const picker = useRef<HTMLInputElement>(null)
  /* What the machine's word is matched against, as it arrives. */
  const waited = useRef<Sending | null>(null)
  const wait = (next: Sending | null): void => {
    waited.current = next
    setSending(next)
  }

  useEffect(() => {
    const done = (): void => {
      waited.current = null
      setSending(null)
    }
    hears.current = (heard) => {
      if (heard.t === 'failed') return done()
      if ((heard.t !== 'heard' && heard.t !== 'attached') || heard.upload !== waited.current?.upload) return
      done()
      if (heard.t === 'attached') return setFiles((was) => [...was, heard.attachment])
      const words = heard.transcript.text
      if (words) setText((was) => withHeard(was, words))
      else setNote(heard.transcript.note ?? t('remote.composer.nothingHeard'))
    }
    return () => {
      hears.current = () => {}
    }
  }, [hears])

  if (!typing) return <p className="rm__said">{t('remote.composer.watchOnly')}</p>

  // A photo is kept in the project the session or the chat is in.
  const project = talking.project

  const upload = async (file: Blob, what: Sending['what'], finish: (upload: string) => RemoteIn): Promise<void> => {
    const id = uploadId()
    const bytes = new Uint8Array(await file.arrayBuffer())
    setNote(null)
    wait({ upload: id, what })
    for (const message of pieces(bytes, id, finish(id))) send(message)
  }

  const photo = async (file: File): Promise<void> => {
    const small = await shrunk(file)
    const refused = photoRefused(small)
    if (refused || !project) return setNote(refused ?? t('remote.composer.noProject'))
    await upload(small, 'photo', (id) => ({ t: 'photo', upload: id, project, mime: small.type }))
  }

  const recorded = (recording: Blob): void => {
    const refused = voiceRefused(recording)
    if (refused) return setNote(refused)
    void upload(recording, 'voice', (id) => ({ t: 'voice', upload: id, mime: recording.type }))
  }

  const message = withFiles(text.trim(), files).trim()
  const go = (): void => {
    if (!message) return
    send(said(talking, message))
    setText('')
    setFiles([])
    setNote(null)
  }

  return (
    <div className="rm__compose">
      {sending && (
        <p className="rm__dim" role="status">
          {sending.what === 'photo' ? t('remote.composer.sendingPhoto') : t('remote.composer.listening')}
        </p>
      )}
      {note && (
        <p className="rm__dim" role="status">
          {note}
        </p>
      )}
      {files.length > 0 && (
        <div className="rm__files">
          {files.map((file) => (
            <button key={file.path} className="rm__file" title={t('remote.composer.leaveOut')} onClick={() => setFiles((was) => was.filter((one) => one.path !== file.path))}>
              {file.name} ×
            </button>
          ))}
        </div>
      )}
      <textarea className="rm__write" aria-label={t('remote.composer.message')} placeholder={t('remote.composer.writeTo', { title: talking.title })} rows={3} value={text} onChange={(event) => setText(event.target.value)} />
      <div className="rm__line">
        {project && (
          <>
            <input
              ref={picker}
              type="file"
              accept="image/*"
              hidden
              onChange={(event) => {
                const picked = event.target.files?.[0]
                event.target.value = ''
                if (picked) void photo(picked)
              }}
            />
            <button className="rm__btn" disabled={sending !== null} onClick={() => picker.current?.click()}>
              {t('remote.composer.photo')}
            </button>
          </>
        )}
        <RemoteMic machine={voice} onRecorded={recorded} onHeard={(words) => setText((was) => withHeard(was, words))} onRefused={setNote} />
        <button className="rm__btn rm__btn--go rm__send" disabled={busy || sending !== null || !message} onClick={go}>
          {busy ? t('remote.composer.answering') : t('remote.composer.send')}
        </button>
      </div>
    </div>
  )
}
