import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { ActivityContext, activityOf, keepActivity, seen, type Activity } from './activity'
import { ASKS, CONTINUE, putBackFrom } from './chat'
import { ChatHead } from './ChatHead'
import { markdownOf, type Exported } from './chatMarkdown'
import type { Extras } from './queueing'
import { titleOf, type Tab } from './strip'
import { ChatFind } from './ChatFind'
import { useMessagesPick } from './chatMessages'
import { ChatThread } from './ChatThread'
import { Composer, type ComposerHandle } from './Composer'
import { DocPeek } from './DocPeek'
import { AppsScopeContext } from './mcpApps'
import { WholeFrom } from './WholeFrom'
import { DropTarget } from './DropTarget'
import { EditFrom } from './EditFrom'
import { PromptRail } from './PromptRail'
import { Quoting } from './Quoting'
import { useTaking } from './useTaking'
import { useThread } from './useOlder'
import { ChatBlank } from './ChatBlank'
import { ask, commands } from './live'
import { useThreadKeys } from './threadKeys'
import { useChat } from './useChat'
import { useJump, type Reveal } from './useJump'
import { useShell } from './useShell'
import { useShortcut } from './useShortcut'

/* A message sent again goes as it was written: its files are already named in its words. */
const NONE: Extras = { files: [], skills: [] }

/*
 * One conversation, per tab.
 *
 * The tab id is the conversation id: opening a second chat is a second
 * conversation, not a second view of the first.
 *
 * The thread and the composer are two centred columns rather than one
 * full-width sheet: a line of prose across a wide monitor is a line nobody
 * finishes, and the composer reads as a thing you write in rather than a bar
 * bolted to the bottom of the window.
 */

export function ChatPane({ tab }: { tab: Tab }): React.JSX.Element {
  const { active, project, rename, drafted, finished, show, threads, mark, reload, tellDoing } = useShell()
  const chat = useChat(tab.id)
  const { pasted, dropped } = useTaking(chat)
  const follow = useThread(project?.id ?? null, tab.id, chat)
  const composer = useRef<ComposerHandle>(null)

  const mine = active?.id === tab.id
  const where = useMemo(() => (project ? { projectId: project.id, conversationId: tab.id } : null), [project, tab.id])
  const reveal = useRef<Reveal | null>(null)
  const jump = useJump(where, chat, follow.hold, reveal)
  /* What the pane reads of the messages: none of it moves with a streamed batch. */
  const { count, said, asked } = useMessagesPick(chat.messages, (all) => ({
    count: all.length,
    said: all.find((one) => one.role === 'user'),
    asked: all.reduce((sum, one) => sum + (one.role === 'user' ? 1 : 0), 0),
  }))
  const [finding, setFinding] = useState(false)
  const [activity, setActivity] = useState(() => activityOf(tab.id))
  const pickActivity = useCallback((picked: Activity) => {
    keepActivity(tab.id, picked)
    setActivity(picked)
  }, [tab.id])
  useShortcut('findChat', () => {
    if (!mine || count === 0) return false
    setFinding(true)
  })
  /* A file an answer names opens beside the conversation, not over it. */
  const [peek, setPeek] = useState<string | null>(null)
  /* An edit waiting on whether the files changed since go back too. */
  const [editing, setEditing] = useState<{ turnId: string; files: number } | null>(null)
  const { messages, editFrom } = chat
  const edit = useCallback((turnId: string) => {
    const files = putBackFrom(messages.get(), turnId)
    if (files > 0) setEditing({ turnId, files })
    else editFrom(turnId, false)
  }, [messages, editFrom])
  /* Where a page an MCP tool came with is asked for: this conversation's account, in its folder. */
  const scope = useMemo(
    () => (chat.profileId && chat.folder ? { profileId: chat.profileId, cwd: chat.folder } : null),
    [chat.profileId, chat.folder],
  )

  useEffect(() => {
    if (tab.draft !== undefined) drafted(tab.id)
  }, [tab.draft, tab.id, drafted])

  const talk = threads.find((one) => one.id === tab.id)
  /* A name kept on the conversation wins; otherwise named once after the first thing you said. */
  const named = talk?.marks?.name
  useEffect(() => {
    if (named) {
      if (named !== tab.title) rename(tab.id, named)
      return
    }
    if (tab.title !== undefined || !said) return
    const name = titleOf(said.parts.map((part) => ('text' in part ? part.text : '')).join(''))
    if (name) rename(tab.id, name)
  }, [rename, said, tab.id, tab.title, named])

  /* A turn that ends while this tab is behind another marks the tab. */
  const wasSending = useRef(chat.sending)
  useEffect(() => {
    if (wasSending.current && !chat.sending) {
      finished(tab.id)
      reload()
    }
    wasSending.current = chat.sending
  }, [chat.sending, finished, reload, tab.id])

  /* What the sidebar row says: working, or waiting on you. */
  const doingNow = chat.asked.length > 0 ? 'waiting' : chat.sending ? 'working' : null
  useEffect(() => tellDoing(tab.id, doingNow), [tellDoing, tab.id, doingNow])
  useEffect(() => () => tellDoing(tab.id, null), [tellDoing, tab.id])

  /* Marked unread by hand: read again once you come back to it. */
  const markedUnread = Boolean(talk?.marks?.unread)
  const wasMine = useRef(false)
  useEffect(() => {
    if (mine && !wasMine.current && markedUnread) mark(tab.id, { unread: false })
    wasMine.current = mine
  }, [mine, markedUnread, mark, tab.id])

  useThreadKeys(mine, chat.messages, follow.box, reveal)
  const title = tab.title ?? 'New chat'
  const renamed = useCallback((next: string) => {
    rename(tab.id, next)
    mark(tab.id, { name: next })
  }, [rename, mark, tab.id])
  /* The whole conversation, not the pages drawn so far. */
  const exported = useCallback(async (what: Exported): Promise<string | null> => {
    if (!project) return null
    const whole = await ask(() => commands.chatHistory(project.id, tab.id, null, null))
    return whole.data ? markdownOf(title, whole.data.messages, what) : null
  }, [project, tab.id, title])

  const empty = count === 0 && !chat.error

  return (
    <>
      <ChatHead tabId={tab.id} title={title} onRename={renamed} onExport={exported} card={chat.card} context={chat.context} session={chat.session} cost={chat.cost} metered={chat.profiles.find((one) => one.id === chat.profileId)?.metered ?? true} orchestrator={project?.orchestrator ?? null} projectId={project?.id ?? null} sending={chat.sending} supervised={ASKS(chat.permission)} activity={activity} onActivity={pickActivity} />

      <DropTarget mine={mine} onDrop={dropped} />
      <div className="scroll" ref={follow.box}>
        {empty ? (
          <ChatBlank project={project} account={chat.profiles.find((one) => one.id === project?.orchestrator)?.label} onTry={(text) => composer.current?.put(text)} recent={threads.filter((one) => one.id !== tab.id).slice(0, 3)} onOpen={(one) => show('chat', { id: one.id, title: one.title })} />
        ) : (
          <AppsScopeContext.Provider value={scope}>
            <ActivityContext.Provider value={seen(activity, finding)}>
            <WholeFrom conversationId={tab.id}>
              <ChatThread messages={chat.messages} box={follow.box} top={follow.top} older={chat.older} stalled={follow.stalled} onMore={follow.more} sending={chat.sending} error={chat.error} folder={chat.folder ?? null} rewindable={chat.rewindable} onRewind={chat.rewind} onEdit={edit} onOpen={setPeek} onContinue={() => chat.say(CONTINUE)} onRetry={(text) => chat.say(text, undefined, NONE)} onResend={(text) => (chat.sending ? void chat.steer(text, NONE) : chat.say(text, undefined, NONE))} reveal={reveal} />
            </WholeFrom>
            </ActivityContext.Provider>
          </AppsScopeContext.Provider>
        )}
      </div>

      {!empty && <PromptRail where={where} said={`${asked}:${chat.sending}`} jump={jump} />}
      {finding && <ChatFind where={where} box={follow.box} jump={jump} onDone={() => setFinding(false)} />}
      <Quoting box={follow.box} onQuote={(text) => composer.current?.add(text)} />
      {editing && <EditFrom files={editing.files} onCancel={() => setEditing(null)} onPick={(putBack) => (chat.editFrom(editing.turnId, putBack), setEditing(null))} />}

      <Composer tab={tab} chat={chat} project={project ?? null} mine={mine} away={follow.away} toEnd={follow.toEnd} handle={composer} pasted={pasted} />
      {peek && <DocPeek path={peek} onOpen={setPeek} onClose={() => setPeek(null)} />}
    </>
  )
}
