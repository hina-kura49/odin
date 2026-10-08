import type { Node as PMNode } from '@milkdown/kit/prose/model'
import { TextSelection } from '@milkdown/kit/prose/state'
import type { EditorView } from '@milkdown/kit/prose/view'
import { canAnimate, cancelAnimations, duration, easing } from '@/lib/motion'

// ---- 並べ替えの単位 ----
// 並べ替えられるのは「同じ入れ物の中のきょうだい」どうし。
// 入れ物は、文書(最上位のブロック)か、リスト(箇条書き・番号付き・チェックリストの項目)。
// リストの項目は子の項目ごと動く。別のリストへの移動や、階層の変更はしない。

export type Container = {
  /** 入れ物のノードの位置。文書なら -1 */
  pos: number
  /** 入れ物の中の、最初の子の位置 */
  contentStart: number
}

export const DOC_CONTAINER: Container = { pos: -1, contentStart: 0 }

const LIST_TYPES = new Set(['bullet_list', 'ordered_list'])

const containerNode = (doc: PMNode, c: Container): PMNode | null => (c.pos === -1 ? doc : doc.nodeAt(c.pos))

/** 入れ物の中の、子の開始位置の一覧 */
export function childStarts(doc: PMNode, c: Container): number[] {
  const starts: number[] = []
  containerNode(doc, c)?.forEach((_child, offset) => starts.push(c.contentStart + offset))
  return starts
}

/** 入れ物の中の、子の DOM 要素(順番どおり) */
export function childElements(view: EditorView, c: Container): HTMLElement[] {
  return childStarts(view.state.doc, c)
    .map((pos) => view.nodeDOM(pos))
    .filter((el): el is HTMLElement => el instanceof HTMLElement)
}

/**
 * pos を含む並べ替えの単位(入れ物と、その中での番号)。
 * pos がリストの項目の中なら、いちばん内側の項目。そうでなければ最上位のブロック。
 */
export function unitAt(doc: PMNode, pos: number): { container: Container; index: number } {
  const $pos = doc.resolve(pos)
  for (let d = $pos.depth; d > 1; d--) {
    if ($pos.node(d).type.name === 'list_item' && LIST_TYPES.has($pos.node(d - 1).type.name)) {
      const listPos = $pos.before(d - 1)
      return { container: { pos: listPos, contentStart: listPos + 1 }, index: $pos.index(d - 1) }
    }
  }
  return { container: DOC_CONTAINER, index: $pos.index(0) }
}

/** 最上位のブロックの DOM 要素(index 順) */
export const blockElements = (view: EditorView) => childElements(view, DOC_CONTAINER)

/**
 * 入れ物の中で、from 番目の子を to 番目に移す(1回の操作なので、取り消しで元に戻る)。
 * カーソルが動かした子の中にあれば、移した先の同じ場所に置く。
 */
export function moveChild(view: EditorView, c: Container, from: number, to: number): void {
  const { doc } = view.state
  const parent = containerNode(doc, c)
  if (!parent || from === to || from < 0 || to < 0 || from >= parent.childCount || to >= parent.childCount) return
  const node = parent.child(from)
  const start = childStarts(doc, c)[from]
  const end = start + node.nodeSize
  const cursor = view.state.selection.head
  const inside = cursor >= start && cursor <= end ? cursor - start : null

  const tr = view.state.tr.delete(start, end)
  // 消した後の文書での挿入位置(入れ物の位置は、中を消しても変わらない)
  let insertAt = c.contentStart
  containerNode(tr.doc, c)?.forEach((child, offset, index) => {
    if (index < to) insertAt = c.contentStart + offset + child.nodeSize
  })
  tr.insert(insertAt, node)
  if (inside !== null) tr.setSelection(TextSelection.near(tr.doc.resolve(Math.min(insertAt + inside, tr.doc.content.size))))
  view.dispatch(tr.scrollIntoView())
}

/** 最上位のブロックの並べ替え */
export const moveBlock = (view: EditorView, from: number, to: number) => moveChild(view, DOC_CONTAINER, from, to)

// ---- 並べ替えの動き(FLIP) ----

type Snapshot = Map<HTMLElement, DOMRect>

/** 入れ物の中の、いま見えている子の見た目の位置を控える(動いている最中なら、その途中の位置) */
export function snapshotChildren(view: EditorView, c: Container): Snapshot {
  const snap: Snapshot = new Map()
  for (const el of childElements(view, c)) {
    const r = el.getBoundingClientRect()
    if (r.bottom >= -200 && r.top <= window.innerHeight + 200) snap.set(el, r)
  }
  return snap
}

/**
 * 並べ替えた後、控えた位置から新しい位置へ滑らせる(ばねの動き)。
 * moved: 動かした子は DOM が作り直されるので、新しい要素と、動かす前の見た目の位置を渡す。
 */
export function settleChildren(view: EditorView, c: Container, before: Snapshot, moved?: { el: HTMLElement; rect: DOMRect }): void {
  const ms = duration('drag')
  const ease = easing('spring')
  const animate = (el: HTMLElement, old: DOMRect) => {
    const now = el.getBoundingClientRect()
    const dy = old.top - now.top
    const dx = old.left - now.left
    if ((Math.abs(dy) < 0.5 && Math.abs(dx) < 0.5) || !canAnimate(el)) return
    el.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'none' }], { duration: ms, easing: ease })
  }
  for (const el of childElements(view, c)) {
    if (moved && el === moved.el) animate(el, moved.rect)
    else {
      const old = before.get(el)
      if (old) animate(el, old)
    }
  }
}

/** 入れ物の中で、from 番目を to 番目へ動かし、周りを滑らせる */
export function moveChildAnimated(view: EditorView, c: Container, from: number, to: number): void {
  const before = snapshotChildren(view, c)
  const movedRect = childElements(view, c)[from]?.getBoundingClientRect()
  for (const el of before.keys()) cancelAnimations(el)
  moveChild(view, c, from, to)
  const newEl = childElements(view, c)[to]
  settleChildren(view, c, before, newEl && movedRect ? { el: newEl, rect: movedRect } : undefined)
}

/**
 * キーボードでの並べ替え(⌘⇧↑ / ⌘⇧↓)。
 * カーソルがリストの項目の中なら、その項目を同じリストの中で動かす(子の項目も一緒に)。
 * リストの端の項目をさらに外へ動かそうとしたときは、カーソルのある最上位のブロック(リスト全体)を動かす。
 */
export function moveCurrentBlock(view: EditorView, delta: -1 | 1): boolean {
  const { doc, selection } = view.state
  const unit = unitAt(doc, selection.head)
  const count = containerNode(doc, unit.container)?.childCount ?? 0
  const to = unit.index + delta
  if (to >= 0 && to < count) {
    moveChildAnimated(view, unit.container, unit.index, to)
    return true
  }
  if (unit.container.pos === -1) return true
  const blockIndex = doc.resolve(selection.head).index(0)
  const blockTo = blockIndex + delta
  if (blockTo >= 0 && blockTo < doc.childCount) moveChildAnimated(view, DOC_CONTAINER, blockIndex, blockTo)
  return true
}
