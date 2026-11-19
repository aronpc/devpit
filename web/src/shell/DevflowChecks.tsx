import { useState } from 'react'

import type { DevflowCheck } from '../gen/bindings'
import { t } from '../i18n'
import { checkOf, REPORTS, type Edit } from './devflowEdit'

/*
 * The execution stage's checks: what devpit runs for a card's evidence, or,
 * under `release`, once for a release. A
 * field is sent when it is left, not on every key, so the file is written
 * once per change rather than once per letter.
 */

function CheckRow({ check, onSave, onRemove }: { check: DevflowCheck; onSave: (next: DevflowCheck) => void; onRemove: () => void }): React.JSX.Element {
  const [name, setName] = useState(check.name)
  const [run, setRun] = useState(check.run)
  const leave = (): void => {
    if (name.trim() !== check.name || run.trim() !== check.run) onSave({ ...check, name: name.trim(), run: run.trim() })
  }
  return (
    <div className="dfcheck">
      <input className="dfinput" aria-label={t('devflow.checks.nameLabel')} value={name} onChange={(event) => setName(event.target.value)} onBlur={leave} />
      <span className="dfcheck__run">
        <input className="dfinput dfinput--mono" aria-label={t('devflow.checks.commandOf', { name: check.name })} value={run} onChange={(event) => setRun(event.target.value)} onBlur={leave} />
        {check.source && <span className="dfcheck__from">{t('devflow.checks.foundIn', { source: check.source })}</span>}
      </span>
      <select aria-label={t('devflow.checks.reportOf', { name: check.name })} value={check.report} onChange={(event) => onSave({ ...check, report: event.target.value })}>
          {REPORTS.map((report) => (
            <option key={report} value={report}>
              {report}
            </option>
          ))}
      </select>
      <button className="btn btn--quiet btn--sm" data-danger onClick={onRemove}>
        {t('devflow.remove')}
      </button>
    </div>
  )
}

/* The catalog key of what an empty list says, by list. */
const EMPTY = {
  checks: 'devflow.checks.empty',
  release: 'devflow.checks.emptyRelease',
}

export function DevflowChecks({
  checks,
  edit,
  list = 'checks',
}: {
  checks: readonly DevflowCheck[]
  edit: Edit
  list?: keyof typeof EMPTY
}): React.JSX.Element {
  const [name, setName] = useState('')
  const [run, setRun] = useState('')
  const release = list === 'release'
  const save = (next: readonly DevflowCheck[]): void => edit(`stages.execution.${list}`, next.map(checkOf))
  const add = (): void => {
    save([...checks, { name: name.trim(), run: run.trim(), report: 'none', evidence: [], source: null }])
    setName('')
    setRun('')
  }
  return (
    <div className="dfchecks">
      <div className="dfcheck dfcheck--head" aria-hidden>
        <span>{t('devflow.checks.headName')}</span>
        <span>{t('devflow.checks.headCommand')}</span>
        <span>{t('devflow.checks.headReport')}</span>
        <span />
      </div>
      {checks.length === 0 && <span className="dfempty">{t(EMPTY[list])}</span>}
      {checks.map((check, at) => (
        <CheckRow
          key={`${check.name}\u0000${check.run}\u0000${check.report}`}
          check={check}
          onSave={(next) => save(checks.map((one, here) => (here === at ? next : one)))}
          onRemove={() => save(checks.filter((_, here) => here !== at))}
        />
      ))}
      <div className="dfcheck dfcheck--new">
        <input className="dfinput" aria-label={t(release ? 'devflow.checks.newNameRelease' : 'devflow.checks.newName')} placeholder={t('devflow.checks.namePlaceholder')} value={name} onChange={(event) => setName(event.target.value)} />
        <input className="dfinput dfinput--mono" aria-label={t(release ? 'devflow.checks.newCommandRelease' : 'devflow.checks.newCommand')} placeholder={t('devflow.checks.commandPlaceholder')} value={run} onChange={(event) => setRun(event.target.value)} />
        <span />
        <button className="btn btn--sm" disabled={!name.trim() || !run.trim()} onClick={add}>
          {t(release ? 'devflow.checks.addRelease' : 'devflow.checks.add')}
        </button>
      </div>
    </div>
  )
}
