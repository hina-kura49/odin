import { useEffect } from 'react'
import { backend } from '@/backend'
import { activeSession, flushActive } from '@/editor/registry'
import { useShortcut } from '@/lib/keyboard'
import { captureShortcutFailed, onOpenVaultRequested, watchAppLifecycle } from '@/lib/platform'
import { useApp } from '@/store/app'
import { CommandPalette } from './CommandPalette'
import { FirstLaunch } from './FirstLaunch'
import { openPalette, togglePalette } from './palette-store'
import { PageView } from './PageView'
import { Sidebar } from './Sidebar'

export function App() {
  const status = useApp((s) => s.status)
  const sidebarCollapsed = useApp((s) => s.sidebarCollapsed)
  const init = useApp((s) => s.init)
  const toggleSidebar = useApp((s) => s.toggleSidebar)

  const handleExternalChange = useApp((s) => s.handleExternalChange)

  useEffect(() => {
    void init()
  }, [init])

  // 隠す・終了する・後ろに回る前に、未保存の変更を保存する
  useEffect(() => watchAppLifecycle({ save: flushActive, hasUnsaved: () => activeSession()?.isDirty() ?? false }), [])

  // メニューの「保管庫を開く…」
  useEffect(() => onOpenVaultRequested(() => useApp.getState().openVault()), [])

  // クイックキャプチャのホットキーを登録できなかったら知らせる(アプリはそのまま使える)
  useEffect(() => {
    void captureShortcutFailed()
      .then((failed) => failed && useApp.getState().warnCaptureShortcut())
      .catch(() => {})
  }, [])

  // 他のアプリでの変更: ツリーを取り直し、開いているページの version を比べる
  useEffect(() => backend().onExternalChange(() => void handleExternalChange()), [handleExternalChange])

  const newPage = useApp((s) => s.newPage)
  useShortcut({ key: '\\', mod: true }, toggleSidebar)
  // 新規ページ(開いているページと同じフォルダ) / 子ページ
  useShortcut({ key: 'n', mod: true }, () => void newPage('sibling'), status === 'ready')
  useShortcut({ key: 'n', mod: true, shift: true }, () => void newPage('child'), status === 'ready')
  // コマンドパレット。⌘P(ページへ移動)と ⌘F(検索)も同じパレットを開く
  useShortcut({ key: 'k', mod: true }, togglePalette, status === 'ready')
  useShortcut({ key: 'p', mod: true }, openPalette, status === 'ready')
  useShortcut({ key: 'f', mod: true }, openPalette, status === 'ready')

  if (status === 'loading') return null
  if (status === 'no-vault') return <FirstLaunch />

  return (
    <div className="relative flex h-full overflow-hidden">
      <Sidebar collapsed={sidebarCollapsed} />
      <PageView />
      <CommandPalette />
    </div>
  )
}
