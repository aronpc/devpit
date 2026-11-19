import { useEffect, useRef, useState } from 'react'

import { t } from '../i18n'
import { VOICE_SECONDS } from './talk'

/*
 * Dictation. A recording the machine makes into words when it has a voice
 * engine set up; the phone's own speech recognition when it has not; and no
 * mic at all when neither is there.
 */

interface Recognized {
  readonly resultIndex: number
  readonly results: ArrayLike<ArrayLike<{ transcript: string }> & { isFinal: boolean }>
}

interface Recognition {
  lang: string
  continuous: boolean
  interimResults: boolean
  onresult: ((event: Recognized) => void) | null
  onend: (() => void) | null
  onerror: (() => void) | null
  start: () => void
  stop: () => void
}

function recognizer(): (new () => Recognition) | null {
  const found = window as unknown as { SpeechRecognition?: new () => Recognition; webkitSpeechRecognition?: new () => Recognition }
  return found.SpeechRecognition ?? found.webkitSpeechRecognition ?? null
}

const recordable = (): boolean => typeof MediaRecorder !== 'undefined' && typeof navigator.mediaDevices?.getUserMedia === 'function'

/** Who makes a recording into words: the machine, the phone, or nobody. */
function hearer(machine: boolean): 'machine' | 'phone' | null {
  if (machine && recordable()) return 'machine'
  return recognizer() ? 'phone' : null
}

/* What this phone records in that the machine reads: Opus where it can,
   AAC on Safari. */
const TYPES = ['audio/webm;codecs=opus', 'audio/mp4', 'audio/ogg;codecs=opus', 'audio/webm']
const recordedAs = (): string | undefined => TYPES.find((type) => MediaRecorder.isTypeSupported?.(type))

export function RemoteMic({
  machine,
  onRecorded,
  onHeard,
  onRefused,
}: {
  /** The machine has a voice engine set up. */
  machine: boolean
  onRecorded: (voice: Blob) => void
  onHeard: (text: string) => void
  onRefused: (why: string) => void
}): React.JSX.Element | null {
  const by = hearer(machine)
  const [on, setOn] = useState(false)
  const stop = useRef<() => void>(() => {})
  const gone = useRef(false)
  // Left mid-recording, the recording is dropped rather than sent.
  useEffect(
    () => () => {
      gone.current = true
      stop.current()
    },
    [],
  )
  if (!by) return null

  const listen = (): void => {
    const Recognizer = recognizer()
    if (!Recognizer) return
    const recognition = new Recognizer()
    recognition.lang = navigator.language
    recognition.continuous = true
    recognition.interimResults = false
    recognition.onresult = (event) => {
      for (let at = event.resultIndex; at < event.results.length; at++) {
        const result = event.results[at]!
        if (result.isFinal && result[0]) onHeard(result[0].transcript.trim())
      }
    }
    recognition.onend = () => setOn(false)
    recognition.onerror = () => setOn(false)
    stop.current = () => recognition.stop()
    recognition.start()
    setOn(true)
  }

  const record = async (): Promise<void> => {
    let stream: MediaStream
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    } catch {
      return onRefused(t('remote.mic.refused'))
    }
    const type = recordedAs()
    const recorder = new MediaRecorder(stream, type ? { mimeType: type } : undefined)
    const chunks: Blob[] = []
    recorder.ondataavailable = (event) => {
      if (event.data.size > 0) chunks.push(event.data)
    }
    const limit = window.setTimeout(() => stop.current(), VOICE_SECONDS * 1000)
    recorder.onstop = () => {
      window.clearTimeout(limit)
      stream.getTracks().forEach((track) => track.stop())
      setOn(false)
      const voice = new Blob(chunks, { type: recorder.mimeType || chunks[0]?.type || 'audio/webm' })
      if (!gone.current && voice.size > 0) onRecorded(voice)
    }
    stop.current = () => {
      if (recorder.state !== 'inactive') recorder.stop()
    }
    recorder.start()
    setOn(true)
  }

  return (
    <button
      className="rm__btn rm__mic"
      data-on={on || undefined}
      aria-label={on ? t('remote.mic.stopDictating') : t('remote.mic.dictate')}
      title={by === 'machine' ? t('remote.mic.byMachine') : t('remote.mic.byPhone')}
      onClick={() => (on ? stop.current() : by === 'machine' ? void record() : listen())}
    >
      {on ? t('remote.mic.stop') : t('remote.mic.mic')}
    </button>
  )
}
