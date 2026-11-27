import { t } from '../i18n'
import { ask, commands } from './live'
import { Icon } from './Icon'

/** The community's Discord. A permanent invite, so an old build still lands. */
export const COMMUNITY_INVITE = 'https://discord.gg/yW8VUyY63b'

/* Opened by the app, not the window: nothing answers wry's new-window request. */
export function openCommunity(): void {
  void ask(() => commands.urlOpen(COMMUNITY_INVITE))
}

const icon = (
  <Icon size="s"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" /></Icon>
)

/** The account menu's row. */
export function CommunityMenuItem({ onPick }: { onPick: () => void }): React.JSX.Element {
  return (
    <button className="newmenu__item" role="menuitem" onClick={() => (onPick(), openCommunity())}>
      <span className="newmenu__ico">{icon}</span>
      <span className="newmenu__label">{t('workspace.community.label')}</span>
    </button>
  )
}

/** The foot of the settings column, under the panes. */
export function CommunityFoot(): React.JSX.Element {
  return (
    <button className="prefs__comm" onClick={openCommunity} title={t('workspace.community.title')}>
      {icon}
      {t('workspace.community.label')}
    </button>
  )
}
