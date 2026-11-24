import type { Change } from '../gen/bindings'
import { discardLoses, type DiscardLoss } from './changes'
import { Confirm } from './Confirm'
import { t } from '../i18n'

/*
 * The confirmation text for one discard, apart from `Changes.tsx` because the
 * three losses read very differently and getting the wrong one wrong is worse
 * than saying none: "nothing is lost" next to a modified file would throw
 * away an hour of uncommitted edits while promising it would not.
 */
const body = (worst: DiscardLoss, many: boolean): React.ReactNode => {
  switch (worst) {
    case 'nothing':
      return t(many ? 'workspace.discard.nothingMany' : 'workspace.discard.nothingOne')
    case 'edits':
      return (
        <>
          <b>{t(many ? 'workspace.discard.editsLostMany' : 'workspace.discard.editsLostOne')}</b>{' '}
          {t(many ? 'workspace.discard.goBackMany' : 'workspace.discard.goBackOne')}
        </>
      )
    case 'file':
      return (
        <>
          {t(many ? 'workspace.discard.noCopyMany' : 'workspace.discard.noCopyOne')}{' '}
          <b>{t(many ? 'workspace.discard.deletesMany' : 'workspace.discard.deletesOne')}</b>
        </>
      )
  }
}

/* Several at once are described by the worst of them: a section holding one
   new file among twenty edits deletes that file, and saying only "edits are
   lost" would hide it. */
const WORST: readonly DiscardLoss[] = ['file', 'edits', 'nothing']

const loss = (changes: readonly Change[]): DiscardLoss =>
  WORST.find((one) => changes.some((change) => discardLoses(change.status) === one)) ?? 'nothing'

export function DiscardConfirm({
  changes,
  onClose,
  onConfirm,
}: {
  changes: readonly Change[]
  onClose: () => void
  onConfirm: () => void
}): React.JSX.Element {
  const one = changes.length === 1 ? changes[0] : undefined
  return (
    <Confirm
      title={one ? t('workspace.discard.titleOne', { path: one.path }) : t('workspace.discard.titleMany', { count: changes.length })}
      danger={t('workspace.discard.action')}
      body={body(loss(changes), one === undefined)}
      onClose={onClose}
      onConfirm={onConfirm}
    />
  )
}
