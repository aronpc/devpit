import { open as pickFile } from '@tauri-apps/plugin-dialog'
import { useEffect, useState } from 'react'

import type { Transcribing } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'
import { PrefMore } from './PrefRow'
import { base64Of, chooseMicrophone, chosenMicrophone, fellBack, formatHere, type Microphone, microphones, namedMicrophones, record, refusal, systemLanguage } from './recorder'
import { keysLabel } from './shortcuts'

/*
 * How voice messages become words: a whisper on this machine, a service the
 * person chose with their own key, or not at all. Nothing is sent anywhere
 * unless the service is chosen here.
 */

/** How long the microphone test records. */
const TEST_MS = 3000

const ENGINES: readonly { id: string; label: string }[] = [
  { id: 'local', label: 'workspace.voice.engineLocal' },
  { id: 'api', label: 'workspace.voice.engineApi' },
  { id: 'off', label: 'workspace.voice.engineOff' },
]

export function VoiceSettings(): React.JSX.Element {
  const [now, setNow] = useState<Transcribing | null>(null)
  const [key, setKey] = useState('')
  const [said, setSaid] = useState<string | null>(null)

  const [looked, setLooked] = useState<'not yet' | 'looking' | 'none'>('not yet')
  const [testing, setTesting] = useState<'no' | 'listening' | 'hearing'>('no')
  const [heard, setHeard] = useState<string | null>(null)
  const [fetching, setFetching] = useState(false)
  const [mics, setMics] = useState<Microphone[]>([])
  const [mic, setMic] = useState(chosenMicrophone)

  /* The microphones, again whenever one is plugged in or pulled out. */
  useEffect(() => {
    const list = (): void => void microphones().then(setMics, () => setMics([]))
    list()
    navigator.mediaDevices?.addEventListener?.('devicechange', list)
    return () => navigator.mediaDevices?.removeEventListener?.('devicechange', list)
  }, [])

  const pickMic = (id: string): void => {
    const picked = mics.find((one) => one.id === id) ?? null
    chooseMicrophone(picked)
    setMic(picked)
  }

  const nameMics = (): void => void namedMicrophones().then(setMics, (error: unknown) => setHeard(refusal(error)))

  /* Three seconds through the same microphone and engine a chat uses, thrown away after. */
  const test = async (): Promise<void> => {
    const format = formatHere()
    if (!format) return setHeard(t('workspace.voice.cannotRecord'))
    setHeard(null)
    try {
      const recording = await record(format)
      setTesting('listening')
      await new Promise((done) => window.setTimeout(done, TEST_MS))
      const blob = await recording.stop()
      /* Opened once, the microphones have names; and the chosen one may have a new id. */
      setMic(chosenMicrophone())
      void microphones().then(setMics, () => undefined)
      const before = recording.missing ? `${fellBack(recording.missing)} ` : ''
      if (!blob) return setHeard(before + t('workspace.voice.gaveNothing'))
      setTesting('hearing')
      const answer = await ask(async () => commands.transcribeTest(blob.type, await base64Of(blob), systemLanguage()))
      setHeard(before + (answer.data?.text ? t('workspace.voice.heard', { text: answer.data.text }) : t('workspace.voice.notHeard', { note: answer.data?.note ?? answer.error ?? t('workspace.voice.noAnswer') })))
    } catch (error) {
      setHeard(refusal(error))
    } finally {
      setTesting('no')
    }
  }

  const fetchModel = (): void => {
    setFetching(true)
    void ask(() => commands.transcribeModelDownload()).then((answer) => {
      setFetching(false)
      setSaid(answer.error)
      if (answer.data) setNow(answer.data)
    })
  }

  useEffect(() => {
    void ask(() => commands.transcribeRead()).then((answer) => setNow(answer.data))
  }, [])

  /* The same read the pane opens with: it looks for a whisper every time, so
     one installed a minute ago is found without leaving Settings. */
  const lookAgain = (): void => {
    setLooked('looking')
    void ask(() => commands.transcribeRead()).then((answer) => {
      setSaid(answer.error)
      if (answer.data) setNow(answer.data)
      setLooked(answer.data?.local ? 'not yet' : 'none')
    })
  }

  const save = (next: Transcribing): void => {
    setNow(next)
    void ask(() => commands.transcribeSet(next.engine, next.model, next.language, next.url)).then((answer) => {
      setSaid(answer.error)
      if (answer.data) setNow(answer.data)
    })
  }

  const keep = (value: string | null): void => {
    void ask(() => commands.transcribeKeySet(value)).then((answer) => {
      setSaid(answer.error)
      if (answer.error || !now) return
      setKey('')
      setNow({ ...now, keySet: value !== null })
    })
  }

  if (!now) return <></>
  const local = now.engine === 'local'
  const api = now.engine === 'api'
  const needsFile = now.local === 'whisper-cli' || now.local === 'whisper-cpp'
  return (
    <div className="pref pref--stack">
      <div className="pref__body">
        <span className="pref__t">{t('workspace.voice.title')}</span>
        <span className="pref__d">{t('workspace.voice.hint')}</span>
        <PrefMore>
          {tRich('workspace.voice.more', { service: <b>{t('workspace.voice.engineApi')}</b> }, { attach: keysLabel('attach'), talk: keysLabel('talk') })}
        </PrefMore>
        <span className="voice__row" role="radiogroup" aria-label={t('workspace.voice.engine')}>
          {ENGINES.map((one) => (
            <button key={one.id} className="btn" data-on={now.engine === one.id ? 'true' : undefined} role="radio" aria-checked={now.engine === one.id} onClick={() => save({ ...now, engine: one.id })}>
              {t(one.label)}
            </button>
          ))}
        </span>
        {local && now.local && <span className="pref__d">{t('workspace.voice.found', { name: now.local })}</span>}
        {local && !now.local && (
          <>
            <span className="voice__row">
              <span className="pref__d">{looked === 'none' ? t('workspace.voice.stillNone') : t('workspace.voice.none')}</span>
              <button className="btn" disabled={looked === 'looking'} onClick={lookAgain}>
                {looked === 'looking' ? t('workspace.voice.looking') : t('workspace.voice.checkAgain')}
              </button>
            </span>
            <PrefMore label={t('workspace.voice.installTitle')}>
              <ul className="prefhow">
                <li>{tRich('workspace.voice.installCpp', { name: <b>whisper.cpp</b>, cli: <code>whisper-cli</code>, ffmpeg: <code>ffmpeg</code>, file: <code>ggml-base.bin</code> })}</li> {/* // ui-text: ok commands and file names are not words */}
                <li>{tRich('workspace.voice.installPip', { name: <b>whisper</b>, pip: <code>pip install openai-whisper</code>, alt: <code>whisper-ctranslate2</code>, base: <code>base</code> })}</li> {/* // ui-text: ok commands and file names are not words */}
              </ul>
            </PrefMore>
          </>
        )}
        {local && needsFile && (
          <span className="voice__row">
            <span className="voice__val" title={now.model}>{now.model || t('workspace.voice.noModel')}</span>
            <button
              className="btn"
              onClick={() =>
                void pickFile({ multiple: false, filters: [{ name: t('workspace.voice.modelFilter'), extensions: ['bin'] }] }).then(
                  (picked) => typeof picked === 'string' && save({ ...now, model: picked }),
                )
              }
            >
              {t('workspace.voice.chooseModel')}
            </button>
            {!now.model && (
              <button className="btn" disabled={fetching} onClick={fetchModel} title={t('workspace.voice.downloadTitle')}>
                {fetching ? t('workspace.voice.downloading') : t('workspace.voice.download')}
              </button>
            )}
          </span>
        )}
        {api && (
          <>
            <label className="voice__row">
              <span className="voice__l">{t('workspace.voice.address')}</span>
              <input className="voice__in" placeholder="https://api.openai.com/v1" /* // ui-text: ok an address */ defaultValue={now.url} onBlur={(event) => event.target.value !== now.url && save({ ...now, url: event.target.value })} />
            </label>
            <label className="voice__row">
              <span className="voice__l">{t('workspace.voice.model')}</span>
              <input className="voice__in" placeholder="gpt-4o-mini-transcribe" /* // ui-text: ok a model id */ defaultValue={now.model} onBlur={(event) => event.target.value !== now.model && save({ ...now, model: event.target.value })} />
            </label>
            <span className="voice__row">
              <span className="voice__l">{t('workspace.voice.key')}</span>
              <input className="voice__in" type="password" autoComplete="off" placeholder={now.keySet ? t('workspace.voice.keyKept') : t('workspace.voice.keyPaste')} value={key} onChange={(event) => setKey(event.target.value)} />
              <button className="btn" disabled={!key.trim()} onClick={() => keep(key)}>
                {t('workspace.voice.keep')}
              </button>
              {now.keySet && (
                <button className="btn" onClick={() => keep(null)}>
                  {t('workspace.voice.forget')}
                </button>
              )}
            </span>
          </>
        )}
        {now.engine !== 'off' && (
          <label className="voice__row">
            <span className="voice__l">{t('workspace.voice.language')}</span>
            <input className="voice__in voice__in--short" placeholder={t('workspace.voice.languageSystem')} defaultValue={now.language} onBlur={(event) => event.target.value !== now.language && save({ ...now, language: event.target.value })} />
          </label>
        )}
        <span className="voice__row">
          <span className="voice__l">{t('workspace.voice.microphone')}</span>
          <select className="voice__in" aria-label={t('workspace.voice.microphone')} value={mic?.id ?? ''} onChange={(event) => pickMic(event.target.value)}>
            <option value="">{t('workspace.voice.systemDefault')}</option>
            {mic && !mics.some((one) => one.id === mic.id) && <option value={mic.id}>{mic.label || t('workspace.voice.chosenBefore')} — {t('workspace.voice.notConnected')}</option>}
            {mics.map((one, at) => (
              <option key={one.id} value={one.id}>
                {one.label || t('workspace.voice.microphoneN', { n: at + 1 })}
              </option>
            ))}
          </select>
          {/* WebKitGTK lists none until a microphone has been opened, and other webviews list them unnamed. */}
          {(mics.length === 0 || mics.some((one) => !one.label)) && (
            <button className="btn" onClick={nameMics} title={t('workspace.voice.findTitle')}>
              {mics.length === 0 ? t('workspace.voice.find') : t('workspace.voice.showNames')}
            </button>
          )}
        </span>
        <span className="voice__row">
          <button className="btn" disabled={testing !== 'no'} onClick={() => void test()}>
            {testing === 'listening' ? t('workspace.voice.listening') : testing === 'hearing' ? t('workspace.voice.hearing') : t('workspace.voice.test')}
          </button>
          {heard && <span className="pref__d">{heard}</span>}
        </span>
        {said && <span className="pref__d voice__bad">{said}</span>}
      </div>
    </div>
  )
}
