import type { AgentHealth, RemoteDraft, RemoteIn, RemoteQuestion, RemoteStop } from '../gen/bindings'
import { t } from '../i18n'

/* What agents and orchestrators are waiting on the person for, answered from here. */

const what = (question: RemoteQuestion): string => {
  try {
    const input = JSON.parse(question.input) as Record<string, unknown>
    const shown = input.command ?? input.file_path ?? input.url ?? input.pattern
    return typeof shown === 'string' ? `${question.tool} ${shown}` : question.tool
  } catch {
    return question.tool
  }
}

export function RemoteWaiting({
  questions,
  drafts,
  stops,
  health,
  answering,
  typing,
  send,
}: {
  questions: readonly RemoteQuestion[]
  drafts: readonly RemoteDraft[]
  stops: readonly RemoteStop[]
  health: AgentHealth | null
  answering: boolean
  typing: boolean
  send: (message: RemoteIn) => void
}): React.JSX.Element {
  return (
    <div className="rm__list">
      {questions.length === 0 && drafts.length === 0 && stops.length === 0 && <p className="rm__said">{t('remote.waiting.nothing')}</p>}
      {stops.length > 0 && (
        <div className="rm__ask">
          <span className="rm__dim">{t('remote.waiting.stopAsked', { count: stops.length })}</span>
          {stops.map(({ profile, stop }) => (
            <div key={`${profile}/${stop.name}`}>
              <code className="rm__what">{stop.name}</code>
              {stop.losing && <span className="rm__dim">{stop.losing}</span>}
            </div>
          ))}
          {typing ? (
            <div className="rm__acts">
              <button className="rm__btn" onClick={() => send({ t: 'stopsDrop', stops: [...stops] })}>
                {t('remote.waiting.keepRunning')}
              </button>
              <button
                className="rm__btn rm__btn--go"
                onClick={() => confirm(t('remote.waiting.stopNames', { names: stops.map(({ stop }) => stop.name).join(', ') })) && send({ t: 'stopsConfirm', stops: [...stops] })}
              >
                {t('remote.waiting.stop', { count: stops.length })}
              </button>
            </div>
          ) : (
            <span className="rm__dim">{t('remote.waiting.typingPerDevice')}</span>
          )}
        </div>
      )}
      {typing && drafts.length > 1 && (
        <div className="rm__acts">
          <button
            className="rm__btn rm__btn--go"
            onClick={() => confirm(t('remote.waiting.confirmDrafts', { count: drafts.length })) && drafts.forEach((draft) => send({ t: 'draftSend', profile: draft.profile, name: draft.name }))}
          >
            {t('remote.waiting.sendAll', { count: drafts.length })}
          </button>
        </div>
      )}
      {questions.map((question) => (
        <div className="rm__ask" key={question.id}>
          <span className="rm__dim">{t('remote.waiting.asksTo', { who: question.project ?? (question.from === 'chat' ? t('remote.waiting.aChat') : t('remote.waiting.aTerminal')) })}</span>
          <code className="rm__what">{what(question)}</code>
          {answering ? (
            <div className="rm__acts">
              <button className="rm__btn" onClick={() => confirm(t('remote.waiting.confirmDeny', { what: what(question) })) && send({ t: 'answer', id: question.id, allow: false })}>
                {t('remote.waiting.deny')}
              </button>
              <button className="rm__btn rm__btn--go" onClick={() => confirm(t('remote.waiting.confirmAllow', { what: what(question) })) && send({ t: 'answer', id: question.id, allow: true })}>
                {t('remote.waiting.allow')}
              </button>
            </div>
          ) : (
            <span className="rm__dim">{t('remote.waiting.answeringPerDevice')}</span>
          )}
        </div>
      ))}
      {drafts.map((draft) => (
        <div className="rm__ask" key={`${draft.profile}/${draft.name}`}>
          <span className="rm__dim">{t('remote.waiting.drafted', { name: draft.name })}</span>
          <code className="rm__what">{draft.text}</code>
          {typing ? (
            <div className="rm__acts">
              <button className="rm__btn rm__btn--go" onClick={() => confirm(t('remote.waiting.confirmDraft', { name: draft.name })) && send({ t: 'draftSend', profile: draft.profile, name: draft.name })}>
                {t('remote.waiting.send')}
              </button>
            </div>
          ) : (
            <span className="rm__dim">{t('remote.waiting.typingPerDevice')}</span>
          )}
        </div>
      ))}
      {health && (
        <div className="rm__ask">
          <span className="rm__dim">{t('remote.waiting.mcp')}</span>
          <span>{health.answering
              ? health.latencyMs === null
                ? t('remote.waiting.mcpUp')
                : t('remote.waiting.mcpUpIn', { ms: Math.round(health.latencyMs) })
              : t('remote.waiting.mcpDown', { failures: health.failures })}</span>
          {typing && (
            <div className="rm__acts">
              <button className="rm__btn" onClick={() => confirm(t('remote.waiting.confirmRestart')) && send({ t: 'agentRestart' })}>
                {t('remote.waiting.restart')}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
