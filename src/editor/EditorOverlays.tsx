import type { Editor } from '@milkdown/kit/core'
import { editorViewCtx } from '@milkdown/kit/core'
import { toggleMark } from '@milkdown/kit/prose/commands'
import { TextSelection } from '@milkdown/kit/prose/state'
import type { EditorView } from '@milkdown/kit/prose/view'
import {
  AlignLeft,
  Bold,
  Code,
  FilePlus,
  GripVertical,
  Heading1,
  Heading2,
  Heading3,
  Image,
  Italic,
  Link,
  List,
  ListOrdered,
  SquareCheck,
  SquareCode,
  Table,
  TextQuote,
  type LucideIcon,
} from 'lucide-react'
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from 'react'
import { cn } from '@/lib/cn'
import { isComposing } from '@/lib/keyboard'
import { usePresence } from '@/lib/presence'
import { useApp } from '@/store/app'
import { blockIndexAt, blockStarts } from './block-move'
import { startBlockDrag } from './block-drag'
import { insertPageLink } from './page-link'
import { trackPosition } from './tracked-pos'
import {
  closeSlash,
  filterSlashItems,
  pageTitleFromQuery,
  removeSlashText,
  runBlockCommand,
  slashKey,
  type SlashIcon,
  type SlashItem,
} from './slash'
import type { ViewBridge } from './view-bridge'

const ICONS: Record<SlashIcon, LucideIcon> = {
  text: AlignLeft,
  h1: Heading1,
  h2: Heading2,
  h3: Heading3,
  bullet: List,
  ordered: ListOrdered,
  task: SquareCheck,
  quote: TextQuote,
  code: SquareCode,
  table: Table,
  image: Image,
  page: FilePlus,
}

type Props = { editor: Editor; bridge: ViewBridge; pagePath: string; host: HTMLElement | null }

/** エディタの上に重ねる部品(スラッシュメニュー・書式メニュー・ドラッグのハンドル) */
export function EditorOverlays({ editor, bridge, pagePath, host }: Props) {
  useSyncExternalStore(bridge.subscribe, bridge.version)
  const view = editor.action((ctx) => ctx.get(editorViewCtx))
  if (!host) return null
  return (
    <>
      <SlashMenu editor={editor} view={view} pagePath={pagePath} host={host} />
      <FormatToolbar view={view} host={host} bridge={bridge} />
      <DragHandle view={view} host={host} />
    </>
  )
}

/** host の中での座標(host はブロックを包む要素。スクロールすると一緒に動く) */
const toHost = (host: HTMLElement, x: number, y: number) => {
  const r = host.getBoundingClientRect()
  return { left: x - r.left, top: y - r.top }
}

// ---- スラッシュメニュー(デザイン 2) ----

function SlashMenu({ editor, view, pagePath, host }: { editor: Editor; view: EditorView; pagePath: string; host: HTMLElement }) {
  const slash = slashKey.getState(view.state) ?? { active: false as const }
  const createPage = useApp((s) => s.createPage)
  // 変換中は絞り込みを止め、確定してから反映する(変換中に候補が入れ替わらないように)
  const [stableQuery, setStableQuery] = useState('')
  if (slash.active && !view.composing && slash.query !== stableQuery) setStableQuery(slash.query)
  const query = slash.active ? stableQuery : ''
  const items = filterSlashItems(query)
  const [selected, setSelected] = useState(0)
  const [imageMode, setImageMode] = useState<{ pos: number } | null>(null)
  const [lastQuery, setLastQuery] = useState(query)
  if (lastQuery !== query) {
    setLastQuery(query)
    setSelected(0)
  }
  const open = (slash.active && !view.composing) || imageMode !== null
  const { mounted, ref } = usePresence<HTMLDivElement>(open, 'menu')
  // メニューの位置。閉じる動きの間は、最後の位置のまま
  const [position, setPosition] = useState({ left: 0, top: 0, above: false })
  if (slash.active) {
    const next = slashPosition(view, host, slash.from)
    if (next && (next.left !== position.left || next.top !== position.top || next.above !== position.above)) setPosition(next)
  }

  const choose = (item: SlashItem | undefined) => {
    if (!item || !slash.active) return
    if (item.id === 'page') {
      const title = pageTitleFromQuery(slash.query) || '無題'
      const pos = removeSlashText(view, slash)
      const tracked = trackPosition(view, pos)
      // 未保存の変更は createPage の前に保存し終える(ストアが行う)。返ってきた title と path を正とする
      void createPage(pagePath, title).then((meta) => {
        const at = tracked.release()
        if (!meta || at === null) return
        view.dispatch(view.state.tr.setSelection(TextSelection.near(view.state.doc.resolve(at))))
        insertPageLink(view, pagePath, meta)
        view.focus()
      })
      return
    }
    if (item.id === 'image') {
      setImageMode({ pos: removeSlashText(view, slash) })
      return
    }
    editor.action((ctx) => runBlockCommand(ctx, view, item, slash))
  }

  // メニューが開いている間は、エディタより先にキーを受け取る(変換中のキーには反応しない)
  useEffect(() => {
    if (!slash.active) return
    const onKey = (e: KeyboardEvent) => {
      if (isComposing(e)) return
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        if (items.length === 0) return
        e.preventDefault()
        e.stopPropagation()
        setSelected((i) => (i + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length)
      } else if ((e.key === 'Enter' || e.key === 'Tab') && items.length > 0) {
        e.preventDefault()
        e.stopPropagation()
        choose(items[selected])
      } else if (e.key === 'Escape') {
        e.preventDefault()
        e.stopPropagation()
        closeSlash(view)
      }
    }
    view.dom.addEventListener('keydown', onKey, true)
    return () => view.dom.removeEventListener('keydown', onKey, true)
  })

  if (!mounted) return null
  const pos = position
  return (
    <div
      ref={ref}
      role="listbox"
      aria-label="ブロックを追加"
      className="editor-popover absolute z-20 w-60 py-1"
      style={{ left: pos.left, top: pos.top, transform: pos.above ? 'translateY(-100%)' : undefined, transformOrigin: pos.above ? 'bottom left' : 'top left' }}
      onMouseDown={(e) => e.preventDefault()}
    >
      {imageMode ? (
        <ImageInput
          onSubmit={(src) => {
            const at = Math.min(imageMode.pos, view.state.doc.content.size)
            const image = view.state.schema.nodes.image.create({ src, alt: '', title: '' })
            view.dispatch(view.state.tr.insert(at, image))
            setImageMode(null)
            view.focus()
          }}
          onCancel={() => {
            setImageMode(null)
            view.focus()
          }}
        />
      ) : items.length === 0 ? (
        <p className="px-3 py-4 text-center text-sm leading-relaxed text-muted">
          「{query}」に一致する
          <br />
          コマンドはありません
        </p>
      ) : (
        items.map((item, i) => {
          const Icon = ICONS[item.id]
          return (
            <button
              key={item.id}
              type="button"
              role="option"
              aria-selected={i === selected}
              className={cn('hover-fade mx-1 flex w-[calc(100%-8px)] items-center gap-2.5 rounded-sm px-2 py-1.5 text-left text-sm', i === selected && 'bg-accent/12')}
              onMouseEnter={() => setSelected(i)}
              onClick={() => choose(item)}
            >
              <Icon size={16} className="shrink-0 text-muted" />
              {item.hint && <span className="text-muted">{item.hint}</span>}
              <span>{item.label}</span>
            </button>
          )
        })
      )}
    </div>
  )
}

function slashPosition(view: EditorView, host: HTMLElement, from: number) {
  try {
    const c = view.coordsAtPos(from)
    const above = window.innerHeight - c.bottom < 360
    return { left: toHost(host, c.left, 0).left, top: above ? toHost(host, 0, c.top).top - 6 : toHost(host, 0, c.bottom).top + 6, above }
  } catch {
    return null
  }
}

function ImageInput({ onSubmit, onCancel }: { onSubmit: (src: string) => void; onCancel: () => void }) {
  const [value, setValue] = useState('')
  return (
    <div className="px-2 py-1.5">
      <input
        autoFocus
        value={value}
        placeholder="画像のパスまたは URL"
        aria-label="画像のパスまたは URL"
        className="w-full rounded-sm border border-border bg-bg px-2 py-1 text-sm outline-none focus:border-focus"
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (isComposing(e.nativeEvent)) return
          if (e.key === 'Enter' && value.trim()) onSubmit(value.trim())
          if (e.key === 'Escape') onCancel()
        }}
        onBlur={onCancel}
      />
    </div>
  )
}

// ---- 書式メニュー(デザイン 3 右) ----

function FormatToolbar({ view, host, bridge }: { view: EditorView; host: HTMLElement; bridge: ViewBridge }) {
  const { selection, schema } = view.state
  const [linkMode, setLinkMode] = useState(false)
  const inCode = selection.$from.parent.type.spec.code === true
  const open =
    linkMode ||
    (selection instanceof TextSelection && !selection.empty && !inCode && view.hasFocus() && !view.composing && !bridge.mouseDown() && !slashKey.getState(view.state)?.active)
  const { mounted, ref } = usePresence<HTMLDivElement>(open, 'menu')
  // 選択範囲の上の中央。リンクを入力している間と、閉じる動きの間は、最後の位置のまま
  const [position, setPosition] = useState({ left: 0, top: 0 })
  if (open && !linkMode) {
    const next = selectionTop(view, host)
    if (next && (next.left !== position.left || next.top !== position.top)) setPosition(next)
  }
  if (!mounted) return null

  const marks = schema.marks
  const active = (name: string) => {
    const type = marks[name]
    if (!type) return false
    const { from, to } = view.state.selection
    return view.state.doc.rangeHasMark(from, to, type)
  }
  const toggle = (name: string) => {
    const type = marks[name]
    if (type) toggleMark(type)(view.state, view.dispatch)
    view.focus()
  }
  const buttons: { name: string; label: string; keys: string; Icon: LucideIcon }[] = [
    { name: 'strong', label: '太字', keys: '⌘B', Icon: Bold },
    { name: 'emphasis', label: '斜体', keys: '⌘I', Icon: Italic },
    { name: 'inlineCode', label: 'コード', keys: '⌘⇧C', Icon: Code },
  ]

  return (
    <div
      ref={ref}
      role="toolbar"
      aria-label="書式"
      className="editor-popover absolute z-20 flex items-stretch gap-0.5 p-1"
      style={{ left: position.left, top: position.top, translate: '-50% -100%', transformOrigin: 'bottom center' }}
      onMouseDown={(e) => {
        if (!(e.target instanceof HTMLInputElement)) e.preventDefault()
      }}
    >
      {linkMode ? (
        <LinkInput
          initial={currentHref(view)}
          onSubmit={(href) => {
            const type = marks.link
            const { from, to } = view.state.selection
            const tr = view.state.tr.removeMark(from, to, type)
            view.dispatch(href ? tr.addMark(from, to, type.create({ href, title: null })) : tr)
            setLinkMode(false)
            view.focus()
          }}
          onCancel={() => {
            setLinkMode(false)
            view.focus()
          }}
        />
      ) : (
        <>
          {buttons.map(({ name, label, keys, Icon }) => (
            <button
              key={name}
              type="button"
              aria-pressed={active(name)}
              title={`${label} (${keys})`}
              className={cn('hover-fade flex flex-col items-center rounded-sm px-2.5 py-1 hover:bg-border/60', active(name) && 'bg-accent/12 text-focus')}
              onClick={() => toggle(name)}
            >
              <span className="flex items-center gap-1.5 text-sm">
                <Icon size={15} strokeWidth={2.25} />
                {label}
              </span>
              <span className="text-[11px] text-muted">{keys}</span>
            </button>
          ))}
          <button
            type="button"
            aria-pressed={active('link')}
            title="リンク"
            className={cn('hover-fade flex items-center rounded-sm px-2.5 hover:bg-border/60', active('link') && 'bg-accent/12 text-focus')}
            onClick={() => setLinkMode(true)}
          >
            <Link size={15} />
          </button>
        </>
      )}
    </div>
  )
}

function selectionTop(view: EditorView, host: HTMLElement) {
  try {
    const { from, to } = view.state.selection
    const a = view.coordsAtPos(from)
    const b = view.coordsAtPos(to)
    const p = toHost(host, (Math.min(a.left, b.left) + Math.max(a.right, b.right)) / 2, Math.min(a.top, b.top))
    return { left: p.left, top: p.top - 8 }
  } catch {
    return null
  }
}

function currentHref(view: EditorView): string {
  const { from, to } = view.state.selection
  let href = ''
  view.state.doc.nodesBetween(from, to, (node) => {
    const link = node.marks.find((m) => m.type.name === 'link')
    if (link && !href) href = String(link.attrs.href)
  })
  return href
}

function LinkInput({ initial, onSubmit, onCancel }: { initial: string; onSubmit: (href: string) => void; onCancel: () => void }) {
  const [value, setValue] = useState(initial)
  return (
    <input
      autoFocus
      value={value}
      placeholder="リンク先の URL(空にすると外す)"
      aria-label="リンク先の URL"
      className="w-72 rounded-sm border border-border bg-bg px-2 py-1 text-sm outline-none focus:border-focus"
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={(e) => {
        if (isComposing(e.nativeEvent)) return
        if (e.key === 'Enter') onSubmit(value.trim())
        if (e.key === 'Escape') onCancel()
      }}
      onBlur={onCancel}
    />
  )
}

// ---- ドラッグのハンドル(デザイン 3 左) ----

function DragHandle({ view, host }: { view: EditorView; host: HTMLElement }) {
  const [hover, setHover] = useState<{ index: number; top: number; left: number } | null>(null)
  const handleRef = useRef<HTMLButtonElement>(null)

  useLayoutEffect(() => {
    // マウスの高さにあるブロックを探す(ブロック全体の位置は読まず、その高さの1点だけを調べる)
    const onMove = (e: MouseEvent) => {
      if (document.body.classList.contains('block-drag-active')) return
      if (e.target === handleRef.current || handleRef.current?.contains(e.target as Node)) return
      const content = view.dom.getBoundingClientRect()
      const found = view.posAtCoords({ left: content.left + 8, top: e.clientY })
      if (!found) return setHover(null)
      const index = blockIndexAt(view, found.pos)
      const start = blockStarts(view)[index]
      const el = start === undefined ? null : view.nodeDOM(start)
      if (!(el instanceof HTMLElement)) return setHover(null)
      const r = el.getBoundingClientRect()
      const line = Number.parseFloat(getComputedStyle(el).lineHeight) || 24
      const p = toHost(host, r.left, r.top + Math.min(r.height, line) / 2)
      setHover((prev) => (prev?.index === index && prev.top === p.top ? prev : { index, top: p.top, left: p.left }))
    }
    const onLeave = (e: MouseEvent) => {
      if (!host.contains(e.relatedTarget as Node | null)) setHover(null)
    }
    host.addEventListener('mousemove', onMove)
    host.addEventListener('mouseleave', onLeave)
    return () => {
      host.removeEventListener('mousemove', onMove)
      host.removeEventListener('mouseleave', onLeave)
    }
  }, [view, host])

  if (!hover || !view.editable) return null
  return (
    <button
      ref={handleRef}
      type="button"
      tabIndex={-1}
      aria-label="ドラッグして並べ替え(キーボードでは ⌘⇧↑ / ⌘⇧↓)"
      className="drag-handle absolute z-10 flex h-6 w-5 items-center justify-center rounded-sm text-muted hover:bg-border/60"
      style={{ left: hover.left - 26, top: hover.top - 12 }}
      onMouseDown={(e) => e.preventDefault()}
      onPointerDown={(e) => {
        if (e.button !== 0) return
        e.preventDefault()
        const scroller = host.closest<HTMLElement>('.overflow-y-auto') ?? document.documentElement
        startBlockDrag(view, hover.index, e.nativeEvent, scroller)
        setHover(null)
      }}
    >
      <GripVertical size={14} />
    </button>
  )
}
