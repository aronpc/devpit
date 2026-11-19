import { useEffect, useState } from 'react'

import type { ConversationPrompts } from '../gen/bindings'
import { ask, commands } from './live'
import { t } from '../i18n'

/* Where a prompt sits along the rail, from the top of the conversation to its end. */
function railAt(at: number, total: number): string {
  return `${total > 1 ? (at / (total - 1)) * 100 : 0}%`
}

/*
 * What the person asked, as ticks down the side of the conversation: each
 * shows its words when pointed at and goes there when clicked, reading the
 * pages above when it is not drawn yet. All of the conversation, not the page
 * the window holds, so it is read from the backend.
 */
export function PromptRail({
  where,
  said,
  jump,
}: {
  where: { projectId: string; conversationId: string } | null
  /** Changes when the person says something: the rail is read again. */
  said: string
  jump: (id: string) => Promise<boolean>
}): React.JSX.Element | null {
  const [read, setRead] = useState<ConversationPrompts | null>(null)

  useEffect(() => {
    if (!where) return
    let gone = false
    void ask(() => commands.chatPrompts(where.projectId, where.conversationId)).then((answer) => {
      if (!gone && answer.data) setRead(answer.data)
    })
    return () => {
      gone = true
    }
  }, [where, said])

  /* One prompt is no way around: there is nowhere else to go. */
  if (!read || read.prompts.length < 2) return null
  return (
    <nav className="prail" aria-label={t('workspace.promptRail.label')}>
      {read.prompts.map((one) => (
        <button key={one.messageId} className="prail__tick" style={{ top: railAt(one.at, read.total) }} aria-label={one.text} onClick={() => void jump(one.messageId)}>
          <span className="prail__peek">{one.text}</span>
        </button>
      ))}
    </nav>
  )
}
