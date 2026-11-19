import type { Message } from '../gen/bindings'
import { useShellPick } from './shellStore'
import { t } from '../i18n'
import { tRich } from '../i18n/rich'

type RewoundPart = Extract<Message['parts'][number], { kind: 'rewound' }>

/* Where a forked conversation came from. The original is one click away and
   says it is whole, so nobody has to guess which copy still has the later
   turns. */
export function Rewound({ part }: { part: RewoundPart }): React.JSX.Element {
  const show = useShellPick((shell) => shell.show)
  return (
    <div className="rwnd">
      {tRich(
        'workspace.rewound.said',
        {
          from: (
            <button className="rwnd__from" onClick={() => show('chat', { id: part.from_conversation })}>
              {t('workspace.rewound.from')}
            </button>
          ),
        },
        { turn: part.turn },
      )}
    </div>
  )
}
