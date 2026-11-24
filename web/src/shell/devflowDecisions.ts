import type { DevflowDecisions } from '../gen/bindings'
import { t } from '../i18n'

/*
 * Where Decisions stands for a project's Devflow, said the same on the Jev
 * panel and on the card as in the replies the agent reads.
 */

export function decisionsLine(decisions: DevflowDecisions): string {
  const why = decisions.reason ?? ''
  switch (decisions.state) {
    case 'on':
      return t('devflow.decisions.on')
    case 'off':
      return why ? t('devflow.decisions.offBecause', { why }) : t('devflow.decisions.off')
    case 'no_key':
      return why || t('devflow.decisions.noKey')
    case 'failing':
      return t('devflow.decisions.failing', { why: why || t('devflow.decisions.noReason') })
  }
}

/** The card's colour for it: a key missing warns, a failing call is red. */
export function decisionsTone(decisions: DevflowDecisions): 'ok' | 'lost' | 'failed' {
  if (decisions.state === 'failing') return 'failed'
  return decisions.state === 'no_key' ? 'lost' : 'ok'
}
