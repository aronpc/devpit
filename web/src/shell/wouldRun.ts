import type { WouldRun } from '../gen/bindings'
import { t } from '../i18n'

/*
 * What a check would do, in words.
 *
 * Pure, and apart from the component, because every one of these sentences is
 * a promise about what a click does. A preview that said "no timeout" when the
 * answer was "nobody set one" would be the screen telling somebody a command
 * will stop when it will not.
 */

export interface Said {
  readonly noCommand: string
  readonly noDirectory: string
  readonly timeout: string
  readonly irreversible: string
}

export function wouldRunWords(would: WouldRun): Said {
  return {
    noCommand: t('workspace.wouldRunCard.noCommand'),
    noDirectory: would.inAWorktree ? t('workspace.wouldRunCard.checkout') : t('workspace.wouldRunCard.projectFolder'),
    timeout: timeoutWords(would.timeoutSeconds),
    irreversible: t('workspace.wouldRunCard.irreversible'),
  }
}

/**
 * How long a check runs before it is stopped.
 *
 * "Nothing set one" rather than "no timeout": the second reads like a
 * decision, and this is the absence of one. A command with no timeout runs
 * until it ends or somebody stops it, and a person about to trust a check
 * should read that as the open-ended thing it is.
 */
export function timeoutWords(seconds: number | null): string {
  if (seconds === null) return t('workspace.wouldRunCard.timeoutNone')
  if (seconds < 60) return t('workspace.wouldRunCard.seconds', { count: seconds })
  const minutes = Math.round(seconds / 60)
  return t('workspace.wouldRunCard.minutes', { count: minutes })
}
