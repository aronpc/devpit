import { useState } from 'react'

import { t } from '../i18n'
import { ACTIVITIES, activityOf, keepActivity, labelOf, type Activity } from './activity'
import { ControlMenu } from './ControlMenu'

/* How much of the work a conversation shows, picked in its corner. */
export function ActivityPick({ value, onPick }: { value: Activity; onPick: (activity: Activity) => void }): React.JSX.Element {
  return (
    <ControlMenu
      label={labelOf(value)}
      title={t('workspace.activity.title')}
      opens="down"
      choices={ACTIVITIES.map((one) => ({ id: one.id, label: one.label, what: one.what, selected: one.id === value }))}
      onPick={(id) => {
        const picked = ACTIVITIES.find((one) => one.id === id)
        if (picked) onPick(picked.id)
      }}
    />
  )
}

/* What a conversation shows until it is given its own pick. */
export function ActivityDefault(): React.JSX.Element {
  const [value, setValue] = useState(() => activityOf(null))
  return (
    <div className="pref">
      <span className="pref__body">
        <span className="pref__t">{t('workspace.activity.chat')}</span>
        <span className="pref__d">{t('workspace.activity.chatWhat')}</span>
      </span>
      <div className="seg seg--tight" role="radiogroup" aria-label={t('workspace.activity.chat')}>
        {ACTIVITIES.map((one) => (
          <button
            key={one.id}
            className="segb"
            role="radio"
            aria-checked={value === one.id}
            title={one.what}
            onClick={() => {
              keepActivity(null, one.id)
              setValue(one.id)
            }}
          >
            {one.label}
          </button>
        ))}
      </div>
    </div>
  )
}
