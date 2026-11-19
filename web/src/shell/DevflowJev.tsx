import { useEffect, useState } from 'react'

import type { DevflowCalibration, DevflowConfig } from '../gen/bindings'
import { accuracyLine } from './DevflowAccuracy'
import { decisionsLine } from './devflowDecisions'
import { rubricsWith, type Edit } from './devflowEdit'
import { t } from '../i18n'
import { ask, commands } from './live'
import { PrefSwitch } from './PrefSwitch'
import { useShell } from './useShell'

/*
 * Jev — Decisions — over Devflow's gates: whether it is asked, whether it
 * only records (shadow) or holds (enforce, which is the person's click), the
 * rubric a stage is judged by, and how often it agreed with the person here.
 * The key and the provider live in Settings → Decisions, never here.
 */

function RubricField({ stage, rubric, onSave }: { stage: string; rubric: string | null; onSave: (rubric: string) => void }): React.JSX.Element {
  const [said, setSaid] = useState(rubric ?? '')
  return (
    <label className="dffield">
      <span className="dffield__l">{stage[0]!.toUpperCase() + stage.slice(1)}</span>
      <input
        className="dfinput"
        aria-label={t('devflow.jev.rubricOf', { stage })}
        placeholder={t('devflow.jev.rubricPlaceholder')}
        value={said}
        onChange={(event) => setSaid(event.target.value)}
        onBlur={() => said.trim() !== (rubric ?? '') && onSave(said)}
      />
    </label>
  )
}

export function DevflowJev({
  config,
  edit,
  projectId,
  onEnforce,
}: {
  config: DevflowConfig
  edit: Edit
  projectId?: string
  /** Asks the person's click to enforce; absent before the file exists. */
  onEnforce?: () => void
}): React.JSX.Element {
  const { openPrefs } = useShell()
  const [accuracy, setAccuracy] = useState<DevflowCalibration | null>(null)
  useEffect(() => {
    if (!projectId) return
    void ask(() => commands.devflowCalibration(projectId)).then((answer) => setAccuracy(answer.data))
  }, [projectId])
  const enforcing = config.decisionsMode === 'enforce'

  return (
    <>
      <PrefSwitch
        on={config.decisionsUse}
        onFlip={() => edit('decisions.use', !config.decisionsUse)}
        title={t('devflow.jev.title')}
        said={t('devflow.jev.said')}
      >
        <div className="dffield">
          <span className="dffield__l">{t('devflow.jev.mode')}</span>
          <span className="dffield__v">
            <span className="dfmode" data-mode={enforcing ? 'enforce' : 'shadow'}>
              <b>{t(enforcing ? 'devflow.jev.enforce' : 'devflow.jev.shadow')}</b>
              {t(enforcing ? 'devflow.jev.enforceSaid' : 'devflow.jev.shadowSaid')}
            </span>
            {onEnforce && !enforcing && (
              <button className="btn btn--sm" onClick={onEnforce}>
                {t('devflow.jev.enforceButton')}
              </button>
            )}
            {onEnforce && enforcing && (
              <button className="btn btn--sm" onClick={() => edit('decisions.mode', 'shadow')}>
                {t('devflow.jev.backToShadow')}
              </button>
            )}
          </span>
        </div>
        {accuracy?.decisions && (
          <div className="dffield">
            <span className="dffield__l">{t('devflow.jev.state')}</span>
            <span className="dffield__v" data-decisions={accuracy.decisions.state}>
              {decisionsLine(accuracy.decisions)}
            </span>
          </div>
        )}
        {!onEnforce && <span className="pref__d">{t('devflow.jev.startsInShadow')}</span>}
        {projectId && <span className="dfsubhead">{t('devflow.jev.rubricPerStage')}</span>}
        {projectId &&
          config.stages.map((stage) => (
            <RubricField
              key={`${stage.name}:${stage.rubric ?? ''}`}
              stage={stage.name}
              rubric={stage.rubric}
              onSave={(rubric) => edit('decisions.rubrics', rubricsWith(config, stage.name, rubric))}
            />
          ))}
        {accuracy?.rubrics.map((one) => (
          <span key={one.rubric} className="pref__d">
            {accuracyLine(one)}
          </span>
        ))}
        {projectId && accuracy && accuracy.rubrics.length === 0 && <span className="pref__d">{t('devflow.jev.noAccuracy')}</span>}
        <div className="dfrow-end">
          <button className="btn btn--quiet btn--sm" onClick={() => openPrefs('decisions')}>
            {t('devflow.jev.keyAndProvider')}
          </button>
        </div>
      </PrefSwitch>
    </>
  )
}
