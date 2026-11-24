import { useState } from 'react'

import type { Comment } from '../gen/bindings'
import { Markdown } from './MarkdownView'
import { since } from './projects'
import { matches } from './shortcuts'
import { composing } from './typing'
import { t } from '../i18n'

/*
 * The card's conversation.
 *
 * Oldest first, because that is the order it was said in. Markdown, because
 * half of what lands here is written by an agent and an agent writes markdown.
 *
 * The author is an id — `you`, or the agent's — and the name is resolved here.
 * A display name stored in a row is a name that is wrong the day it changes.
 */

const NAMES: Readonly<Record<string, string>> = {
  claude: 'Claude Code',
  codex: 'Codex',
  gemini: 'Gemini',
  opencode: 'OpenCode',
}

/* `you` and an agent that wrote through devpit without saying which one it is
   are words, so they are said in the language the person chose. */
const nameOf = (author: string): string =>
  author === 'you' ? t('workspace.comments.you') : author === 'agent' ? t('workspace.comments.agent') : (NAMES[author] ?? author)

export function Comments({
  comments,
  onSay,
  onEdit,
  onDelete,
}: {
  comments: readonly Comment[]
  onSay: (body: string) => Promise<string | null>
  onEdit: (id: string, body: string) => Promise<string | null>
  onDelete: (id: string) => void
}): React.JSX.Element {
  const [draft, setDraft] = useState('')
  const [editing, setEditing] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  /* The field keeps what was typed until the write succeeds. A box that
     clears itself on a refusal loses the thing it was refusing. */
  const say = async (): Promise<void> => {
    if (!draft.trim()) return
    const refused = await onSay(draft)
    setProblem(refused)
    if (!refused) setDraft('')
  }

  return (
    <section className="conv">
      <h2 className="cardp__h">{t('workspace.comments.title')}</h2>

      {comments.length === 0 && <p className="pref__d">{t('workspace.comments.none')}</p>}

      {comments.map((one) =>
        editing === one.id ? (
          <Editing
            key={one.id}
            comment={one}
            onCancel={() => setEditing(null)}
            onDone={async (body) => {
              const refused = await onEdit(one.id, body)
              setProblem(refused)
              if (!refused) setEditing(null)
            }}
          />
        ) : (
          <article className="conv__one" key={one.id} data-mine={one.author === 'you'}>
            <header className="conv__who">
              <span className="conv__name">{nameOf(one.author)}</span>
              <span className="conv__when">{since(one.createdAt)}</span>
              {/* Marked rather than silent: a line that can change under you
                  with no mark is a line you cannot rely on. */}
              {one.editedAt && <span className="conv__edited">{t('workspace.comments.edited')}</span>}
              <span className="conv__acts">
                <button className="conv__act" onClick={() => setEditing(one.id)}>
                  {t('workspace.comments.edit')}
                </button>
                <button className="conv__act" data-danger onClick={() => onDelete(one.id)}>
                  {t('workspace.comments.delete')}
                </button>
              </span>
            </header>
            <Markdown source={one.body} />
          </article>
        ),
      )}

      <div className="conv__new">
        <textarea
          className="conv__f"
          value={draft}
          rows={3}
          placeholder={t('workspace.comments.placeholder')}
          onChange={(event) => setDraft(event.target.value)}
          /* Enter is a newline here — this is prose, not a chat line, and
             half of what gets pasted in is a stack trace. */
          onKeyDown={(event) => {
            if (matches('comment', event) && !composing(event)) {
              event.preventDefault()
              void say()
            }
          }}
        />
        <div className="ask__row">
          <button className="btn btn--go" disabled={!draft.trim()} onClick={() => void say()}>
            {t('workspace.comments.comment')}
          </button>
        </div>
      </div>

      {problem && <p className="wtb__no">{problem}</p>}
    </section>
  )
}

function Editing({
  comment,
  onDone,
  onCancel,
}: {
  comment: Comment
  onDone: (body: string) => void
  onCancel: () => void
}): React.JSX.Element {
  const [draft, setDraft] = useState(comment.body)
  return (
    <article className="conv__one">
      <textarea
        className="conv__f"
        value={draft}
        rows={4}
        autoFocus
        aria-label={t('workspace.comments.editLabel')}
        onChange={(event) => setDraft(event.target.value)}
      />
      <div className="ask__row">
        <button className="btn" onClick={onCancel}>
          {t('workspace.comments.cancel')}
        </button>
        <button className="btn btn--go" disabled={!draft.trim()} onClick={() => onDone(draft)}>
          {t('workspace.comments.save')}
        </button>
      </div>
    </article>
  )
}
