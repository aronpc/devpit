import { useState } from 'react'

import { useForwarded } from './forwardedRequests'
import { ask, commands } from './live'

/*
 * Requests another orchestrator forwarded, in the chat they were kept for.
 *
 * The words of another orchestrator approve nothing, and what they ask is
 * for the person to decide, so a request starts no turn and sends nothing:
 * it waits here as a card, editable. Send is the person's click: it starts
 * the turn, with the words as they were left, and the request leaves when
 * the turn starts. Discard lets it go. Two requests are two cards, each
 * told apart by who sent it and by its id.
 */

export function ForwardedDrafts({ projectId, conversation }: { projectId: string; conversation: string | null }): React.JSX.Element | null {
  const { requests, reload } = useForwarded(projectId)
  /* What the person has typed over a request, per sender and id. */
  const [edited, setEdited] = useState<Readonly<Record<string, string>>>({})
  /* What refused a send, per sender and id: the request stays as it was. */
  const [refused, setRefused] = useState<Readonly<Record<string, string>>>({})
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set())
  const here = requests.filter((one) => one.conversation === conversation)
  if (here.length === 0) return null

  const discard = (from: string, id: string): void => {
    const key = `${from}\n${id}`
    setPending((was) => new Set(was).add(key))
    void ask(() => commands.forwardedDrop(projectId, from, id)).then(() => {
      setEdited(({ [key]: _gone, ...rest }) => rest)
      reload()
    }).finally(() => setPending((was) => without(was, key)))
  }

  const send = (from: string, id: string, text: string): void => {
    const key = `${from}\n${id}`
    setPending((was) => new Set(was).add(key))
    void ask(() => commands.forwardedSend(projectId, from, id, text)).then((said) => {
      if (said.error) return setRefused((was) => ({ ...was, [key]: said.error ?? '' }))
      setRefused(({ [key]: _gone, ...rest }) => rest)
      setEdited(({ [key]: _words, ...rest }) => rest)
      reload()
    }).finally(() => setPending((was) => without(was, key)))
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
            {refused[key] && <p role="alert">{refused[key]}</p>}
            <div className="wprompt__opts">
              <button className="wprompt__o" disabled={pending.has(key)} onClick={() => send(one.from, one.id, text)}>
                Send
              </button>
              <button className="wprompt__o wprompt__esc" disabled={pending.has(key)} onClick={() => discard(one.from, one.id)}>
                Discard
              </button>
            </div>
          </div>
        )
      })}
    </section>
  )
}

function without(values: ReadonlySet<string>, value: string): ReadonlySet<string> {
  const left = new Set(values)
  left.delete(value)
  return left
}
