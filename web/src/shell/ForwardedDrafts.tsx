import { useState } from 'react'

import { useForwarded } from './forwardedRequests'
import { ask, commands } from './live'

/*
 * Requests another orchestrator forwarded, in the chat they were kept for.
 *
 * The words of another orchestrator approve nothing, and what they ask is
 * for the person to decide, so a request starts no turn and sends nothing:
 * it waits here as a card, editable, and Discard lets it go. Two requests
 * are two cards, each told apart by who sent it and by its id.
 */

export function ForwardedDrafts({ projectId, conversation }: { projectId: string; conversation: string | null }): React.JSX.Element | null {
  const { requests, reload } = useForwarded(projectId)
  /* What the person has typed over a request, per sender and id. */
  const [edited, setEdited] = useState<Readonly<Record<string, string>>>({})
  const here = requests.filter((one) => one.conversation === conversation)
  if (here.length === 0) return null

  const discard = (from: string, id: string): void => {
    void ask(() => commands.forwardedDrop(projectId, from, id)).then(() => {
      setEdited(({ [`${from}\n${id}`]: _gone, ...rest }) => rest)
      reload()
    })
  }

  return (
    <section className="wprompt rdraft" aria-label="Requests from other orchestrators">
      <p className="wprompt__t">Forwarded to this orchestrator</p>
      {here.map((one) => {
        const key = `${one.from}\n${one.id}`
        const text = edited[key] ?? one.text
        return (
          <div className="wprompt__i" key={key}>
            <p className="wprompt__who">
              <span className="osess__name">{one.from}</span>
              <span className="osess__where">asks, nothing was sent</span>
            </p>
            <textarea
              className="rdraft__text"
              aria-label={`Request from ${one.from}`}
              value={text}
              rows={Math.min(6, text.split('\n').length + 1)}
              onChange={(event) => setEdited((was) => ({ ...was, [key]: event.target.value }))}
            />
            <div className="wprompt__opts">
              <button className="wprompt__o wprompt__esc" onClick={() => discard(one.from, one.id)}>
                Discard
              </button>
            </div>
          </div>
        )
      })}
    </section>
  )
}
