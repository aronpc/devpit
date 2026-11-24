import { t } from '../i18n'
import { ControlMenu, type Choice } from './ControlMenu'
import { effortName, modeName, modes } from './chat'
import { ModelPicker } from './ModelPicker'
import type { useChat } from './useChat'

/*
 * What this turn will run as: which account and model, how hard it thinks,
 * and what it may do without asking.
 *
 * Every one of these was a `<select>`, which drew the operating system's own
 * dropdown over the window and had nowhere to say what a choice does — and
 * for permission, what it does is the whole question.
 */

export function Chips({
  chat,
  refocus,
}: {
  chat: ReturnType<typeof useChat>
  refocus: () => void
}): React.JSX.Element {
  const efforts = chat.profiles.find((one) => one.id === chat.profileId)?.efforts ?? []
  const fallback = chat.profiles.find((one) => one.id === chat.profileId)?.effortDefault ?? null

  const permissionChoices: Choice[] = modes().map((mode) => ({
    id: mode.id,
    label: mode.label,
    what: mode.what,
    selected: mode.id === chat.permission,
  }))

  const levels: Choice[] = efforts.map((effort) => ({
    id: effort,
    label: effortName(effort),
    section: t('workspace.chips.reasoning'),
    suffix: effort === fallback ? t('workspace.chips.default') : undefined,
    selected: effort === (chat.effort ?? fallback),
  }))

  return (
    <>
      <ModelPicker
        profiles={chat.profiles}
        profileId={chat.profileId}
        model={chat.model}
        fixed={chat.fixed}
        onPick={(profileId, model) => {
          chat.pick(profileId)
          chat.setModel(model)
          refocus()
        }}
      />

      {/* No chip when the CLI offers no levels: a menu with one item in it is
          a control that cannot be used. */}
      {levels.length > 0 && (
        <ControlMenu
          label={effortName(chat.effort ?? fallback ?? efforts[0])}
          title={t('workspace.chips.reasoning')}
          choices={levels}
          onPick={(effort) => {
            chat.setEffort(effort)
            refocus()
          }}
        />
      )}

      <ControlMenu
        label={modeName(chat.permission)}
        title={t('workspace.chips.mayDo')}
        choices={permissionChoices}
        onPick={(mode) => {
          chat.setPermission(mode)
          refocus()
        }}
      />
    </>
  )
}
