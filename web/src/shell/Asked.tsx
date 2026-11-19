import { useState } from 'react'

import type { Question } from '../gen/bindings'
import { allowKey, alwaysOffered, grantsOf, questionsOf, subjectOf, useFocused, type Chosen } from './choices'
import { Choosing } from './Choosing'
import { Markdown } from './MarkdownView'
import { committed, composing } from './typing'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'

/*
 * What the agent is waiting to be allowed to do, or waiting to be told.
 *
 * Above the composer, where your hands already are — not in the thread,
 * where it scrolls away from the person who has to answer it, and not in a
 * dialog, which would hide the output that says why the agent wants this.
 */

/** What "always" will cover, said before it is pressed. */
const alwaysMeans = (tool: string): string =>
  ['Edit', 'MultiEdit', 'Write', 'NotebookEdit'].includes(tool)
    ? t('workspace.asked.alwaysEdit')
    : tool === 'Bash'
      ? t('workspace.asked.alwaysCommand')
      : t('workspace.asked.alwaysTool', { tool })

/** What a question can be answered with: once either way, or always in this chat. */
export type Said = 'allow' | 'deny' | 'always'

export function Asked({
  questions,
  onAnswer,
}: {
  questions: readonly Question[]
  onAnswer: (id: string, said: Said, chosen?: Chosen, reason?: string) => void
}): React.JSX.Element {
  return (
    <>
      {questions.map((question) => {
        const asked = questionsOf(question)
        return asked ? (
          <Choosing key={question.id} asked={asked} onPick={(chosen) => onAnswer(question.id, 'allow', chosen)} onRefuse={() => onAnswer(question.id, 'deny')} />
        ) : (
          <Allowing
            key={question.id}
            question={question}
            onAnswer={(said, reason, plan) =>
              plan ? onAnswer(question.id, said, [[plan]]) : reason?.trim() ? onAnswer(question.id, said, undefined, reason.trim()) : onAnswer(question.id, said)
            }
          />
        )
      })}
    </>
  )
}

/* A request: what it is about on its first line, the tool's JSON folded away.
   A refusal can say why, and the agent reads it in place of the tool's result.
   A plan is approved as written or as edited here, or refused. */
function Allowing({ question, onAnswer }: { question: Question; onAnswer: (said: Said, reason?: string, plan?: string) => void }): React.JSX.Element {
  const card = useFocused<HTMLElement>()
  const [why, setWhy] = useState('')
  const plan = question.touch?.kind === 'plan' ? question.touch.plan : null
  const [editing, setEditing] = useState<string | null>(null)
  const edited = plan !== null && editing !== null && editing.trim() !== plan.trim() ? editing : undefined
  const what = plan ? '' : subjectOf(question)
  const grants = grantsOf(question.input)
  const lines = what.split('\n').length
  return (
    <section
      className="asking"
      ref={card}
      tabIndex={-1}
      aria-label={plan ? t('workspace.asked.plan') : undefined}
      onKeyDown={(event) => {
        if (composing(event)) return
        const said = allowKey(event.key, event.target === event.currentTarget)
        if (!said) return
        event.preventDefault()
        onAnswer(said, said === 'deny' ? why : undefined, said === 'allow' ? edited : undefined)
      }}
    >
      <div className="asking__t">
        {plan ? (
          <>
            {tRich('workspace.asked.planAsk', { plan: <b>{t('workspace.asked.plan')}</b> })}
          </>
        ) : (
          <>
            <b>{question.tool}</b>
            {lines > 1 && <span className="asking__lines">{` ${t('workspace.asked.lines', { count: lines })}`}</span>}
            {what ? <code className="asking__what">{` ${what}`}</code> : t('workspace.asked.allowThis')}
          </>
        )}
      </div>
      {grants.length > 0 && (
        <ul className="asking__grants" aria-label={t('workspace.asked.alsoAsks')}>
          {grants.map((one) => (
            <li key={one}>{one}</li>
          ))}
        </ul>
      )}
      {plan &&
        (editing === null ? (
          <div className="asking__plan">
            <Markdown source={plan} />
          </div>
        ) : (
          <textarea className="asking__edit" aria-label={t('workspace.asked.planEdit')} value={editing} onChange={(event) => setEditing(event.target.value)} />
        ))}
      <details className="asking__more">
        <summary>{t('workspace.asked.details')}</summary>
        <pre className="asking__in">{question.input}</pre>
      </details>
      <div className="asking__row">
        <input
          className="asking__why"
          value={why}
          placeholder={plan ? t('workspace.asked.changePlaceholder') : t('workspace.asked.whyPlaceholder')}
          aria-label={t('workspace.asked.reasonLabel')}
          onChange={(event) => setWhy(event.target.value)}
          onKeyDown={(event) => {
            if (!committed(event) || !why.trim()) return
            event.preventDefault()
            onAnswer('deny', why)
          }}
        />
        <button className="btn" onClick={() => onAnswer('deny', why)}>
          {t('workspace.asked.refuse')}
        </button>
        {plan && editing === null && (
          <button className="btn" onClick={() => setEditing(plan)}>
            {t('workspace.asked.edit')}
          </button>
        )}
        {!plan && grants.length === 0 && alwaysOffered(question.input) && (
          <button className="btn" onClick={() => onAnswer('always')} title={alwaysMeans(question.tool)}>
            {t('workspace.asked.always')}
          </button>
        )}
        <button className="btn btn--go" onClick={() => onAnswer('allow', undefined, edited)}>
          {plan ? t('workspace.asked.approve') : t('workspace.asked.allow')}
        </button>
      </div>
    </section>
  )
}
