import { useEffect } from 'react'
import { useShortcut } from '@/lib/keyboard'
import { useApp } from '@/store/app'
import { PageView } from './PageView'
import { Sidebar } from './Sidebar'

export function App() {
  const status = useApp((s) => s.status)
  const sidebarCollapsed = useApp((s) => s.sidebarCollapsed)
  const init = useApp((s) => s.init)
  const openVault = useApp((s) => s.openVault)
  const toggleSidebar = useApp((s) => s.toggleSidebar)

  useEffect(() => {
    void init()
  }, [init])

  useShortcut({ key: '\\', mod: true }, toggleSidebar)

  if (status === 'loading') return null
  if (status === 'no-vault') {
    // 段階7でデザインの「初回起動」画面にする
    return (
      <div className="flex h-full items-center justify-center">
        <button type="button" onClick={() => void openVault()} className="rounded-md bg-accent px-4 py-2 text-white">
          フォルダを選択…
        </button>
      </div>
    )
  }

  return (
    <div className="relative flex h-full overflow-hidden">
      <Sidebar collapsed={sidebarCollapsed} />
      <PageView />
    </div>
  )
}
