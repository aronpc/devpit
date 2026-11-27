import { useId, useState } from 'react'

import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { Icon } from './Icon'

/*
 * Installing and uninstalling a capability, each behind a dialog.
 *
 * Not `Confirm`: installing loses nothing, so a red button would lie, and
 * uninstalling takes two steps and a tick box. Same `.ask` markup and styles.
 */

/** The final button names what it will do, so a ticked box cannot go unnoticed. */
export function uninstallLabel(deleteData: boolean, files: number | null): string {
  if (!deleteData) return t('workspace.capDialog.uninstall')
  return files ? t('workspace.capDialog.uninstallDeleteFiles', { count: files }) : t('workspace.capDialog.uninstallDeleteData')
}

/** What the pane says once it is gone, from the backend's own count of deleted files. */
export function uninstalledInWords(name: string, deleteData: boolean, removed: number): string {
  if (!deleteData) return t('workspace.capDialog.uninstalledKept', { name })
  return t('workspace.capDialog.uninstalledDeleted', { name, count: removed })
}

/** Null when the folder could not be read: a count nobody read is not zero. */
function filesInWords(files: number | null): string {
  if (files === null) return t('workspace.capDialog.filesUnknown')
  if (files === 0) return t('workspace.capDialog.filesNone')
  return t('workspace.capDialog.filesSome', { count: files })
}

function Ask({
  title,
  onClose,
  children,
}: {
  title: (id: string) => React.ReactNode
  onClose: () => void
  children: React.ReactNode
}): React.JSX.Element {
  const titled = useId()
  return (
    <div className="ask" data-open="true" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <div className="ask__box" role="dialog" aria-modal="true" aria-labelledby={titled}>
        {title(titled)}
        {children}
      </div>
    </div>
  )
}

export function InstallCapability({
  name,
  adds,
  onClose,
  onConfirm,
}: {
  name: string
  adds: readonly string[]
  onClose: () => void
  onConfirm: () => void
}): React.JSX.Element {
  return (
    <Ask onClose={onClose} title={(id) => <h2 className="ask__t" id={id}>{t('workspace.capDialog.installTitle', { name })}</h2>}>
      <p className="ask__d">
        {tRich('workspace.capDialog.installBody', { bold: <b>{t('workspace.capDialog.installBold')}</b> }, { name })}
      </p>
      <ul className="capcard__adds" aria-label={t('workspace.capDialog.adds')}>
        {adds.map((words) => (
          <li className="capcard__add" key={words}>
            {words}
          </li>
        ))}
      </ul>
      <div className="ask__row">
        <button className="btn" onClick={onClose}>
          {t('workspace.capDialog.cancel')}
        </button>
        <button className="btn btn--go" autoFocus onClick={onConfirm}>
          {t('workspace.capDialog.install')}
        </button>
      </div>
    </Ask>
  )
}

export function UninstallCapability({
  name,
  files,
  onClose,
  onConfirm,
}: {
  name: string
  /** How many files it holds, or null while unread or unreadable. */
  files: number | null
  onClose: () => void
  onConfirm: (deleteData: boolean) => void
}): React.JSX.Element {
  const [step, setStep] = useState<1 | 2>(1)
  /* Off every time it opens: deleting data is chosen, never inherited. */
  const [deleteData, setDeleteData] = useState(false)

  const title = (id: string): React.ReactNode => (
    <>
      <p className="ask__step">{t('workspace.capDialog.step', { step })}</p>
      <h2 className="ask__t" id={id}>
        {step === 1 ? t('workspace.capDialog.uninstallTitle', { name }) : t('workspace.capDialog.filesTitle', { name })}
      </h2>
    </>
  )

  return step === 1 ? (
    <Ask onClose={onClose} title={title}>
      <p className="ask__d">
        {tRich('workspace.capDialog.leaves', { bold: <b>{t('workspace.capDialog.leavesBold')}</b> }, { name })}
      </p>
      <div className="ask__row">
        <button className="btn" onClick={onClose}>
          {t('workspace.capDialog.cancel')}
        </button>
        <button className="btn" onClick={() => setStep(2)}>
          {t('workspace.capDialog.continue')}
        </button>
      </div>
    </Ask>
  ) : (
    <Ask onClose={onClose} title={title}>
      <p className="ask__d">{t('workspace.capDialog.unlessTick')}</p>
      <button className="ask__opt" role="checkbox" aria-checked={deleteData} onClick={() => setDeleteData((was) => !was)}>
        <span className="box">
          <Icon size="s" aria-hidden="true">
            <path d="M20 6 9 17l-5-5" />
          </Icon>
        </span>
        <span>
          <span className="ask__ot">{t('workspace.capDialog.alsoDelete', { name })}</span>
          <span className="ask__od">{filesInWords(files)} {t('workspace.capDialog.undone')}</span>
        </span>
      </button>
      <div className="ask__row">
        <button className="btn" onClick={onClose}>
          {t('workspace.capDialog.cancel')}
        </button>
        <button className="btn btn--danger" onClick={() => onConfirm(deleteData)}>
          {uninstallLabel(deleteData, files)}
        </button>
      </div>
    </Ask>
  )
}
