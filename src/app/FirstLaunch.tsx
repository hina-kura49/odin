import { Folder } from 'lucide-react'
import { useApp } from '@/store/app'
import { EmptyState } from './EmptyState'
import { NoticeBar } from './NoticeBar'

/** 初回起動(デザイン 6): 保管庫のフォルダをまだ選んでいない。保管庫を開けなかったときも、ここで選び直す */
export function FirstLaunch() {
  const openVault = useApp((s) => s.openVault)
  return (
    <div className="flex h-full flex-col bg-panel">
      {/* タイトルバーを重ねているので、上の帯でウィンドウを動かせるようにする */}
      <header data-tauri-drag-region className="h-12 shrink-0" />
      <div className="relative flex min-h-0 flex-1 items-center justify-center overflow-y-auto">
        <div className="absolute inset-x-0 top-0 px-6">
          <NoticeBar />
        </div>
        <EmptyState
          icon={<Folder size={44} strokeWidth={1.25} />}
          title="ノートの保存先を選択"
          action={
            <div className="flex flex-col items-center gap-3">
              <button type="button" autoFocus onClick={() => void openVault()} className="hover-fade rounded-md bg-accent px-4 py-2 text-sm text-white hover:bg-focus">
                フォルダを選択…
              </button>
              <p className="text-sm text-muted">あとから変更できます。</p>
            </div>
          }
        >
          ローカルのフォルダにMarkdownファイルを
          <br />
          保存します。
        </EmptyState>
      </div>
    </div>
  )
}
