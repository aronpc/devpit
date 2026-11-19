import { Children, useId, useState } from 'react'

import type { DevflowConfig, DevflowStage } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { DevflowChecks } from './DevflowChecks'
import { added, moved, PUBLISH, removed, type Edit } from './devflowEdit'
import { ask, commands } from './live'

/*
 * The stages, in the order the work reaches them, each with its switch and
 * what it holds: who approves, where the plan goes, the checks, the gate,
 * and the skill it follows — read where it is, from the layer that has it.
 * An editor of a file that exists can also move, remove and add stages: the
 * project's own and its teams', beside devpit's.
 */

const named = (stage: DevflowStage): string => (stage.own ? stage.uses : stage.name[0]!.toUpperCase() + stage.name.slice(1))

function SkillLine({ stage, projectId }: { stage: DevflowStage; projectId?: string }): React.JSX.Element | null {
  const [text, setText] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const skill = stage.skill
  if (!skill) return null
  const where = skill.refused ?? (skill.file ? `${skill.layer} · ${skill.file}` : skill.layer)
  /* An own stage's skill is read by the stage tool; the built-in ones here. */
  const viewable = Boolean(projectId) && !skill.refused && !stage.own
  const view = (): void => {
    if (text !== null || !projectId) return setText(null)
    void ask(() => commands.devflowSkillText(projectId, skill.name)).then((answer) => {
      setText(answer.data?.text ?? null)
      setProblem(answer.error)
    })
  }
  return (
    <>
      <div className="dffield">
        <span className="dffield__l">{t('devflow.stages.skill')}</span>
        <span className="dffield__v">
          <span className={skill.refused ? 'dfskill voice__bad' : 'dfskill'} title={where}>
            <b>{skill.name}</b>
            <span>{where}</span>
          </span>
          {viewable && (
            <button className="btn btn--quiet btn--sm" aria-expanded={text !== null} onClick={view}>
              {text === null ? t('devflow.stages.viewSkill') : t('devflow.stages.hide')}
            </button>
          )}
        </span>
      </div>
      {problem && <span className="pref__d voice__bad">{problem}</span>}
      {text !== null && <pre className="errprev__body">{text}</pre>}
    </>
  )
}

function GateRow({ gate, edit }: { gate: string; edit: Edit }): React.JSX.Element {
  const [rubric, setRubric] = useState(gate)
  return (
    <label className="dffield">
      <span className="dffield__l">{t('devflow.stages.gate')}</span>
      <input
        className="dfinput"
        aria-label={t('devflow.stages.gateRubric')}
        value={rubric}
        onChange={(event) => setRubric(event.target.value)}
        onBlur={() => rubric.trim() !== gate && edit('stages.completion.gate', rubric.trim())}
      />
    </label>
  )
}

function Arrange({ config, stage, edit }: { config: DevflowConfig; stage: DevflowStage; edit: Edit }): React.JSX.Element {
  const at = config.stages.findIndex((one) => one.name === stage.name)
  return (
    <div className="dfstage__tools">
      <button className="btn btn--quiet btn--sm" aria-label={t('devflow.stages.moveUp', { stage: stage.name })} disabled={at === 0} onClick={() => edit('stages', moved(config, stage.name, -1))}>
        {t('devflow.stages.up')}
      </button>
      <button className="btn btn--quiet btn--sm" aria-label={t('devflow.stages.moveDown', { stage: stage.name })} disabled={at === config.stages.length - 1} onClick={() => edit('stages', moved(config, stage.name, 1))}>
        {t('devflow.stages.down')}
      </button>
      <button className="btn btn--quiet btn--sm" data-danger aria-label={t('devflow.stages.removeStage', { stage: stage.name })} onClick={() => edit('stages', removed(config, stage.name))}>
        {t('devflow.remove')}
      </button>
    </div>
  )
}

function AddStage({ config, edit }: { config: DevflowConfig; edit: Edit }): React.JSX.Element {
  const [uses, setUses] = useState('')
  const [command, setCommand] = useState('')
  const add = (): void => {
    edit('stages', added(config, uses, command))
    setUses('')
    setCommand('')
  }
  return (
    <div className="dfstage dfstage--add">
      <span className="dfstage__n" aria-hidden>
        {'+'}
      </span>
      <div className="dfstage__card">
        <span className="dfstage__t">{t('devflow.stages.addTitle')}</span>
        <span className="dfstage__d">
          {tRich('devflow.stages.addBody', {
            project: <code>project/&lt;skill&gt;</code>, // ui-text: ok identifier
            personal: <code>personal/&lt;skill&gt;</code>, // ui-text: ok identifier
            skills: <code>skills/&lt;name&gt;</code>, // ui-text: ok identifier
            team: <code>&lt;name&gt;/&lt;skill&gt;</code>, // ui-text: ok identifier
          })}
        </span>
        <div className="dfadd">
          <input className="dfinput dfinput--short" aria-label={t('devflow.stages.usesLabel')} placeholder={EXAMPLE_USES} value={uses} onChange={(event) => setUses(event.target.value)} />
          <input className="dfinput" aria-label={t('devflow.stages.commandLabel')} placeholder={t('devflow.stages.commandPlaceholder')} value={command} onChange={(event) => setCommand(event.target.value)} />
          <button className="btn" disabled={!/^[a-z0-9][a-z0-9_-]*\/[a-z0-9][a-z0-9_-]*$/.test(uses.trim())} onClick={add}>
            {t('devflow.stages.add')}
          </button>
        </div>
      </div>
    </div>
  )
}

/** One stage as a card on the flow's line: its place, what it is, its switch,
 *  and what it holds. Only the switch switches. */
function StageCard({
  at,
  on,
  onFlip,
  title,
  said,
  children,
}: {
  at: number
  on: boolean
  onFlip: () => void
  title: string
  said: string
  children?: React.ReactNode
}): React.JSX.Element {
  const id = useId()
  const holds = Children.toArray(children).length > 0
  return (
    <section className="dfstage" data-off={on ? undefined : ''} aria-labelledby={`${id}t`}>
      <span className="dfstage__n" aria-hidden>
        {at + 1}
      </span>
      <div className="dfstage__card">
        <div className="dfstage__head">
          <div className="dfstage__what">
            <span className="dfstage__t" id={`${id}t`}>
              {title}
            </span>
            <span className="dfstage__d" id={`${id}d`}>
              {said}
            </span>
          </div>
          <span className="dfstage__state">{t(on ? 'devflow.on' : 'devflow.off')}</span>
          <button className="prefsw" role="switch" aria-checked={on} aria-labelledby={`${id}t`} aria-describedby={`${id}d`} onClick={onFlip}>
            <span className="sw"></span>
          </button>
        </div>
        {holds && <div className="dfstage__body">{children}</div>}
      </div>
    </section>
  )
}

const EXAMPLE_USES = 'project/pr' // ui-text: ok identifier

/* The built-in stage ids; the catalog says what each is (`devflow.stages.says.<id>`). */
const BUILT_IN = ['briefing', 'context', 'plan', 'execution', 'review', 'completion']

/** What a stage says it is: an own stage by what gates it, a built-in one by its own sentence. */
function stageSaid(stage: DevflowStage): string {
  if (stage.own) {
    if (stage.command) return t('devflow.stages.own.command', { command: stage.command })
    return stage.gate ? t('devflow.stages.own.gate', { gate: stage.gate }) : t('devflow.stages.own.skill')
  }
  return BUILT_IN.includes(stage.name) ? t(`devflow.stages.says.${stage.name}`) : ''
}

export function DevflowStages({
  config,
  edit,
  projectId,
  withChecks = true,
  arrange = false,
}: {
  config: DevflowConfig
  edit: Edit
  projectId?: string
  /** A setup asks for the checks on a step of their own. */
  withChecks?: boolean
  /** Stages moved, removed and added: an editor of a file that exists. */
  arrange?: boolean
}): React.JSX.Element {
  return (
    <div className="dfstages">
      {config.stages.map((stage, at) => (
        <StageCard
          key={stage.uses}
          at={at}
          on={stage.enabled}
          onFlip={() => edit(`stages.${stage.name}.enabled`, !stage.enabled)}
          title={named(stage)}
          said={stageSaid(stage)}
        >
          {stage.approve !== null && (
            <label className="dffield">
              <span className="dffield__l">{t('devflow.stages.approval')}</span>
              <select aria-label={t('devflow.stages.approveLabel', { stage: stage.name })} value={stage.approve} onChange={(event) => edit(`stages.${stage.name}.approve`, event.target.value)}>
                <option value="person">{t('devflow.stages.approvePerson')}</option>
                <option value="none">{t('devflow.stages.approveNone')}</option>
              </select>
            </label>
          )}
          {stage.uses === 'builtin/plan' && (
            <label className="dffield">
              <span className="dffield__l">{t('devflow.stages.cardsIn')}</span>
              <select aria-label={t('devflow.stages.cardsInLabel')} value={stage.publish ?? 'board'} onChange={(event) => edit('stages.plan.publish', event.target.value)}>
                {PUBLISH.map((where) => (
                  <option key={where} value={where}>
                    {where === 'board' ? t('devflow.stages.publishBoard') : t('devflow.stages.publishOther', { where })}
                  </option>
                ))}
              </select>
            </label>
          )}
          {stage.uses === 'builtin/plan' && (
            <label className="dffield">
              <span className="dffield__l">{t('devflow.stages.nextCards')}</span>
              <select aria-label={t('devflow.stages.nextCardsLabel')} value={stage.unblockOn ?? 'done'} onChange={(event) => edit('stages.plan.unblock_on', event.target.value)}>
                <option value="done">{t('devflow.stages.unblockDone')}</option>
                <option value="gate">{t('devflow.stages.unblockGate')}</option>
              </select>
            </label>
          )}
          {stage.uses === 'builtin/execution' && withChecks && (
            <div className="dffield dffield--top">
              <span className="dffield__l">{t('devflow.stages.checks')}</span>
              <div className="dffield__v">
                <DevflowChecks checks={stage.checks} edit={edit} />
              </div>
            </div>
          )}
          {stage.uses === 'builtin/execution' && withChecks && (
            <div className="dffield dffield--top">
              <span className="dffield__l">{t('devflow.stages.releaseChecks')}</span>
              <div className="dffield__v">
                <DevflowChecks checks={stage.release ?? []} edit={edit} list="release" />
              </div>
            </div>
          )}
          {stage.uses === 'builtin/completion' && stage.gate !== null && <GateRow key={stage.gate} gate={stage.gate} edit={edit} />}
          <SkillLine stage={stage} projectId={projectId} />
          {arrange && <Arrange config={config} stage={stage} edit={edit} />}
        </StageCard>
      ))}
      {arrange && <AddStage config={config} edit={edit} />}
    </div>
  )
}
