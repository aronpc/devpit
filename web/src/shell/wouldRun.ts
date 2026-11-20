import type { WouldRun } from '../gen/bindings'

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
  readonly identity: string | null
  readonly check: string | null
}

export function wouldRunWords(would: WouldRun): Said {
  return {
    noCommand: 'Nothing on a command line — this step is an agent turn or a shell.',
    noDirectory: would.inAWorktree
      ? 'A checkout of this card’s own, made the first time something needs it.'
      : 'The project folder.',
    timeout: timeoutWords(would.timeoutSeconds),
    irreversible: 'This step is marked as having no undo. devpit asks twice before firing it.',
    identity: identityWords(would.profile, would.model),
    check: would.check,
  }
}

export function identityWords(profile: string | null, model: string | null): string | null {
  return [profile, model].filter(Boolean).join(' · ') || null
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
  if (seconds === null) return 'Nothing set a limit — it runs until it ends or you stop it.'
  if (seconds < 60) return `${seconds} seconds.`
  const minutes = Math.round(seconds / 60)
  return `${minutes} ${minutes === 1 ? 'minute' : 'minutes'}.`
}
