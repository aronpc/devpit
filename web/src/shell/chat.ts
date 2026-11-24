import { Channel, invoke } from '@tauri-apps/api/core'

import type {
  Ask,
  Conversation,
  Frame,
  Message,
  Profile,
  TurnEnd,
} from '../gen/bindings'
import { formatDate } from '../i18n/format'
import { t } from '../i18n'
import { inBrowser } from '../transport'
import { millis, outcome, took } from './turnFold'
import { inTauri } from './window'

/* chat.send streams over a Channel, like the pty, so it is hand-written for
   the same reason: specta cannot describe the channel type. */
export interface Sent {
  readonly end: Promise<TurnEnd>
}

export function send(ask: Ask, onFrame: (frame: Frame) => void): Sent | null {
  /* Streamed over a Tauri channel: in a browser, not yet (R3). */
  if (!inTauri() || inBrowser()) return null
  const channel = new Channel<Frame>()
  channel.onmessage = onFrame
  return { end: invoke('chat_send', { ask, onFrame: channel }) as Promise<TurnEnd> }
}

/* The turns running in a conversation, from where they are: what they sent
   before this chat was open, then the rest, one turn after another. Resolves
   when the last ends — `false` at once when nothing was running. `leave`
   stops the frames: a chat that has gone would otherwise be sent every one
   until the turn ended, once per remount. */
export interface Joined {
  readonly running: Promise<boolean>
  readonly leave: () => void
}

export function rejoin(conversationId: string, onFrame: (frame: Frame) => void): Joined | null {
  /* Streamed over a Tauri channel: in a browser, not yet (R3). */
  if (!inTauri() || inBrowser()) return null
  const channel = new Channel<Frame>()
  channel.onmessage = onFrame
  return {
    running: invoke('chat_rejoin', { conversationId, onFrame: channel }) as Promise<boolean>,
    leave: () => void invoke('chat_leave', { conversationId, channel: channel.id }).catch(() => {}),
  }
}

/* One opening of a conversation in a chat: a mount, or the next conversation
   shown in the same pane. A frame or an answer still on its way to one that
   closed lands nowhere — a single flag, set again by the next opening, let an
   old rejoin write one conversation's turn into another. */
export interface Opening {
  readonly open: () => boolean
  /** Kept to be left when this opening closes. */
  readonly join: (joined: Joined | null) => Joined | null
  readonly close: () => void
  /** This chat's own turn is running; a turn woken meanwhile waits for it. */
  sending: boolean
  woke: boolean
}

export function opening(): Opening {
  let open = true
  const joined = new Set<Joined>()
  return {
    open: () => open,
    join: (one) => {
      if (one && open) joined.add(one)
      else one?.leave()
      return one
    },
    close: () => {
      open = false
      for (const one of joined) one.leave()
      joined.clear()
    },
    sending: false,
    woke: false,
  }
}

/* Frames arrive many to a paint while an answer streams; they are applied
   together, once per frame drawn, rather than one render each, and just
   after that frame is painted: the messages' store draws at once, and drawn
   inside the frame it made the frame late. A timer stands in when no frame
   comes: WebKitGTK gives none to a hidden window, nor to some visible ones,
   and the whole answer then waited for the end. */
export function batched(apply: (frames: readonly Frame[]) => void): { push: (frame: Frame) => void; flush: () => void } {
  let pending: Frame[] = []
  let scheduled: { cancel: () => void } | null = null
  const flush = (): void => {
    scheduled?.cancel()
    scheduled = null
    if (pending.length === 0) return
    const frames = pending
    pending = []
    apply(frames)
  }
  const push = (frame: Frame): void => {
    pending.push(frame)
    if (scheduled) return
    const timer = setTimeout(flush, 100)
    let after: ReturnType<typeof setTimeout> | undefined
    const painted = document.hidden ? null : requestAnimationFrame(() => (after = setTimeout(flush, 0)))
    scheduled = {
      cancel: () => {
        clearTimeout(timer)
        clearTimeout(after)
        if (painted !== null) cancelAnimationFrame(painted)
      },
    }
  }
  return { push, flush }
}

export { applied, appended, appliedAll, failedOn, joined, prepended, retaken, withFiles, withHeard } from './frames'

/* A message sent into the turn in flight sits before the answer still being
   written, where the transcript keeps it. */
export function steered(messages: readonly Message[], said: Message): readonly Message[] {
  /* The turn's relay may have drawn it before the steer answered. */
  if (messages.some((one) => one.id === said.id)) return messages
  const at = messages.findLastIndex((one) => one.streaming)
  return at < 0 ? [...messages, said] : [...messages.slice(0, at), said, ...messages.slice(at)]
}

/* A turn that is over, however it ended, leaves nothing streaming: a refused
   or failed turn sent no frame that would say so, and its answer kept the
   working dots until a reload. The same list when nothing changes. */
export function settled(messages: readonly Message[]): readonly Message[] {
  if (!messages.some((one) => one.streaming)) return messages
  return messages.map((one) => (one.streaming ? { ...one, streaming: false } : one))
}

/* The profile a conversation is tied to, or nothing before its first turn.
   Fixed on purpose: one transcript, one account. */
export function fixedTo(conversation: Conversation | null): string | null {
  return conversation?.profile ? conversation.profile : null
}

/* What the composer may offer: the installed profiles, narrowed to the one
   the conversation already belongs to. A profile off the PATH is not an
   option — picking it would only fail at send time. */
export function choices(
  profiles: readonly Profile[],
  fixed: string | null,
): readonly Profile[] {
  // `runnable` and not merely installed: the composer spawns a process, and a
  // profile the shell alone knows — a function in someone's `.zshrc` — has no
  // process to spawn. The terminal can still open it, which is why the two are
  // different answers.
  const usable = profiles.filter((profile) => profile.reach === 'runnable')
  return fixed ? usable.filter((profile) => profile.id === fixed) : usable
}

/* Whether the send button does anything. */
export function ready(prompt: string, sending: boolean, profileId: string | null): boolean {
  return !sending && profileId !== null && prompt.trim().length > 0
}

/* What a conversation has cost, or nothing when it has cost nothing yet.
   A `$0.00` on screen is noise wearing the clothes of information. */
export function money(usd: number): string | null {
  if (!(usd > 0)) return null
  return usd < 0.01 ? `$${usd.toFixed(4)}` : `$${usd.toFixed(2)}`
}

/* What a conversation spent, for an agent that may not say: one that reports
   no cost reads as unavailable, never as nothing spent. */
export const spent = (usd: number, metered: boolean): string | null => money(usd) ?? (metered ? null : t('workspace.meter.na'))

/* What the agent may do without asking.

   `manual` stops and asks, and the question arrives in the thread with accept
   and refuse on it — which is what makes it an option at all. It was left out
   while there was nowhere to answer. */
export interface Mode {
  readonly id: string
  readonly label: string
  /* What it actually does. "Ask" names nothing on its own — the whole point
     of the row is knowing what you are agreeing to. */
  readonly what: string
}

/* A function, so the words follow the language the person chose. */
export const modes = (): readonly Mode[] => [
  { id: 'manual', label: t('sessions.mode.manual.label'), what: t('sessions.mode.manual.what') },
  { id: 'acceptEdits', label: t('sessions.mode.acceptEdits.label'), what: t('sessions.mode.acceptEdits.what') },
  { id: 'bypassPermissions', label: t('sessions.mode.bypassPermissions.label'), what: t('sessions.mode.bypassPermissions.what') },
  { id: 'plan', label: t('sessions.mode.plan.label'), what: t('sessions.mode.plan.what') },
]

export const modeName = (id: string): string =>
  modes().find((mode) => mode.id === id)?.label ?? id

/* How hard the agent is asked to think, as words rather than the CLI's own
   tokens: `xhigh` is an argument, not a label. */
const EFFORT_LEVELS = ['low', 'medium', 'high', 'xhigh', 'max']

export const effortName = (effort: string): string =>
  EFFORT_LEVELS.includes(effort) ? t(`sessions.effort.${effort}`) : effort

/* Which mode stops to ask. The session is told to hold its tools only in that
   one — a turn held in a mode that never asks would wait for a question that
   never comes. */
export const ASKS = (mode: string): boolean => mode === 'manual'

/* Whether a conversation stopped with the person's message unanswered.

   The answer is written when the turn ends, so an app that closed part way
   through leaves exactly this: a question and nothing under it. Said on
   screen, it reads as what happened; left silent, it reads as a message that
   was never sent. */
export function unanswered(messages: readonly Message[], sending: boolean): boolean {
  if (sending) return false
  const last = messages.at(-1)
  return last !== undefined && last.role === 'user'
}


/* What "Continue" sends: the agent is asked to look before it goes on, since
   the step it was on may have half happened. */
export const CONTINUE = 'Your last turn stopped before it finished. Check what its last step left behind, then carry on from there.'

/* Whether the conversation can be picked up where it stopped: a message with
   no answer saved, or an answer that was stopped part way. */
export function continuable(messages: readonly Message[], sending: boolean): boolean {
  if (sending) return false
  const last = messages.at(-1)
  if (!last) return false
  if (last.role === 'user') return true
  return last.role === 'assistant' && !last.streaming && last.ended != null && outcome(last.ended) === 'stopped'
}

/* A message's words, as the person sent them. */
export const saidText = (message: Message): string => message.parts.map((part) => ('text' in part ? part.text : '')).join('')

/* What "Retry" sends again: the last thing said, when the turn it started
   failed or never answered. */
export function retryable(messages: readonly Message[], sending: boolean): string | null {
  const last = messages.at(-1)
  if (sending || !last) return null
  const failed = last.role === 'user' || (last.role === 'assistant' && !last.streaming && last.ended != null && outcome(last.ended) === 'failed')
  const said = failed ? messages.findLast((one) => one.role === 'user') : undefined
  return said ? saidText(said) : null
}

/* How many files an edit from `turnId` can put back: what that turn and the
   later ones changed, when the checkout they found was kept. */
export function putBackFrom(messages: readonly Message[], turnId: string): number {
  const from = messages.findIndex((one) => one.turnId === turnId)
  if (from < 0) return 0
  const changes = messages.slice(from).flatMap((one) => one.parts.flatMap((part) => (part.kind === 'changes' ? [part] : [])))
  if (!changes[0]?.before) return 0
  return new Set(changes.flatMap((part) => part.files.map((file) => file.path))).size
}

/* A time of day in the language the person chose, not the machine's. */
export const clock = (ms: number): string => formatDate(ms, { hour: '2-digit', minute: '2-digit' })

/* The quiet line under a turn: when it ended, on what, how long, how much. */
export function metaOf(message: Pick<Message, 'createdAt' | 'ended'>): readonly string[] {
  const ended = message.ended
  const began = millis(message.createdAt)
  const at = millis(ended?.at) ?? began
  const ms = ended?.durationMs ?? (began !== null && ended ? (millis(ended.at) ?? began) - began : null)
  return [
    at === null ? null : clock(at),
    ended?.model?.replace(/^claude-/, '').replace(/-\d{8}$/, ''),
    ended?.effort,
    ms ? took(ms) : null,
    ended?.tokens ? `${Math.round(ended.tokens / 1000)}k tokens` : null,
    // Tokens counted and no cost: the model is one the price table does not know.
    ended?.costUsd ? money(ended.costUsd) : ended?.tokens && ended.costUsd == null ? t('sessions.chat.costNa') : null,
  ].filter((one): one is string => Boolean(one))
}
