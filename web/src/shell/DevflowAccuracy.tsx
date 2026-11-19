import type { DevflowCalibration, RubricAccuracy } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'

/*
 * How each rubric Devflow asks Jev agrees with you in this project, learnt
 * from what you did: a card reopened after Jev called it done, a finding
 * called a false positive, a card finished. Jev is never trained; only the
 * line a rubric draws moves, and this says where it would agree most.
 */

const percent = (share: number): string => `${Math.round(share * 100)}%`

export function accuracyLine(one: RubricAccuracy): string {
  let said = t('devflow.accuracy.decisions', { rubric: one.rubric, count: one.decisions, labelled: one.labelled })
  if (one.accuracy != null) {
    said +=
      one.min == null
        ? t('devflow.accuracy.agreed', { percent: percent(one.accuracy) })
        : t('devflow.accuracy.agreedAt', { percent: percent(one.accuracy), min: one.min })
  }
  if (one.suggestedMin != null) {
    said +=
      one.suggestedGrey == null
        ? t('devflow.accuracy.tryMin', { min: one.suggestedMin })
        : t('devflow.accuracy.tryMinGrey', { min: one.suggestedMin, grey: one.suggestedGrey })
    if (one.suggestedAccuracy != null) said += t('devflow.accuracy.tryAccuracy', { percent: percent(one.suggestedAccuracy) })
  } else if (one.note) said += t('devflow.accuracy.note', { note: one.note })
  if (one.held) said += t('devflow.accuracy.held', { held: one.held })
  return `${said}.`
}

export function DevflowAccuracy({ calibration }: { calibration: DevflowCalibration }): React.JSX.Element {
  return (
    <div className="pref pref--stack">
      <div className="pref__body">
        <span className="pref__t">{t('devflow.accuracy.title')}</span>
        <span className="pref__d">{tRich('devflow.accuracy.body', {
          path: <code>.devpit/devflow/rubrics/</code>, // ui-text: ok identifier
        })}</span>
        {calibration.rubrics.map((one) => (
          <span key={one.rubric} className="pref__d">
            {accuracyLine(one)}
          </span>
        ))}
      </div>
    </div>
  )
}
