import type { EditorView } from '@milkdown/kit/prose/view'
import { blockElements, moveBlock, settleBlocks, snapshotBlocks } from './block-move'

/**
 * ハンドルをつまんでブロックを並べ替える。
 * - つまんだブロックはポインタに付いて動き、周りのブロックは滑って場所を空ける(transform とばねのトランジション)
 * - 離すと、つまんだブロックが新しい場所へ収まる。Esc で取りやめると元の場所へ戻る
 * - 開始時に位置を一度だけ読み、移動中は計算と、位置が変わるブロックへの transform の書き込みだけを行う
 *   (数百ブロックでも、毎フレームのレイアウト再計算を起こさない)
 */
export function startBlockDrag(view: EditorView, index: number, event: PointerEvent, scroller: HTMLElement): void {
  const els = blockElements(view)
  const dragged = els[index]
  if (!dragged) return
  const rects = els.map((el) => el.getBoundingClientRect())
  const own = rects[index]
  const nextTop = rects[index + 1]?.top ?? rects[index - 1]?.bottom
  const gap = index + 1 < rects.length ? nextTop - own.bottom : own.top - (nextTop ?? own.top)
  const step = own.height + Math.max(0, gap)
  const startY = event.clientY
  const startScroll = scroller.scrollTop
  const mids = rects.map((r) => r.top + r.height / 2)
  const shifts = new Array<number>(els.length).fill(0)
  let target = index
  let lastY = startY
  let raf = 0

  // ドロップ位置を示す線(デザイン 3「ドラッグ中」)
  const host = view.dom.parentElement ?? view.dom
  // 線の位置は、ブロックを包む要素の中での位置(開始時の座標で計算する。スクロールしても一緒に動く)
  const hostTop = host.getBoundingClientRect().top
  const line = document.createElement('div')
  line.className = 'block-drop-line'
  host.append(line)

  dragged.classList.add('block-dragging')
  for (const [i, el] of els.entries()) if (i !== index) el.classList.add('block-shifting')
  document.body.classList.add('block-drag-active')

  const place = (clientY: number) => {
    lastY = clientY
    const scrolled = scroller.scrollTop - startScroll
    const dy = clientY - startY + scrolled
    dragged.style.transform = `translateY(${dy}px)`
    const center = mids[index] + dy
    let up = 0
    let down = 0
    for (let i = 0; i < els.length; i++) {
      if (i === index) continue
      const want = i < index && center < mids[i] ? step : i > index && center > mids[i] ? -step : 0
      if (want > 0) up++
      if (want < 0) down++
      if (shifts[i] !== want) {
        shifts[i] = want
        els[i].style.transform = want ? `translateY(${want}px)` : ''
      }
    }
    target = index - up + down
    // 線は、空いた場所の上端に置く
    const lineY = target < index ? rects[target].top - Math.max(0, gap) / 2 : target > index ? rects[target].bottom + Math.max(0, gap) / 2 : own.top - Math.max(0, gap) / 2
    line.style.transform = `translateY(${lineY - hostTop}px)`
    line.style.opacity = target === index ? '0' : '1'
  }

  // 端に近づいたら自動でスクロールする
  const autoScroll = () => {
    const box = scroller.getBoundingClientRect()
    const edge = 56
    const speed = lastY < box.top + edge ? -(box.top + edge - lastY) / 3 : lastY > box.bottom - edge ? (lastY - (box.bottom - edge)) / 3 : 0
    if (speed !== 0) {
      scroller.scrollTop += speed
      place(lastY)
    }
    raf = requestAnimationFrame(autoScroll)
  }
  raf = requestAnimationFrame(autoScroll)

  const finish = (commit: boolean) => {
    cancelAnimationFrame(raf)
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onUp)
    window.removeEventListener('pointercancel', onCancel)
    window.removeEventListener('keydown', onKey, true)
    window.removeEventListener('blur', onCancel)
    // いまの見た目の位置を控えてから、transform を外して文書を並べ替え、控えた位置から滑らせる
    const before = snapshotBlocks(view)
    const draggedRect = dragged.getBoundingClientRect()
    for (const el of els) {
      el.classList.remove('block-dragging', 'block-shifting')
      el.style.transform = ''
    }
    line.remove()
    document.body.classList.remove('block-drag-active')
    if (commit && target !== index) {
      moveBlock(view, index, target)
      const movedEl = blockElements(view)[target]
      settleBlocks(view, before, movedEl ? { el: movedEl, rect: draggedRect } : undefined)
    } else {
      settleBlocks(view, before, { el: dragged, rect: draggedRect })
    }
    view.focus()
  }
  const onMove = (e: PointerEvent) => place(e.clientY)
  const onUp = () => finish(true)
  const onCancel = () => finish(false)
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      finish(false)
    }
  }
  window.addEventListener('pointermove', onMove)
  window.addEventListener('pointerup', onUp)
  window.addEventListener('pointercancel', onCancel)
  window.addEventListener('keydown', onKey, true)
  // ドラッグ中にほかのアプリへ切り替えたら(離す操作が届かないので)取りやめる
  window.addEventListener('blur', onCancel)
  place(startY)
}
