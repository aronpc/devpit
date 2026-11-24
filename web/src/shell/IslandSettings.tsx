import { useEffect, useState } from 'react'

import type { IslandDrawn, IslandHow } from '../gen/bindings'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'
import { ask, commands } from './live'
import { PrefSwitch } from './PrefSwitch'

/* Where the island lands depends on the desktop; saying it here is what
   tells a GNOME user why it is off, and what showing it anyway costs. */
const DRAWN: Record<IslandDrawn, string> = {
  above: 'workspace.island.above',
  layer: 'workspace.island.layer',
  plain: 'workspace.island.plain',
}

/** The island's switch, and what this desktop makes of it. */
export function IslandSettings({ on, onFlip }: { on: boolean; onFlip: () => void }): React.JSX.Element {
  const [how, setHow] = useState<IslandHow | null>(null)
  useEffect(() => {
    void Promise.resolve()
      .then(() => commands.islandDrawn())
      .then(setHow)
      .catch(() => undefined)
  }, [])

  const anyway = (): void => {
    if (!how) return
    void ask(() => commands.islandAnywaySet(!how.anyway)).then((answer) => {
      if (answer.data) setHow(answer.data)
    })
  }

  return (
    <>
      <PrefSwitch
        on={on}
        onFlip={onFlip}
        title={t('workspace.island.title')}
        said={t('workspace.island.said')}
        more={<>{tRich('workspace.island.more', { reply: <b>{t('workspace.island.reply')}</b> })} {how && t(DRAWN[how.drawn])}</>}
      />
      {on && how?.drawn === 'plain' && (
        <PrefSwitch
          on={how.anyway}
          onFlip={anyway}
          title={t('workspace.island.anyway')}
          said={t('workspace.island.anywaySaid')}
        />
      )}
    </>
  )
}
