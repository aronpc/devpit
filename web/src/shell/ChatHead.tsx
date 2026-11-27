import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { memo, useEffect, useState } from 'react'

import type { Context } from '../gen/bindings'
import type { Activity } from './activity'
import { ActivityPick } from './ActivityPick'
import { spent } from './chat'
import { ChatCardChip } from './ChatCardChip'
import { useDetails, useDetailsMenu } from './chatDetails'
import type { Exported } from './chatMarkdown'
import { ContextMeter } from './ContextMeter'
import { AssistantToggle } from './AssistantToggle'
import { CopySession } from './CopySession'
import { Menu, MenuItem } from './Menu'
import { SessionsChip } from './OrchestratorChat'
import { PaneCorner } from './PaneCorner'
import { RemoteToggle } from './RemoteToggle'
import { Rename } from './Rename'
import { SessionCost } from './SessionCost'
import { COPIED_MS } from './useCopied'
import type { ChatCard } from './useChat'
import { t } from '../i18n'
import { Icon } from './Icon'

/*
 * A thin bar over the conversation: its name, renamed where it is, and what
 * the corner used to float over the text — the card, the context, the cost
 * and the pane's own actions.
 */

export const ChatHead = memo(function ChatHead({
  tabId,
  title,
  onRename,
  onExport,
  card,
  context,
  session,
  cost,
  metered,
  orchestrator,
  projectId,
  sending,
  supervised,
  activity,
  onActivity,
}: {
  tabId: string
  title: string
  onRename: (title: string) => void
  /** The conversation as markdown, or nothing when it could not be read. */
  onExport: (what: Exported) => Promise<string | null>
  card: ChatCard | null
  context: Context | null
  session: string | null
  cost: number
  /** Whether its agent reports cost and context; when not, they read n/a. */
  metered: boolean
  orchestrator: string | null
  projectId: string | null
  sending: boolean
  supervised: boolean
  activity: Activity
  onActivity: (activity: Activity) => void
}): React.JSX.Element {
  const [renaming, setRenaming] = useState(false)
  const shows = useDetails()
  const details = useDetailsMenu()
  return (
    <div className="chead" onContextMenu={details.open}>
      {details.menu}
      <PaneCorner tabId={tabId} what="chat">
        {renaming ? (
          <Rename
            className="chead__t"
            value={title}
            editing
            onDone={(next) => {
              if (next) onRename(next)
              setRenaming(false)
            }}
          />
        ) : (
          <button className="chead__t" title={t('sessions.chat.rename')} onClick={() => setRenaming(true)}>
            {title}
          </button>
        )}
        <ActivityPick value={activity} onPick={onActivity} />
        {shows('card') && <ChatCardChip card={card} />}
        {shows('context') && <ContextMeter context={context} metered={metered} />}
        {shows('cost') && (metered && session ? <SessionCost sessionId={session} className="pcorner__cost scost" /> : spent(cost, metered) && <span className="pcorner__cost">{spent(cost, metered)}</span>)}
        <ChatExport onExport={onExport} />
        {session && <CopySession id={session} />}
        {orchestrator && <SessionsChip profileId={orchestrator} />}
        {orchestrator && projectId && <AssistantToggle projectId={projectId} />}
        {projectId && <RemoteToggle projectId={projectId} conversationId={tabId} sending={sending} supervised={supervised} />}
      </PaneCorner>
    </div>
  )
})

/* Copies the conversation as markdown: what was said, or that and every tool it ran. */
function ChatExport({ onExport }: { onExport: (what: Exported) => Promise<string | null> }): React.JSX.Element {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null)
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    if (!copied) return
    const timer = window.setTimeout(() => setCopied(false), COPIED_MS)
    return () => window.clearTimeout(timer)
  }, [copied])
  /* Through the app's clipboard, as the other copy menus go: the write comes
     after the whole conversation is read, no longer inside the click. */
  const pick = (what: Exported): void => {
    setAt(null)
    void onExport(what).then(async (text) => {
      if (!text) return
      await writeText(text)
      setCopied(true)
    })
  }
  return (
    <>
      <button
        className="sq26"
        aria-label={copied ? t('sessions.chat.copied') : t('sessions.chat.export')}
        title={t('sessions.chat.export')}
        aria-haspopup="menu"
        onClick={(event) => {
          const box = event.currentTarget.getBoundingClientRect()
          setAt(at ? null : { x: box.left, y: box.bottom + 4 })
        }}
      >
        {copied ? (
          <Icon size="s"><path d="m5 13 4 4L19 7" /></Icon>
        ) : (
          <Icon size="s"><path d="M12 3v12M7 10l5 5 5-5M5 21h14" /></Icon>
        )}
      </button>
      {at && (
        <Menu at={at} label={t('sessions.chat.export')} onClose={() => setAt(null)}>
          <MenuItem label={t('sessions.chat.copyMd')} onPick={() => pick('messages')} />
          <MenuItem label={t('sessions.chat.copyMdTools')} onPick={() => pick('tools')} />
        </Menu>
      )}
    </>
  )
}
