import { useState } from 'react'

import type { DevflowConfig, DevflowPreview, DevflowScreen } from '../gen/bindings'
import { t } from '../i18n'
import { DevflowChecks } from './DevflowChecks'
import { applied, customSet, YAML_FILE, type Edit } from './devflowEdit'
import { DevflowJev } from './DevflowJev'
import { DevflowReviewers } from './DevflowReviewers'
import { DevflowStages } from './DevflowStages'
import { ask, commands } from './live'

/*
 * A Custom setup in four steps, each starting from what was detected; then
 * the YAML it writes, with its comments; then Create. Nothing is written
 * before Create, and devpit commits nothing after it.
 */

/* Step ids: the catalog says their names (`devflow.custom.step.<id>`) and what each decides, once above it. */
const STEPS = ['stages', 'checks', 'reviewers', 'jev'] as const

function Steps({ step, onStep }: { step: number; onStep: (at: number) => void }): React.JSX.Element {
  return (
    <ol className="dfsteps" aria-label={t('devflow.custom.stepsLabel')}>
      {STEPS.map((name, at) => (
        <li key={name} className="dfsteps__i" data-state={at === step ? 'now' : at < step ? 'done' : 'next'}>
          <button className="dfsteps__b" aria-current={at === step ? 'step' : undefined} onClick={() => onStep(at)}>
            <span className="dfsteps__n">{at + 1}</span>
            <span className="dfsteps__name">{t(`devflow.custom.step.${name}`)}</span>
          </button>
        </li>
      ))}
    </ol>
  )
}

function Nav({ step, back, children }: { step: string; back: React.ReactNode; children: React.ReactNode }): React.JSX.Element {
  return (
    <div className="dfnav">
      {back}
      <span className="dfnav__at">{step}</span>
      {children}
    </div>
  )
}

export function DevflowCustom({
  projectId,
  start,
  onCreated,
  onCancel,
}: {
  projectId: string
  start: DevflowConfig
  onCreated: (screen: DevflowScreen) => void
  onCancel: () => void
}): React.JSX.Element {
  const [draft, setDraft] = useState(start)
  const [step, setStep] = useState(0)
  const [preview, setPreview] = useState<DevflowPreview | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const edit: Edit = (path, value) => setDraft((was) => applied(was, path, value))
  const set = (): string => JSON.stringify(customSet(draft))

  const previewed = (): void =>
    void ask(() => commands.devflowPreview(projectId, 'custom', set())).then((answer) => {
      setPreview(answer.data)
      setProblem(answer.error)
    })
  const create = (): void =>
    void ask(() => commands.devflowCreate(projectId, 'custom', set())).then((answer) => {
      if (answer.data) onCreated(answer.data)
      setProblem(answer.error)
    })

  if (preview) {
    return (
      <>
        <div className="dfstephead">
          <h2 className="dfstephead__t">{t('devflow.custom.previewTitle')}</h2>
          <p className="dfstephead__d">{t('devflow.custom.previewBody')}</p>
        </div>
        <div className="dffile">
          <div className="dffile__head">{YAML_FILE}</div>
          <pre className="dffile__body" aria-label={t('devflow.custom.previewLabel')}>
            {preview.yaml}
          </pre>
        </div>
        <ul className="dffiles" aria-label={t('devflow.custom.filesLabel')}>
          {preview.files.map((file) => (
            <li key={file.path} className="dffiles__i">
              <span className="dffiles__change" data-change={file.change}>
                {file.change}
              </span>
              <code>{file.path}</code>
            </li>
          ))}
        </ul>
        {problem && <p className="pref__d voice__bad">{problem}</p>}
        <Nav
          step={t('devflow.custom.ready')}
          back={
            <button className="btn" onClick={() => setPreview(null)}>
              {t('devflow.back')}
            </button>
          }
        >
          <button className="btn btn--go" onClick={create}>
            {t('devflow.custom.create')}
          </button>
        </Nav>
      </>
    )
  }

  const execution = draft.stages.find((stage) => stage.name === 'execution')
  return (
    <>
      <Steps step={step} onStep={setStep} />
      <div className="dfstephead">
        <h2 className="dfstephead__t">{t(`devflow.custom.step.${STEPS[step]}`)}</h2>
        <p className="dfstephead__d">{t(`devflow.custom.says.${STEPS[step]}`)}</p>
      </div>
      {step === 0 && <DevflowStages config={draft} edit={edit} withChecks={false} />}
      {step === 1 && execution && (
        <div className="dfblock">
          <DevflowChecks checks={execution.checks} edit={edit} />
        </div>
      )}
      {step === 2 && <DevflowReviewers config={draft} edit={edit} bare />}
      {step === 3 && <DevflowJev config={draft} edit={edit} />}
      {problem && <p className="pref__d voice__bad">{problem}</p>}
      <Nav
        step={t('devflow.custom.stepOf', { step: step + 1, total: STEPS.length })}
        back={
          <button className="btn" onClick={step === 0 ? onCancel : () => setStep(step - 1)}>
            {step === 0 ? t('devflow.cancel') : t('devflow.back')}
          </button>
        }
      >
        {step < STEPS.length - 1 ? (
          <button className="btn btn--go" onClick={() => setStep(step + 1)}>
            {t('devflow.custom.next')}
          </button>
        ) : (
          <button className="btn btn--go" onClick={previewed}>
            {t('devflow.custom.preview')}
          </button>
        )}
      </Nav>
    </>
  )
}
