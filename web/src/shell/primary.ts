import type { Change } from '../gen/bindings'
import { t } from '../i18n'
import { formatNumber } from '../i18n/format'

/*
 * The one button at the top of the Changes panel.
 *
 * One and not three. The panel used to offer "Stage all", "Unstage all" and a
 * Commit button at the far bottom of a list that is forty rows long — so the
 * button you needed was either competing with two you did not, or off the
 * screen entirely.
 *
 * What the button says is decided by what git is holding, which is a small
 * state machine and therefore something a test can hold rather than something
 * the markup works out as it draws.
 */

export type Doing = 'stage' | 'commit' | 'nothing'

export interface Primary {
  readonly doing: Doing
  readonly label: string
  /** Said out loud when the button cannot be pressed, so a disabled button is
   *  never a mystery. */
  readonly why: string | null
  readonly disabled: boolean
  /** How many files the press would take, for the label to name. */
  readonly files: number
}

export function primary(changes: readonly Change[], message: string): Primary {
  const staged = changes.filter((change) => change.staged).length
  const loose = changes.length - staged

  if (changes.length === 0) {
    return { doing: 'nothing', label: t('workspace.changes.commit'), why: t('workspace.changes.nothingHasChanged'), disabled: true, files: 0 }
  }

  /* Staging comes first only while the index is empty, which is the step in
     front of you then. Once anything is staged the button commits, because
     staging a few files on purpose and committing only those is an ordinary
     thing to do — and the panel says how many are left out, in a row directly
     under the button, which is where that warning belongs. */
  if (staged === 0) {
    return { doing: 'stage', label: t('workspace.changes.stageAllButton'), why: null, disabled: false, files: loose }
  }

  if (message.trim().length === 0) {
    return {
      doing: 'commit',
      label: t('workspace.changes.commitCount', { count: staged }),
      why: t('workspace.changes.sayWhat'),
      disabled: true,
      files: staged,
    }
  }

  return { doing: 'commit', label: t('workspace.changes.commitCount', { count: staged }), why: null, disabled: false, files: staged }
}

/**
 * Thousands, so six thousand lines does not read as six hundred.
 *
 * In the locale the person chose, which the core resolved; a caller with
 * its own words to say names one out loud.
 */
export function counted(lines: number, locale?: string): string {
  return formatNumber(lines, {}, locale)
}
