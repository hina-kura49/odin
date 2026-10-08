import { ChevronsRight } from 'lucide-react'
import { useCallback, useLayoutEffect, useRef, useState } from 'react'
import { MarkdownEditor } from '@/editor/MarkdownEditor'
import { duration, easing, exitRatio, slideFrom } from '@/lib/motion'
import { ancestorsOf, useApp } from '@/store/app'

export function PageView() {
  const page = useApp((s) => s.page)
  const tree = useApp((s) => s.tree)
  const sidebarCollapsed = useApp((s) => s.sidebarCollapsed)
  const toggleSidebar = useApp((s) => s.toggleSidebar)
  const trail = page ? ancestorsOf(tree, page.path) : []
  const title = trail.at(-1)?.title ?? ''
  // エディタが最初の文書を表示できるまで、タイトルも含めて何も出さない
  const [ready, setReady] = useState(false)
  const onReady = useCallback(() => setReady(true), [])
  const mainRef = useSidebarMotion(sidebarCollapsed)

  return (
    <main ref={mainRef} className="flex min-w-0 flex-1 flex-col bg-panel">
      <header data-tauri-drag-region data-follow-sidebar className="flex h-12 shrink-0 items-center gap-2 px-4 text-sm text-muted">
        {sidebarCollapsed && (
          <button type="button" onClick={toggleSidebar} title="サイドバーを開く (⌘\)" className="hover-fade rounded-sm p-1 hover:bg-border/60">
            <ChevronsRight size={16} />
          </button>
        )}
        <nav aria-label="パンくずリスト" className="truncate">
          {trail.map((n, i) => (
            <span key={n.path}>
              {i > 0 && <span className="mx-1.5">/</span>}
              {n.title}
            </span>
          ))}
        </nav>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {page && (
          <article data-follow-sidebar className="page" style={{ visibility: ready ? 'visible' : 'hidden' }}>
            <h1 className="page-title">{title}</h1>
            <MarkdownEditor path={page.path} loadId={page.loadId} content={page.content} onReady={onReady} />
          </article>
        )}
      </div>
    </main>
  )
}

/**
 * サイドバーの開閉に本文を追従させる。本文の幅を毎フレーム変えるとレイアウトの再計算が続くので、
 * 左の余白は一度に切り替え、パンくずと本文の見た目の位置だけを元の位置から滑らせる(transform)。
 * 動いている最中に再び開閉しても、今の位置から続けて動く。
 */
function useSidebarMotion(collapsed: boolean) {
  const mainRef = useRef<HTMLElement>(null)
  const first = useRef(true)
  useLayoutEffect(() => {
    const main = mainRef.current
    if (!main) return
    const margin = collapsed ? '0px' : 'var(--sidebar-width)'
    if (first.current) {
      first.current = false
      main.style.marginLeft = margin
      return
    }
    const targets = Array.from(main.querySelectorAll<HTMLElement>('[data-follow-sidebar]'))
    const before = targets.map((el) => el.getBoundingClientRect())
    main.style.marginLeft = margin
    // 閉じる(出る)動きは、開く(入る)動きより短く
    const ms = duration('sidebar') * (collapsed ? exitRatio() : 1)
    targets.forEach((el, i) => slideFrom(el, before[i], { duration: ms, easing: easing(collapsed ? 'exit' : 'enter'), axis: 'x' }))
  }, [collapsed])
  return mainRef
}
