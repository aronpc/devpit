import { useState } from 'react'

import type { DevflowConfig, DevflowReviewer } from '../gen/bindings'
import { t } from '../i18n'
import { AgentMark } from './AgentMark'
import { CONSOLIDATIONS, PERSONAS, REVIEWER_AGENTS, reviewerOf, SEVERITIES, type Edit } from './devflowEdit'
import { useKnownAgents } from './useKnownAgents'

/*
 * Who reviews a card's change: each reviewer a persona on one of the
 * person's own agent CLIs, the findings merged as `consolidate` says, and
 * the severities that hold the card.
 */

function ModelField({ model, onSave }: { model: string | null; onSave: (model: string | null) => void }): React.JSX.Element {
  const [said, setSaid] = useState(model ?? '')
  return (
    <input
      className="dfinput"
      aria-label={t('devflow.reviewers.model')}
      placeholder={t('devflow.reviewers.modelPlaceholder')}
      value={said}
      onChange={(event) => setSaid(event.target.value)}
      onBlur={() => (said.trim() || null) !== model && onSave(said.trim() || null)}
    />
  )
}

export function DevflowReviewers({ config, edit, bare = false }: { config: DevflowConfig; edit: Edit; /** Under a heading that already says it. */ bare?: boolean }): React.JSX.Element {
  const known = useKnownAgents()
  /* A CLI not installed here can still be named: the review skips it and says so. */
  const installed = (agent: string): boolean => known.some((one) => one.id === agent && one.installed)
  const save = (next: readonly DevflowReviewer[]): void => edit('review.reviewers', next.map(reviewerOf))
  const change = (at: number, one: Partial<DevflowReviewer>): void =>
    save(config.reviewers.map((was, here) => (here === at ? { ...was, ...one } : was)))
  const firstInstalled = REVIEWER_AGENTS.find(installed) ?? 'claude'
  const blocking = new Set(config.blockOn)

  return (
    <div className="dfblock">
      {!bare && (
        <div className="dfblock__head">
          <span className="dfblock__t">{t('devflow.reviewers.title')}</span>
          <span className="dfblock__d">{t('devflow.reviewers.body')}</span>
        </div>
      )}
      <div className="dfrevs">
        {config.reviewers.length > 0 && (
          <div className="dfrev dfrev--head" aria-hidden>
            <span>{t('devflow.reviewers.headAgent')}</span>
            <span>{t('devflow.reviewers.headPersona')}</span>
            <span>{t('devflow.reviewers.headModel')}</span>
            <span />
          </div>
        )}
        {config.reviewers.length === 0 && <span className="dfempty">{t('devflow.reviewers.none')}</span>}
        {config.reviewers.map((one, at) => (
          <div className="dfrev" key={`${at}:${one.agent}:${one.persona}:${one.model ?? ''}`}>
            <span className="dfrev__agent">
              <AgentMark agent={one.agent} />
              <select aria-label={t('devflow.reviewers.agentLabel')} value={one.agent} onChange={(event) => change(at, { agent: event.target.value })}>
                {REVIEWER_AGENTS.map((agent) => (
                  <option key={agent} value={agent}>
                    {agent}
                    {installed(agent) ? '' : t('devflow.reviewers.notInstalled')}
                  </option>
                ))}
              </select>
            </span>
            <select aria-label={t('devflow.reviewers.personaLabel')} value={one.persona} onChange={(event) => change(at, { persona: event.target.value })}>
              {[...new Set([...PERSONAS, one.persona])].map((persona) => (
                <option key={persona} value={persona}>
                  {persona}
                </option>
              ))}
            </select>
            <ModelField model={one.model} onSave={(model) => change(at, { model })} />
            <button className="btn btn--quiet btn--sm" data-danger onClick={() => save(config.reviewers.filter((_, here) => here !== at))}>
              {t('devflow.remove')}
            </button>
          </div>
        ))}
        <div className="dfrevs__add">
          <button className="btn btn--sm" onClick={() => save([...config.reviewers, { agent: firstInstalled, persona: 'devflow-security', model: null }])}>
            {t('devflow.reviewers.add')}
          </button>
        </div>
      </div>
      <div className="dfblock__fields">
        <label className="dffield">
          <span className="dffield__l">{t('devflow.reviewers.mergedBy')}</span>
          <select aria-label={t('devflow.reviewers.mergeLabel')} value={config.consolidate} onChange={(event) => edit('review.consolidate', event.target.value)}>
            {CONSOLIDATIONS.map((how) => (
              <option key={how} value={how}>
                {t(`devflow.reviewers.merge.${how}`)}
              </option>
            ))}
          </select>
        </label>
        <div className="dffield">
          <span className="dffield__l">{t('devflow.reviewers.holdsOn')}</span>
          <div className="dfseg" role="group" aria-label={t('devflow.reviewers.severitiesLabel')}>
            {SEVERITIES.map((severity) => (
              <button
                key={severity}
                className="dfseg__b"
                data-severity={severity}
                aria-pressed={blocking.has(severity)}
                data-on={blocking.has(severity) ? '' : undefined}
                onClick={() => edit('review.block_on', SEVERITIES.filter((one) => (one === severity ? !blocking.has(one) : blocking.has(one))))}
              >
                {severity}
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
