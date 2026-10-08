import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { cn } from '@/lib/cn'
import { isComposing } from '@/lib/keyboard'
import { usePresence } from '@/lib/presence'

export type MenuItem = { label: string; keys?: string; run: () => void }

/**
 * 右クリック(またはキーボード)で開く小さなメニュー。
 * ↑↓ で選び、Enter で実行、Esc やメニューの外のクリックで閉じる。閉じたら、開く前の場所へ入力位置を戻す。
 */
export function ContextMenu({ at, items, onClose }: { at: { x: number; y: number } | null; items: MenuItem[]; onClose: () => void }) {
  const open = at !== null
  const { mounted, ref } = usePresence<HTMLDivElement>(open, 'menu')
  const [active, setActiveState] = useState(0)
  // キーを素早く続けて押しても取りこぼさないよう、選んでいる項目は最新の値を読む
  const activeRef = useRef(0)
  const setActive = (next: number | ((prev: number) => number)) => {
    activeRef.current = typeof next === 'function' ? next(activeRef.current) : next
    setActiveState(activeRef.current)
  }
  // 閉じる動きの間も、最後に開いた位置と項目のまま見せる
  const [lastAt, setLastAt] = useState(at)
  const [lastItems, setLastItems] = useState(items)
  const returnFocus = useRef<HTMLElement | null>(null)
  if (at && at !== lastAt) {
    setLastAt(at)
    setLastItems(items)
    setActiveState(0)
  }

  useLayoutEffect(() => {
    if (!open) return
    activeRef.current = 0
    returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    ref.current?.focus()
  }, [open, ref])

  useEffect(() => {
    if (!open) return
    const onDown = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose()
    }
    window.addEventListener('pointerdown', onDown, true)
    return () => window.removeEventListener('pointerdown', onDown, true)
  }, [open, onClose, ref])

  if (!mounted || !lastAt) return null

  const close = () => {
    onClose()
    returnFocus.current?.focus()
  }
  const run = (item: MenuItem | undefined) => {
    if (!item) return
    close()
    item.run()
  }

  // 画面の端からはみ出さない位置
  const left = Math.min(lastAt.x, window.innerWidth - 220)
  const top = Math.min(lastAt.y, window.innerHeight - (lastItems.length * 32 + 16))

  return createPortal(
    <div
      ref={ref}
      role="menu"
      tabIndex={-1}
      className="editor-popover fixed z-50 w-52 py-1 outline-none"
      style={{ left, top, transformOrigin: 'top left' }}
      onKeyDown={(e) => {
        if (isComposing(e.nativeEvent)) return
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault()
          setActive((i) => (i + (e.key === 'ArrowDown' ? 1 : lastItems.length - 1)) % lastItems.length)
        } else if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          run(lastItems[activeRef.current])
        } else if (e.key === 'Escape') {
          e.preventDefault()
          close()
        }
      }}
    >
      {lastItems.map((item, i) => (
        <button
          key={item.label}
          type="button"
          role="menuitem"
          tabIndex={-1}
          className={cn(
            'hover-fade mx-1 flex w-[calc(100%-8px)] items-center rounded-sm px-2.5 py-1.5 text-left text-sm',
            i === active && 'bg-accent/12',
          )}
          onMouseEnter={() => setActive(i)}
          onClick={() => run(item)}
        >
          <span className="flex-1">{item.label}</span>
          {item.keys && <span className="text-xs text-muted">{item.keys}</span>}
        </button>
      ))}
    </div>,
    document.body,
  )
}
