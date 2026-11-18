import { open as pickFile } from '@tauri-apps/plugin-dialog'
import { useEffect, useState } from 'react'

import type { Transcribing } from '../gen/bindings'
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
  { id: 'local', label: 'This machine' },
  { id: 'api', label: 'A service' },
  { id: 'off', label: 'Off' },
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
    if (!format) return setHeard('This window cannot record: GStreamer’s plugins for the microphone are missing.')
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
      if (!blob) return setHeard(`${before}The microphone gave nothing: check it is the one chosen above.`)
      setTesting('hearing')
      const answer = await ask(async () => commands.transcribeTest(blob.type, await base64Of(blob), systemLanguage()))
      setHeard(before + (answer.data?.text ? `Heard: “${answer.data.text}”` : `Recorded, not heard: ${answer.data?.note ?? answer.error ?? 'no answer'}.`))
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
        <span className="pref__t">Voice messages</span>
        <span className="pref__d">Speak to a chat: the words land in its composer, to read before sending.</span>
        <PrefMore>
          The microphone sits beside the paperclip in a chat&rsquo;s composer ({keysLabel('attach')} attaches files); a terminal has neither. Click or hold it to speak, or hold {keysLabel('talk')} while the chat is in front (Settings → Keyboard changes the key). Without an engine the recording goes as a file. Nothing is sent anywhere unless <b>A service</b> is chosen here.
        </PrefMore>
        <span className="voice__row" role="radiogroup" aria-label="Engine">
          {ENGINES.map((one) => (
            <button key={one.id} className="btn" data-on={now.engine === one.id ? 'true' : undefined} role="radio" aria-checked={now.engine === one.id} onClick={() => save({ ...now, engine: one.id })}>
              {one.label}
            </button>
          ))}
        </span>
        {local && now.local && <span className="pref__d">Found {now.local}.</span>}
        {local && !now.local && (
          <>
            <span className="voice__row">
              <span className="pref__d">{looked === 'none' ? 'Still no whisper on this machine.' : 'No whisper found on this machine.'}</span>
              <button className="btn" disabled={looked === 'looking'} onClick={lookAgain}>
                {looked === 'looking' ? 'Looking…' : 'Check again'}
              </button>
            </span>
            <PrefMore label="How to install one">
              <ul className="prefhow">
                <li><b>whisper.cpp</b>: <code>whisper-cli</code> on the PATH, <code>ffmpeg</code> beside it, and a model file such as <code>ggml-base.bin</code> from the whisper.cpp releases, chosen here.</li>
                <li><b>whisper</b>: <code>pip install openai-whisper</code> (or <code>whisper-ctranslate2</code>), which fetches its <code>base</code> model the first time.</li>
              </ul>
            </PrefMore>
          </>
        )}
        {local && needsFile && (
          <span className="voice__row">
            <span className="voice__val" title={now.model}>{now.model || 'No model file chosen'}</span>
            <button
              className="btn"
              onClick={() =>
                void pickFile({ multiple: false, filters: [{ name: 'whisper.cpp model', extensions: ['bin'] }] }).then(
                  (picked) => typeof picked === 'string' && save({ ...now, model: picked }),
                )
              }
            >
              Choose model
            </button>
            {!now.model && (
              <button className="btn" disabled={fetching} onClick={fetchModel} title="ggml-base.bin from whisper.cpp's releases, about 150 MB, into ~/.devpit/models">
                {fetching ? 'Downloading…' : 'Download base model'}
              </button>
            )}
          </span>
        )}
        {api && (
          <>
            <label className="voice__row">
              <span className="voice__l">Address</span>
              <input className="voice__in" placeholder="https://api.openai.com/v1" defaultValue={now.url} onBlur={(event) => event.target.value !== now.url && save({ ...now, url: event.target.value })} />
            </label>
            <label className="voice__row">
              <span className="voice__l">Model</span>
              <input className="voice__in" placeholder="gpt-4o-mini-transcribe" defaultValue={now.model} onBlur={(event) => event.target.value !== now.model && save({ ...now, model: event.target.value })} />
            </label>
            <span className="voice__row">
              <span className="voice__l">Key</span>
              <input className="voice__in" type="password" autoComplete="off" placeholder={now.keySet ? 'Kept — type to replace' : 'Paste the service’s key'} value={key} onChange={(event) => setKey(event.target.value)} />
              <button className="btn" disabled={!key.trim()} onClick={() => keep(key)}>
                Keep
              </button>
              {now.keySet && (
                <button className="btn" onClick={() => keep(null)}>
                  Forget
                </button>
              )}
            </span>
          </>
        )}
        {now.engine !== 'off' && (
          <label className="voice__row">
            <span className="voice__l">Language</span>
            <input className="voice__in voice__in--short" placeholder="system" defaultValue={now.language} onBlur={(event) => event.target.value !== now.language && save({ ...now, language: event.target.value })} />
          </label>
        )}
        <span className="voice__row">
          <span className="voice__l">Microphone</span>
          <select className="voice__in" aria-label="Microphone" value={mic?.id ?? ''} onChange={(event) => pickMic(event.target.value)}>
            <option value="">System default</option>
            {mic && !mics.some((one) => one.id === mic.id) && <option value={mic.id}>{mic.label || 'The one chosen before'} — not connected</option>}
            {mics.map((one, at) => (
              <option key={one.id} value={one.id}>
                {one.label || `Microphone ${at + 1}`}
              </option>
            ))}
          </select>
          {/* WebKitGTK lists none until a microphone has been opened, and other webviews list them unnamed. */}
          {(mics.length === 0 || mics.some((one) => !one.label)) && (
            <button className="btn" onClick={nameMics} title="Opens the microphone for an instant: the system lists and names them only then">
              {mics.length === 0 ? 'Find microphones' : 'Show their names'}
            </button>
          )}
        </span>
        <span className="voice__row">
          <button className="btn" disabled={testing !== 'no'} onClick={() => void test()}>
            {testing === 'listening' ? 'Listening… say something' : testing === 'hearing' ? 'Hearing…' : 'Test the microphone'}
          </button>
          {heard && <span className="pref__d">{heard}</span>}
        </span>
        {said && <span className="pref__d voice__bad">{said}</span>}
      </div>
    </div>
  )
}
