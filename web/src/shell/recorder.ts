/*
 * A voice message, recorded in the window: the microphone through
 * MediaRecorder where this webview writes WebM or Ogg, and otherwise as WAV
 * made here from the microphone's samples.
 *
 * The only other format a webview offers is MP4, and WebKitGTK writes it
 * fragmented (no `isofmp4mux`, so `mp4mux` in fragments): the transcription
 * services refuse that file as unsupported. WAV is taken by every one of
 * them and by a whisper on this machine.
 */

/** The longest a voice message runs before it stops on its own. */
export const LONGEST_MS = 5 * 60_000

const FORMATS = ['audio/webm;codecs=opus', 'audio/webm', 'audio/ogg;codecs=opus']

/** Made here from the samples, when MediaRecorder writes none of the formats above. */
export const WAV = 'audio/wav'

/** The rate a WAV is kept at: what whisper hears at, and a third of the size of 48 kHz. */
export const WAV_RATE = 16_000

/** The format to record in, or null where this webview records none. */
export function formatHere(): string | null {
  if (!navigator.mediaDevices?.getUserMedia) return null
  const written = typeof MediaRecorder === 'undefined' ? undefined : FORMATS.find((one) => MediaRecorder.isTypeSupported(one))
  return written ?? (typeof AudioContext === 'undefined' ? null : WAV)
}

/* Half the width of the low-pass, in samples of the source. */
const REACH = 32

const gcd = (a: number, b: number): number => (b === 0 ? a : gcd(b, a % b))

/*
 * The low-pass for each place a new sample can fall between two old ones: a
 * Blackman-windowed sinc cut at 90 % of the new Nyquist, its weights summing
 * to one. The places repeat every `phases` new samples (one at 48 kHz, 160 at
 * 44.1 kHz), so the sines are worked out once and not for every sample.
 */
function kernels(rate: number, phases: number, ratio: number): Float32Array[] {
  const cut = (0.9 * WAV_RATE) / 2 / rate
  return Array.from({ length: phases }, (_, phase) => {
    const fraction = ((phase * ratio) % phases) / phases
    const weights = new Float32Array(2 * REACH)
    let sum = 0
    for (let k = 0; k < 2 * REACH; k++) {
      const x = k - REACH + 1 - fraction
      const sinc = x === 0 ? 2 * cut : Math.sin(2 * Math.PI * cut * x) / (Math.PI * x)
      const w = (x + REACH) / (2 * REACH)
      weights[k] = sinc * (0.42 - 0.5 * Math.cos(2 * Math.PI * w) + 0.08 * Math.cos(4 * Math.PI * w))
      sum += weights[k]!
    }
    return weights.map((one) => one / sum)
  })
}

/**
 * Samples at `rate`, brought to `WAV_RATE` through a low-pass so nothing
 * above 8 kHz folds back into the speech. Averaging blocks of samples lets a
 * 10 kHz tone through at half its strength as a 6 kHz one, and WebKitGTK's
 * own resampler (OfflineAudioContext) lets it through whole, so it is done
 * here.
 */
export function resampled(pieces: readonly Float32Array[], rate: number): Float32Array {
  const total = pieces.reduce((sum, one) => sum + one.length, 0)
  const all = new Float32Array(total)
  let at = 0
  for (const one of pieces) {
    all.set(one, at)
    at += one.length
  }
  const source = Math.round(rate)
  if (source === WAV_RATE) return all
  const common = gcd(source, WAV_RATE)
  const ratio = source / common
  const phases = WAV_RATE / common
  const filters = kernels(source, phases, ratio)
  const out = new Float32Array(Math.floor((total * phases) / ratio))
  for (let i = 0; i < out.length; i++) {
    const base = Math.floor((i * ratio) / phases)
    const weights = filters[i % phases]!
    const first = base - REACH + 1
    let sum = 0
    for (let k = Math.max(0, -first); k < 2 * REACH && first + k < total; k++) sum += all[first + k]! * weights[k]!
    out[i] = sum
  }
  return out
}

/** Samples already at `WAV_RATE` as a 16-bit mono WAV. */
export function wavOf(samples: Float32Array): Blob {
  const count = samples.length
  const bytes = new DataView(new ArrayBuffer(44 + count * 2))
  const text = (at: number, said: string): void => [...said].forEach((char, i) => bytes.setUint8(at + i, char.charCodeAt(0)))
  text(0, 'RIFF')
  bytes.setUint32(4, 36 + count * 2, true)
  text(8, 'WAVE')
  text(12, 'fmt ')
  bytes.setUint32(16, 16, true)
  bytes.setUint16(20, 1, true) // PCM
  bytes.setUint16(22, 1, true) // mono
  bytes.setUint32(24, WAV_RATE, true)
  bytes.setUint32(28, WAV_RATE * 2, true)
  bytes.setUint16(32, 2, true)
  bytes.setUint16(34, 16, true)
  text(36, 'data')
  bytes.setUint32(40, count * 2, true)
  for (let i = 0; i < count; i++) {
    const one = Math.max(-1, Math.min(1, samples[i]!))
    bytes.setInt16(44 + i * 2, one < 0 ? one * 0x8000 : one * 0x7fff, true)
  }
  return new Blob([bytes.buffer], { type: WAV })
}

/** The language to hear it in when the settings name none: the system's, by its first part. */
export function systemLanguage(): string | null {
  const code = (navigator.language || '').split('-')[0]?.toLowerCase() ?? ''
  return /^[a-z]{2,3}$/.test(code) ? code : null
}

/** Why the microphone could not be had, in words. */
export function refusal(error: unknown): string {
  const name = error instanceof DOMException ? error.name : ''
  if (name === 'NotAllowedError' || name === 'SecurityError')
    return 'The microphone is blocked: allow devpit to use it in the system’s privacy settings, then try again.'
  if (name === 'NotFoundError') return 'No microphone was found.'
  return 'The microphone could not be opened.'
}

/**
 * What the microphone is asked for. WebKitGTK's recorder takes the sample
 * rate the track declares as the rate its encoder must be fed, and a track
 * opened with `audio: true` declares 0 there: nothing can be fed at rate 0,
 * the microphone is left unlinked and every recording comes back empty. A
 * track asked for a rate declares one. Elsewhere it is only a preference.
 */
export const MICROPHONE: MediaTrackConstraints = { sampleRate: { ideal: 48_000 } }

export interface Recording {
  /** Stops and answers with what was heard; null when nothing was. */
  stop: () => Promise<Blob | null>
  cancel: () => void
}

/* The microphone's samples, kept as they come and made a WAV at the end. */
async function sampled(stream: MediaStream, release: () => void): Promise<Recording> {
  const context = new AudioContext()
  const pieces: Float32Array[] = []
  let source: MediaStreamAudioSourceNode | null = null
  let taker: ScriptProcessorNode | null = null
  const end = (): void => {
    if (taker) taker.onaudioprocess = null
    source?.disconnect()
    taker?.disconnect()
    void context.close()
    release()
  }
  try {
    source = context.createMediaStreamSource(stream)
    taker = context.createScriptProcessor(4096, 1, 1)
    taker.onaudioprocess = (event) => pieces.push(new Float32Array(event.inputBuffer.getChannelData(0)))
    source.connect(taker)
    /* A processor not led anywhere is never run; what it writes out is silence. */
    taker.connect(context.destination)
    await context.resume()
  } catch (error) {
    end()
    throw error
  }
  return {
    stop: async () => {
      end()
      return pieces.length > 0 ? wavOf(resampled(pieces, context.sampleRate)) : null
    },
    cancel: () => {
      pieces.length = 0
      end()
    },
  }
}

/**
 * Starts recording; rejects when the microphone cannot be had. Whatever
 * fails once the microphone is open closes it before the error goes on, or
 * the system's microphone light stays on with nothing recording.
 */
export async function record(format: string): Promise<Recording> {
  const stream = await navigator.mediaDevices.getUserMedia({ audio: MICROPHONE })
  const release = (): void => stream.getTracks().forEach((track) => track.stop())
  if (format === WAV) {
    try {
      return await sampled(stream, release)
    } catch (error) {
      release()
      throw error
    }
  }
  const chunks: Blob[] = []
  let recorder: MediaRecorder
  let done: Promise<void>
  try {
    recorder = new MediaRecorder(stream, { mimeType: format })
    recorder.ondataavailable = (event) => event.data.size > 0 && chunks.push(event.data)
    done = new Promise<void>((resolve) => (recorder.onstop = () => resolve()))
    recorder.start(1000)
  } catch (error) {
    release()
    throw error
  }
  return {
    stop: async () => {
      if (recorder.state !== 'inactive') recorder.stop()
      await done
      release()
      return chunks.length > 0 ? new Blob(chunks, { type: format.split(';')[0] }) : null
    },
    cancel: () => {
      chunks.length = 0
      if (recorder.state !== 'inactive') recorder.stop()
      release()
    },
  }
}

/** A blob as the base64 the paste command takes. */
export function base64Of(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const url = typeof reader.result === 'string' ? reader.result : ''
      resolve(url.slice(url.indexOf(',') + 1))
    }
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(blob)
  })
}
