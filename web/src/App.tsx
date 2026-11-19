import { LocaleRoot } from './i18n/LocaleRoot'
import { AppShell } from './shell/AppShell'

export default function App(): React.JSX.Element {
  return (
    <LocaleRoot>
      <AppShell />
    </LocaleRoot>
  )
}
