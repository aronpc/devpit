import type { Installation, OutsideSession, Profile } from '../gen/bindings'
import { t } from '../i18n'

/*
 * Which profile opens a session started outside devpit.
 *
 * A session belongs to the installation that wrote it, and resuming it under
 * another installation's account would start somewhere with none of its
 * history. So the profile has to run against that same configuration
 * directory: one of the profiles the installation lists by name, or — for the
 * installation no profile names — the CLI devpit found on its own.
 */

export function profileFor(
  session: OutsideSession,
  installations: readonly Installation[],
  profiles: readonly Profile[],
): Profile | null {
  return profileForInstallation(session.installation, installations, profiles)
}

/* The same rule, for a session known only by the installation folder it is in. */
export function profileForInstallation(
  directory: string,
  installations: readonly Installation[],
  profiles: readonly Profile[],
): Profile | null {
  const installation = installations.find((one) => one.directory === directory)
  if (!installation) return null
  const runnable = profiles.filter((profile) => profile.reach === 'runnable' && profile.driver === 'claude')
  if (installation.profiles.length > 0) {
    return runnable.find((profile) => runsAgainst(profile, installation)) ?? null
  }
  // No profile names this installation: it is the one a plain `claude` uses.
  return runnable.find((profile) => !profile.mine) ?? null
}

/* By id when the answer carries ids; by name for one that predates them. */
const runsAgainst = (profile: Profile, installation: Installation): boolean =>
  installation.ids ? installation.ids.includes(profile.id) : installation.profiles.includes(profile.label)

/*
 * The installation a profile runs against: where its sign-in, its history,
 * its skills and its MCP servers are.
 *
 * A profile devpit only discovered carries no environment, so it runs where
 * this process would — the first installation, which is always that one.
 */
export function installationOf(
  profile: Profile | undefined,
  installations: readonly Installation[],
): Installation | null {
  if (!profile || profile.driver !== 'claude') return null
  const named = installations.find((one) => runsAgainst(profile, one))
  if (named) return named
  return profile.mine ? null : (installations[0] ?? null)
}

/*
 * The account a chat is on, as the earlier-conversation lists narrow to it.
 * `null` when nothing is chosen yet, and the lists are not narrowed.
 */
export interface Scope {
  readonly profileId: string
  /** Its installation, or null when it has none on this machine yet. */
  readonly directory: string | null
}

export function scopeOf(
  profileId: string | null,
  profiles: readonly Profile[],
  installations: readonly Installation[],
): Scope | null {
  if (!profileId) return null
  const profile = profiles.find((one) => one.id === profileId)
  return { profileId, directory: installationOf(profile, installations)?.directory ?? null }
}

/* Whether one of devpit's own conversations is on this account: the same
   profile, or another on the same installation — `glm` and `glm-fast` write
   one history. A conversation that never spoke belongs to nobody yet. */
export function threadInScope(
  profileOfThread: string,
  scope: Scope | null,
  profiles: readonly Profile[],
  installations: readonly Installation[],
): boolean {
  if (!scope || profileOfThread === '' || profileOfThread === scope.profileId) return true
  const theirs = installationOf(profiles.find((one) => one.id === profileOfThread), installations)
  return scope.directory !== null && theirs?.directory === scope.directory
}

/* Whether a terminal session was written by this account's installation. */
export const sessionInScope = (installation: string, scope: Scope | null): boolean =>
  !scope || installation === scope.directory

/* What a session is called in the list: the CLI's title, or that it has none. */
export const titled = (session: OutsideSession): string => session.title ?? t('sessions.outside.untitled')
