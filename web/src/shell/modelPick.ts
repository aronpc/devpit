import type { OfferedModel, Profile } from '../gen/bindings'
import { t } from '../i18n'
import { modelHint, modelName } from './models'
import { ACCOUNT, valueOf } from './profiles'

/*
 * What the model picker lists, apart from how it is drawn.
 *
 * The rail is the account, the list is its models, and the star is a shelf
 * across all of them. A search reaches every account: without one, the rail
 * decides — anything else would hide the model you just typed the name of.
 */

export interface Row {
  readonly profile: Profile
  readonly model: string
}

/** The rail's own tab for the starred shelf. No profile id is empty. */
export const SHELF = ''

export const keyOf = (row: Row): string => `${row.profile.id}:${row.model}`

/* What the agent itself said of a model: its name, and why it cannot be used. */
export const offerOf = (profile: Profile, model: string): OfferedModel | undefined =>
  profile.offered?.find((one) => one.id === model)

/* A model as a person reads it: the agent's own name for it where it gave one. */
export const shownName = (profile: Profile, model: string): string =>
  offerOf(profile, model)?.name ?? modelName(model, profile.env ?? [])

export function rowsFor(
  usable: readonly Profile[],
  tab: string,
  query: string,
  stars: readonly string[],
): Row[] {
  const needle = query.trim().toLowerCase()
  const searched = needle ? usable : usable.filter((one) => tab === SHELF || one.id === tab)
  return searched.flatMap((profile) =>
    (profile.models ?? [])
      .map((model) => ({ profile, model }))
      .filter((row) => {
        if (!needle) return tab !== SHELF || stars.includes(keyOf(row))
        const env = profile.env ?? []
        return `${shownName(profile, row.model)} ${modelHint(row.model, env)} ${profile.label}`
          .toLowerCase()
          .includes(needle)
      }),
  )
}

/* The next tab along the rail, wrapping, with the shelf as the first stop. */
export function stepTab(usable: readonly Profile[], tab: string, step: 1 | -1): string {
  const stops = [SHELF, ...usable.map((one) => one.id)]
  const at = Math.max(0, stops.indexOf(tab))
  return stops[(at + step + stops.length) % stops.length]
}

/* The model a conversation is on: the one picked, or what the account runs
   when nobody picked — which is the list's first entry, its default. */
export const current = (profile: Profile | undefined, model: string | null): string =>
  model ?? profile?.models?.[0] ?? ''

/*
 * One line under an account's name saying what makes it that account.
 *
 * Two profiles of one CLI look the same by their mark; what tells `claude2`
 * from `glm` is where it signs in and where it sends requests, so that is
 * what the line says — the endpoint's host first, since that is the bigger
 * difference, then the config folder.
 */
export function accountHint(profile: Profile): string {
  const env = profile.env ?? []
  const url = valueOf(env, ACCOUNT.url)
  if (url) {
    try {
      return new URL(url).host
    } catch {
      return url
    }
  }
  const config = valueOf(env, ACCOUNT.config)
  if (config) return config.split('/').filter(Boolean).pop() ?? config
  return profile.command === profile.driver ? t('sessions.model.defaultSignIn') : profile.command
}
