import { ChevronRight, ChevronsLeft, CircleArrowDown, FileText, Folder, FolderOpen, Plus } from 'lucide-react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { TreeNode } from '@/backend'
import { cn } from '@/lib/cn'
import { flipRows } from '@/lib/flip-rows'
import { isComposing } from '@/lib/keyboard'
import { ancestorsOf, isPage, useApp } from '@/store/app'
import { ContextMenu, type MenuItem } from './ContextMenu'

/** macOS の信号機ボタン(Tauri の titleBarStyle: Overlay で表示される)の分だけ空ける */
const isTauri = '__TAURI_INTERNALS__' in window

export function Sidebar({ collapsed }: { collapsed: boolean }) {
  const toggleSidebar = useApp((s) => s.toggleSidebar)
  const newPage = useApp((s) => s.newPage)
  const [notesOpen, setNotesOpen] = useState(true)

  return (
    <aside
      className="sidebar absolute inset-y-0 left-0 z-10 flex w-[var(--sidebar-width)] flex-col border-r border-border bg-bg"
      data-collapsed={collapsed}
      inert={collapsed}
      aria-hidden={collapsed}
    >
      <div data-tauri-drag-region className={cn('flex h-12 items-center gap-2 pr-2', isTauri ? 'pl-20' : 'pl-4')}>
        <span data-tauri-drag-region className="flex-1 truncate text-base font-semibold">
          Odin
        </span>
        <button
          type="button"
          onClick={toggleSidebar}
          title="サイドバーを閉じる (⌘\)"
          className="hover-fade rounded-sm p-1 text-muted hover:bg-border/60"
        >
          <ChevronsLeft size={16} />
        </button>
      </div>

      <nav className="flex flex-col gap-0.5 px-2 pt-1">
        <NavItem icon={<FileText size={16} />} label="すべてのページ" />
        <NavItem icon={<CircleArrowDown size={16} />} label="クイックキャプチャ" />
      </nav>

      <button
        type="button"
        onClick={() => setNotesOpen((v) => !v)}
        aria-expanded={notesOpen}
        className="hover-fade mx-2 mt-5 flex items-center rounded-sm px-2 py-1 text-sm text-muted hover:bg-border/60"
      >
        <span className="flex-1 text-left">ノート</span>
        <ChevronRight size={14} className="chevron" data-open={notesOpen} />
      </button>

      {notesOpen ? <Tree /> : <div className="flex-1" />}

      <div className="border-t border-border px-2 py-2">
        <button
          type="button"
          title="新規ページ (⌘N)"
          className="hover-fade flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-base text-muted hover:bg-border/60"
          onClick={() => void newPage('sibling')}
        >
          <Plus size={16} />
          新規ページ
        </button>
      </div>
    </aside>
  )
}

function NavItem({ icon, label }: { icon: React.ReactNode; label: string }) {
  return (
    <button type="button" className="hover-fade flex items-center gap-2 rounded-sm px-2 py-1.5 text-base hover:bg-border/60">
      <span className="text-muted">{icon}</span>
      {label}
    </button>
  )
}

type Row = { node: TreeNode; depth: number; open: boolean; parent: string | null }

/** 開いているフォルダだけをたどって、表示する行の一覧にする */
function flatten(nodes: TreeNode[], expanded: Set<string>, depth = 0, parent: string | null = null, out: Row[] = []): Row[] {
  for (const node of nodes) {
    const open = expanded.has(node.path)
    out.push({ node, depth, open, parent })
    if (open && node.children.length > 0) flatten(node.children, expanded, depth + 1, node.path, out)
  }
  return out
}

const ROW_HEIGHT = 28

/**
 * ノートのツリー。数百項目でも軽く開閉できるよう、見えている行だけを描く(TanStack Virtual)。
 * 行の位置は top で決め、開閉の動き(flipRows)には transform を使う。
 *
 * キーボード: ツリー全体に入力位置を置き、いまの行を aria-activedescendant で示す(描かれていない行へも移れる)。
 *   ↑↓ 移動 / → 開く・子へ / ← 閉じる・親へ / Enter・Space 開く / Home・End / ⌘⌫ 削除 / Shift+F10 メニュー
 */
function Tree() {
  // スクロール領域はツリー自身が持つ(親の ref は子の初回計測に間に合わないため)
  const scrollRef = useRef<HTMLDivElement>(null)
  const tree = useApp((s) => s.tree)
  const selectedPath = useApp((s) => s.selectedPath)
  const reveal = useApp((s) => s.reveal)
  const openPage = useApp((s) => s.openPage)
  const prefetch = useApp((s) => s.prefetch)
  const newPage = useApp((s) => s.newPage)
  const requestDelete = useApp((s) => s.requestDelete)
  const focusTitle = useApp((s) => s.focusTitle)

  // 最上位のフォルダと、選択中のページの祖先は最初から開いておく
  const [expanded, setExpanded] = useState<Set<string>>(() => {
    const set = new Set(tree.map((n) => n.path))
    if (selectedPath) for (const a of ancestorsOf(tree, selectedPath)) set.add(a.path)
    return set
  })
  const rows = useMemo(() => flatten(tree, expanded), [tree, expanded])
  const [active, setActive] = useState<string | null>(selectedPath)
  const activeIndex = Math.max(0, rows.findIndex((r) => r.node.path === active))
  const [menu, setMenu] = useState<{ at: { x: number; y: number }; row: Row } | null>(null)

  // React Compiler は使っていないので、メモ化できない API でも問題ない
  // oxlint-disable-next-line react/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    getItemKey: (i) => rows[i].node.path,
    overscan: 8,
  })

  const setOpen = useCallback(
    (path: string, open: boolean) =>
      flipRows(scrollRef.current, () =>
        setExpanded((prev) => {
          if (prev.has(path) === open) return prev
          const next = new Set(prev)
          if (open) next.add(path)
          else next.delete(path)
          return next
        }),
      ),
    [],
  )

  // ページを開いたら、そのページの行を見せる(祖先を開き、スクロールする)
  const [pendingScroll, setPendingScroll] = useState<string | null>(null)
  const [lastReveal, setLastReveal] = useState(reveal)
  if (reveal && reveal !== lastReveal) {
    setLastReveal(reveal)
    const ancestors = ancestorsOf(tree, reveal.path).slice(0, -1)
    if (ancestors.some((a) => !expanded.has(a.path))) setExpanded((prev) => new Set([...prev, ...ancestors.map((a) => a.path)]))
    setActive(reveal.path)
    setPendingScroll(reveal.path)
  }
  useEffect(() => {
    if (!pendingScroll) return
    const index = rows.findIndex((r) => r.node.path === pendingScroll)
    if (index >= 0) virtualizer.scrollToIndex(index, { align: 'auto' })
    setPendingScroll(null)
  }, [pendingScroll, rows, virtualizer])

  const moveTo = (index: number) => {
    const row = rows[Math.max(0, Math.min(rows.length - 1, index))]
    if (!row) return
    setActive(row.node.path)
    virtualizer.scrollToIndex(rows.indexOf(row), { align: 'auto' })
  }

  const activate = (row: Row) => {
    if (isPage(row.node)) void openPage(row.node.path)
    else setOpen(row.node.path, !row.open)
  }

  const menuItems = (row: Row): MenuItem[] =>
    isPage(row.node)
      ? [
          { label: '開く', keys: '↩', run: () => void openPage(row.node.path) },
          { label: '子ページを作成', keys: '⌘⇧N', run: () => void openPage(row.node.path).then(() => newPage('child')) },
          {
            label: '名前を変更',
            run: () => void openPage(row.node.path).then(() => focusTitle(row.node.path, true)),
          },
          { label: '削除', keys: '⌘⌫', run: () => void requestDelete(row.node.path) },
        ]
      : [{ label: 'このフォルダに新規ページ', run: () => void newPage({ folder: row.node.path }) }]

  const openMenuAt = (row: Row, el: Element | null) => {
    const r = el?.getBoundingClientRect()
    setMenu({ row, at: { x: (r?.left ?? 0) + 24, y: (r?.bottom ?? 0) + 2 } })
  }

  const onKeyDown = (e: React.KeyboardEvent<HTMLUListElement>) => {
    if (isComposing(e.nativeEvent) || rows.length === 0) return
    const row = rows[activeIndex]
    const handled = () => {
      e.preventDefault()
      e.stopPropagation()
    }
    switch (e.key) {
      case 'ArrowDown':
        handled()
        moveTo(activeIndex + 1)
        break
      case 'ArrowUp':
        handled()
        moveTo(activeIndex - 1)
        break
      case 'Home':
        handled()
        moveTo(0)
        break
      case 'End':
        handled()
        moveTo(rows.length - 1)
        break
      case 'ArrowRight':
        handled()
        if (row.node.children.length > 0 && !row.open) setOpen(row.node.path, true)
        else if (row.open) moveTo(activeIndex + 1)
        break
      case 'ArrowLeft':
        handled()
        if (row.open) setOpen(row.node.path, false)
        else if (row.parent) moveTo(rows.findIndex((r) => r.node.path === row.parent))
        break
      case 'Enter':
      case ' ':
        handled()
        activate(row)
        break
      case 'Backspace':
      case 'Delete':
        if ((e.metaKey || e.key === 'Delete') && isPage(row.node)) {
          handled()
          void requestDelete(row.node.path)
        }
        break
      case 'F10':
        if (e.shiftKey) {
          handled()
          openMenuAt(row, document.getElementById(rowId(activeIndex)))
        }
        break
      case 'ContextMenu':
        handled()
        openMenuAt(row, document.getElementById(rowId(activeIndex)))
        break
    }
  }

  return (
    <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
      <ul
        role="tree"
        aria-label="ノート"
        tabIndex={0}
        aria-activedescendant={rows.length > 0 ? rowId(activeIndex) : undefined}
        className="tree relative outline-none"
        style={{ height: virtualizer.getTotalSize() }}
        onKeyDown={onKeyDown}
      >
        {virtualizer.getVirtualItems().map((item) => {
          const row = rows[item.index]
          const { node, depth, open } = row
          const page = isPage(node)
          const selected = node.path === selectedPath
          const hasChildren = node.children.length > 0
          return (
            <li
              key={item.key}
              id={rowId(item.index)}
              role="treeitem"
              aria-level={depth + 1}
              aria-selected={selected}
              aria-expanded={hasChildren ? open : undefined}
              data-row={node.path}
              data-active={item.index === activeIndex || undefined}
              className={cn(
                'tree-row hover-fade absolute inset-x-0 flex cursor-default items-center gap-1.5 rounded-sm pr-2 text-base',
                selected ? 'bg-accent/12 font-medium text-text' : 'hover:bg-border/60',
              )}
              style={{ top: item.start, height: ROW_HEIGHT, paddingLeft: 4 + depth * 14 }}
              onMouseDown={(e) => {
                // クリックでもツリーに入力位置を置き、続けてキーボードで操作できるようにする
                e.preventDefault()
                setActive(node.path)
                e.currentTarget.closest<HTMLElement>('[role=tree]')?.focus()
              }}
              onClick={() => activate(row)}
              onContextMenu={(e) => {
                e.preventDefault()
                setActive(node.path)
                setMenu({ row, at: { x: e.clientX, y: e.clientY } })
              }}
              onMouseEnter={() => page && prefetch(node.path)}
            >
              <span
                className={cn('flex w-3.5 justify-center text-muted', !hasChildren && 'invisible')}
                onClick={(e) => {
                  e.stopPropagation()
                  setOpen(node.path, !open)
                }}
              >
                <ChevronRight size={12} className="chevron" data-open={open} />
              </span>
              <span className="text-muted">{page ? <FileText size={15} /> : open ? <FolderOpen size={15} /> : <Folder size={15} />}</span>
              <span className="truncate">{node.title}</span>
            </li>
          )
        })}
      </ul>
      <ContextMenu at={menu?.at ?? null} items={menu ? menuItems(menu.row) : []} onClose={() => setMenu(null)} />
    </div>
  )
}

const rowId = (index: number) => `tree-row-${index}`
