import { memo, useImperativeHandle, useRef } from 'react'

import type { Project } from '../gen/bindings'
import { Asked } from './Asked'
import { Chips } from './Chips'
import { ready, withHeard } from './chat'
import { useMessagesPick } from './chatMessages'
import { ChatWhere } from './ChatWhere'
import { ComposerFiles } from './ComposerFiles'
import { ComposerStatus } from './ComposerStatus'
import { ComposerTools } from './ComposerTools'
import { useDraftShortcut } from './draftShortcut'
import { JumpToEnd } from './JumpToEnd'
import { MentionMenu } from './MentionMenu'
import { SessionsWaiting } from './OrchestratorChat'
import { keysLabel, matches } from './shortcuts'
import { OwnCommand } from './OwnCommand'
import { Queued } from './Queued'
import { useRecall } from './recall'
import { SecretWarning } from './SecretWarning'
import { SendButton } from './SendButton'
import { SkillPills } from './SkillPills'
import { SlashMenu } from './SlashMenu'
import { targetOf } from './stop'
import type { Tab } from './strip'
import { committed } from './typing'
import type { Chat } from './useChat'
import { keptStash, swapStash, useDraft } from './useDraft'
import { useMention } from './useMention'
import { useQueue } from './useQueue'
import type { Extras } from './queueing'
import { useSlash } from './useSlash'
import { useStop } from './useStop'
import { useShellPick } from './shellStore'
import { followUpOf, sendKeyOf, useViewPrefs } from './viewPrefs'
import { t } from '../i18n'

/* What the conversation above may do to what is being written. */
/** `put` replaces what is typed; `add` goes after it. */
export type ComposerHandle = { put: (text: string) => void; add: (text: string) => void }

/*
 * Where a message is written. Its own component, holding the text: a key
 * pressed here draws the composer again, never the conversation above it.
 * Memoised, and it picks only the turn in flight from the messages: a
 * streamed batch does not draw it.
 */
export const Composer = memo(function Composer({
  tab,
  chat,
  project,
  mine,
  away,
  toEnd,
  handle,
  pasted,
}: {
  tab: Tab
  chat: Chat
  project: Project | null
  mine: boolean
  away: boolean
  toEnd: () => void
  handle: React.Ref<ComposerHandle>
  pasted: (event: React.ClipboardEvent<HTMLTextAreaElement>) => void
}): React.JSX.Element {
  /* A chat opened from a card starts with the card in the composer, unsent. */
  const [prompt, setPrompt] = useDraft(tab.id, tab.draft)
  const slash = useSlash(chat.profileId, prompt, setPrompt, Boolean(project?.orchestrator))
  const field = useRef<HTMLTextAreaElement>(null)
  const mention = useMention(project?.id ?? null, prompt, field, setPrompt)
  useImperativeHandle(
    handle,
    () => ({
      put: (text) => (setPrompt(text), field.current?.focus()),
      add: (text) => (setPrompt((was) => (was.trim() ? `${was.trimEnd()}\n\n${text}` : text)), field.current?.focus()),
    }),
    [setPrompt],
  )

  /* Typed while a turn runs, a message waits for it rather than being
     refused, and takes what was attached to it along. */
  const queue = useQueue(project?.id ?? null, tab.id, chat)
  const sendDraft = useDraftShortcut(project?.orchestrator ?? null, setPrompt)
  const extras = (): Extras => ({ files: chat.files, skills: chat.skills })
  const prefs = useViewPrefs()
  const show = useShellPick((shell) => shell.show)
  /* `other` is the second key's way: into the turn when the first queues, and the reverse. */
  const send = (other = false): void => {
    if (!ready(prompt, false, chat.profileId)) return
    if (sendDraft(prompt) || slash.ran(prompt)) return setPrompt('')
    if (chat.sending && followUpOf(prefs, other) === 'steer') return steerTyped()
    if (chat.sending) {
      queue.add(prompt, extras())
      chat.clearExtras()
    } else chat.say(prompt, (back) => setPrompt((was) => was || back))
    setPrompt('')
    toEnd()
  }
  /* Into the turn in flight; refused because it is ending, it goes next instead. */
  const steerTyped = (): void => {
    if (!ready(prompt, false, chat.profileId)) return
    const typed = prompt
    const given = extras()
    setPrompt('')
    chat.clearExtras()
    void chat.steer(typed, given).then((taken) => taken || queue.add(typed, given))
    toEnd()
  }
  const recall = useRecall(chat.messages, prompt, setPrompt, chat.files.length > 0)
  const streaming = useMessagesPick(chat.messages, (all) => all.find((one) => one.streaming)?.turnId ?? null)

  /* A stop holds the queue: it was meant for a turn that is no longer going
     to happen as planned, so nothing goes until the person says so. */
  const halt = (): void => {
    queue.pause()
    chat.stop()
  }
  const giveBack = (): void => {
    const back = queue.takeAll()
    if (!back) return
    setPrompt((was) => [back.prompt, was].filter(Boolean).join('\n\n'))
    for (const file of back.files) chat.keep(file)
    if (back.skills.length > 0) chat.setSkills([...new Set([...chat.skills, ...back.skills])])
    field.current?.focus()
  }

  /* The turn in flight is the one still streaming; arming is tied to it so a
     press that armed against a finished turn cannot stop the next one. */
  const showEsc = useStop(
    chat.sending && mine,
    targetOf(tab.id, streaming),
    halt,
  )

  return (
    <div className="composer">
      <div className="composer__col">
        <JumpToEnd away={away} onJump={toEnd} />
        {/* The question sits above the composer, where your hands are — not
            in the thread, where it scrolls away from you. */}
        {/* A session stopped on the person, answered from here: the one
            thing about the others that cannot wait for a glance aside. */}
        {project?.orchestrator && <SessionsWaiting profileId={project.orchestrator} />}
        <Asked questions={chat.asked} onAnswer={chat.answer} />
        <ComposerStatus conversationId={tab.id} messages={chat.messages} />
        <SecretWarning text={prompt} />
        <Queued queue={queue} sending={chat.sending} onBack={giveBack} />

        <div className="composer__in">
          <ComposerFiles chat={chat} />

          <SkillPills picked={chat.skills} onChange={chat.setSkills} profile={chat.profiles.find((one) => one.id === chat.profileId)} />
          <SlashMenu slash={slash} />
          <MentionMenu menu={mention} />
          <OwnCommand slash={slash} from={tab.id} projectId={project?.id ?? null} profileId={chat.profileId} orchestrator={project?.orchestrator ?? null} onClose={() => (slash.closeOwn(), field.current?.focus())} />
          <textarea
            className="composer__ph"
            aria-label={t('workspace.composer.label')}
            placeholder={!prompt && keptStash(tab.id) ? t('workspace.composer.stashed', { keys: keysLabel('stashDraft') }) : t('workspace.composer.placeholder')}
            ref={field}
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            onPaste={pasted}
            onKeyDown={(event) => {
              /* Enter sends, Shift+Enter a new line; `committed` keeps an accent's Enter out. */
              if (slash.keyDown(event) || mention.keyDown(event) || recall(event)) return
              if (matches('stashDraft', event)) return (event.preventDefault(), setPrompt(swapStash(tab.id, prompt)))
              if (!committed(event)) return
              /* Held down, it would steer the whole queue in, one repeat at a time. */
              if (matches('steerQueued', event)) return (event.preventDefault(), event.repeat || queue.first())
              if (matches('sendBackground', event)) return (event.preventDefault(), ready(prompt, false, chat.profileId) && (send(), show('chat')))
              const action = sendKeyOf(event, prefs.sendKey)
              if (action) {
                event.preventDefault()
                send(action === 'other')
              }
            }}
          />

          <div className="composer__row">
            {/* A control takes focus and gives it back: the cursor was mid-sentence. Cancelling
                the press instead would be cheaper and swallow the click that opens the menu. */}
            <Chips chat={chat} refocus={() => field.current?.focus()} />
            <ComposerTools chat={chat} mine={mine} onHeard={(text) => (setPrompt((was) => withHeard(was, text)), field.current?.focus())} />
            <SendButton sending={chat.sending} showEsc={showEsc} can={ready(prompt, chat.sending, chat.profileId)} onSend={() => send()} onStop={halt} onForce={chat.forceable ? chat.force : undefined} />
          </div>
        </div>

        <ChatWhere />
      </div>
    </div>
  )
})
