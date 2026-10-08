import { TextSelection } from '@milkdown/kit/prose/state'
import type { EditorView } from '@milkdown/kit/prose/view'
import { duration, easing, reducedMotion } from '@/lib/motion'

// ---- ブロック(文書の最上位のノード)の並べ替え ----

/** 最上位のブロックの開始位置の一覧 */
export function blockStarts(view: EditorView): number[] {
  const starts: number[] = []
  view.state.doc.forEach((_node, offset) => starts.push(offset))
  return starts
}

/** 最上位のブロックの DOM 要素(index 順) */
export function blockElements(view: EditorView): HTMLElement[] {
  return blockStarts(view).map((pos) => view.nodeDOM(pos)).filter((el): el is HTMLElement => el instanceof HTMLElement)
}

/** pos を含む最上位のブロックの番号 */
export const blockIndexAt = (view: EditorView, pos: number): number => view.state.doc.resolve(pos).index(0)

/**
 * from 番目のブロックを、to 番目に移す(1回の操作なので、取り消しで元に戻る)。
 * カーソルが動かしたブロックの中にあれば、移した先の同じ場所に置く。
 */
export function moveBlock(view: EditorView, from: number, to: number): void {
  const { doc } = view.state
  if (from === to || from < 0 || to < 0 || from >= doc.childCount || to >= doc.childCount) return
  const starts = blockStarts(view)
  const node = doc.child(from)
  const start = starts[from]
  const end = start + node.nodeSize
  const cursor = view.state.selection.head
  const inside = cursor >= start && cursor <= end ? cursor - start : null

  const tr = view.state.tr.delete(start, end)
  // 消した後の文書での、挿入位置
  let insertAt = 0
  tr.doc.forEach((child, offset, index) => {
    if (index < to) insertAt = offset + child.nodeSize
  })
  tr.insert(insertAt, node)
  if (inside !== null) tr.setSelection(TextSelection.near(tr.doc.resolve(Math.min(insertAt + inside, tr.doc.content.size))))
  view.dispatch(tr.scrollIntoView())
}

// ---- 並べ替えの動き(FLIP) ----

type Snapshot = Map<HTMLElement, DOMRect>

/** いま見えているブロックの見た目の位置を控える(動いている最中なら、その途中の位置) */
export function snapshotBlocks(view: EditorView, extra?: [HTMLElement, DOMRect]): Snapshot {
  const snap: Snapshot = new Map()
  const viewport = { top: 0, bottom: window.innerHeight }
  for (const el of blockElements(view)) {
    const r = el.getBoundingClientRect()
    if (r.bottom >= viewport.top - 200 && r.top <= viewport.bottom + 200) snap.set(el, r)
  }
  if (extra) snap.set(extra[0], extra[1])
  return snap
}

/**
 * 並べ替えた後、控えた位置から新しい位置へ滑らせる(ばねの動き)。
 * movedFrom: 動かしたブロックは DOM が作り直されるので、新しい要素と、動かす前の見た目の位置を渡す。
 */
export function settleBlocks(view: EditorView, before: Snapshot, moved?: { el: HTMLElement; rect: DOMRect }): void {
  if (reducedMotion()) return
  const ms = duration('drag')
  const ease = easing('spring')
  const animate = (el: HTMLElement, old: DOMRect) => {
    const now = el.getBoundingClientRect()
    const dy = old.top - now.top
    const dx = old.left - now.left
    if (Math.abs(dy) < 0.5 && Math.abs(dx) < 0.5) return
    el.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'none' }], { duration: ms, easing: ease })
  }
  for (const el of blockElements(view)) {
    if (moved && el === moved.el) animate(el, moved.rect)
    else {
      const old = before.get(el)
      if (old) animate(el, old)
    }
  }
}

/** キーボードでの並べ替え(⌘⇧↑ / ⌘⇧↓)。カーソルのあるブロックを1つ上・下へ */
export function moveCurrentBlock(view: EditorView, delta: -1 | 1): boolean {
  const from = blockIndexAt(view, view.state.selection.head)
  const to = from + delta
  if (to < 0 || to >= view.state.doc.childCount) return false
  const before = snapshotBlocks(view)
  const movedEl = blockElements(view)[from]
  const movedRect = movedEl?.getBoundingClientRect()
  for (const el of before.keys()) for (const a of el.getAnimations()) a.cancel()
  moveBlock(view, from, to)
  const newEl = blockElements(view)[to]
  settleBlocks(view, before, newEl && movedRect ? { el: newEl, rect: movedRect } : undefined)
  return true
}
