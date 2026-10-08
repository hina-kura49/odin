import { ChevronRight, ChevronsLeft, CircleArrowDown, FileText, Folder, FolderOpen, Plus, Trash2 } from 'lucide-react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useMemo, useRef, useState } from 'react'
import type { TreeNode } from '@/backend'
import { cn } from '@/lib/cn'
import { flipRows } from '@/lib/flip-rows'
import { ancestorsOf, isPage, useApp } from '@/store/app'

/** macOS の信号機ボタン(Tauri の titleBarStyle: Overlay で表示される)の分だけ空ける */
const isTauri = '__TAURI_INTERNALS__' in window

export function Sidebar({ collapsed }: { collapsed: boolean }) {
  const toggleSidebar = useApp((s) => s.toggleSidebar)
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
        <NavItem icon={<Trash2 size={16} />} label="ゴミ箱" />
      </nav>

      <button
        type="button"
        onClick={() => setNotesOpen((v) => !v)}
        className="hover-fade mx-2 mt-5 flex items-center rounded-sm px-2 py-1 text-sm text-muted hover:bg-border/60"
      >
        <span className="flex-1 text-left">ノート</span>
        <ChevronRight size={14} className="chevron" data-open={notesOpen} />
      </button>

      {notesOpen ? <Tree /> : <div className="flex-1" />}

      <div className="border-t border-border px-2 py-2">
        <button type="button" className="hover-fade flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-base text-muted hover:bg-border/60">
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

type Row = { node: TreeNode; depth: number; open: boolean }

/** 開いているフォルダだけをたどって、表示する行の一覧にする */
function flatten(nodes: TreeNode[], expanded: Set<string>, depth = 0, out: Row[] = []): Row[] {
  for (const node of nodes) {
    const open = expanded.has(node.path)
    out.push({ node, depth, open })
    if (open && node.children.length > 0) flatten(node.children, expanded, depth + 1, out)
  }
  return out
}

const ROW_HEIGHT = 28

/**
 * ノートのツリー。数百項目でも軽く開閉できるよう、見えている行だけを描く(TanStack Virtual)。
 * 行の位置は top で決め、開閉の動き(flipRows)には transform を使う。
 */
function Tree() {
  // スクロール領域はツリー自身が持つ(親の ref は子の初回計測に間に合わないため)
  const scrollRef = useRef<HTMLDivElement>(null)
  const tree = useApp((s) => s.tree)
  const selectedPath = useApp((s) => s.selectedPath)
  const openPage = useApp((s) => s.openPage)
  const prefetch = useApp((s) => s.prefetch)
  // 最上位のフォルダと、選択中のページの祖先は最初から開いておく
  const [expanded, setExpanded] = useState<Set<string>>(() => {
    const set = new Set(tree.map((n) => n.path))
    if (selectedPath) for (const a of ancestorsOf(tree, selectedPath)) set.add(a.path)
    return set
  })
  const rows = useMemo(() => flatten(tree, expanded), [tree, expanded])
  // React Compiler は使っていないので、メモ化できない API でも問題ない
  // oxlint-disable-next-line react/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    getItemKey: (i) => rows[i].node.path,
    overscan: 8,
  })

  const toggle = (path: string) =>
    flipRows(scrollRef.current, () =>
      setExpanded((prev) => {
        const next = new Set(prev)
        if (next.has(path)) next.delete(path)
        else next.add(path)
        return next
      }),
    )

  return (
    <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
    <ul role="tree" className="relative" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((item) => {
        const { node, depth, open } = rows[item.index]
        const page = isPage(node)
        const selected = node.path === selectedPath
        const hasChildren = node.children.length > 0
        return (
          <li
            key={item.key}
            role="treeitem"
            aria-level={depth + 1}
            aria-selected={selected}
            aria-expanded={hasChildren ? open : undefined}
            data-row={node.path}
            className={cn(
              'hover-fade absolute inset-x-0 flex cursor-default items-center gap-1.5 rounded-sm pr-2 text-base',
              selected ? 'bg-accent/12 font-medium text-text' : 'hover:bg-border/60',
            )}
            style={{ top: item.start, height: ROW_HEIGHT, paddingLeft: 4 + depth * 14 }}
            onClick={() => (page ? void openPage(node.path) : toggle(node.path))}
            onMouseEnter={() => page && prefetch(node.path)}
          >
            <span
              className={cn('flex w-3.5 justify-center text-muted', !hasChildren && 'invisible')}
              onClick={(e) => {
                e.stopPropagation()
                toggle(node.path)
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
    </div>
  )
}
