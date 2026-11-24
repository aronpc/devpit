import { Fragment, type ReactNode } from 'react'

import { t, type TranslateArgs } from './index'

/*
 * A sentence with a piece of the screen inside it — a name in bold, a path in
 * code — said in the order the language wants. The catalog carries the
 * placeholders (`{label}`); the caller carries what goes in them.
 */
export function tRich(key: string, nodes: Record<string, ReactNode>, args: TranslateArgs = {}): ReactNode[] {
  /* Each node's own placeholder is handed to `t` as itself, so it survives
     the fill and can be split on here. */
  const kept: TranslateArgs = { ...args }
  for (const name of Object.keys(nodes)) kept[name] = `{${name}}`
  return t(key, kept)
    .split(/(\{\w+\})/)
    .map((part, at) => {
      const name = /^\{(\w+)\}$/.exec(part)?.[1]
      return name !== undefined && name in nodes ? <Fragment key={at}>{nodes[name]}</Fragment> : part
    })
}
