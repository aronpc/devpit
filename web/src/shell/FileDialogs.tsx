import { AskName } from './AskName'
import { Confirm } from './Confirm'
import type { FileActions } from './useFileActions'
import { t } from '../i18n'

/* The three dialogs the file actions raise. Given the whole `FileActions`
   rather than eight props: they are one thing, and a caller that has the hook
   has all of it already. */
export function FileDialogs({ actions }: { actions: FileActions }): React.JSX.Element | null {
  const { naming, renaming, deleting, close, create, rename, destroy } = actions

  if (naming) {
    return (
      <AskName
        title={naming.folder ? t('workspace.fileDialogs.newFolder') : t('workspace.fileDialogs.newFile')}
        placeholder={naming.folder ? t('workspace.fileDialogs.folderExample') : t('workspace.fileDialogs.fileExample')}
        action={t('workspace.fileDialogs.create')}
        onClose={close}
        onName={create}
      />
    )
  }

  if (renaming) {
    const name = renaming.split('/').pop() ?? ''
    return (
      <AskName
        title={t('workspace.fileDialogs.rename', { name })}
        placeholder={name}
        action={t('workspace.fileDialogs.renameAction')}
        onClose={close}
        onName={rename}
      />
    )
  }

  if (deleting) {
    return (
      <Confirm
        title={t('workspace.fileDialogs.delete', { name: deleting })}
        danger={t('workspace.fileDialogs.deleteAction')}
        body={
          <>
            {t('workspace.fileDialogs.deleteBody')} <b>{t('workspace.fileDialogs.deleteGone')}</b>
          </>
        }
        onClose={close}
        onConfirm={destroy}
      />
    )
  }

  return null
}
