import { flushSync } from 'react-dom'
import { cancelAnimations, duration, easing, fadeIn, reducedMotion } from './motion'

/**
 * 一覧の行を出し入れするときの動き(ツリーの開閉)。
 * 高さを毎フレーム変えるとレイアウトの再計算が続くので、レイアウトは一度に切り替え、
 * 画面に見えている行だけを、元の位置から新しい位置へ transform で滑らせる。新しく現れた行はフェードで入れる。
 * 長いリストでも、動かすのは見えている数十行だけ。
 * 行は data-row 属性の値(パスなど)で見分ける。仮想リストで要素が作り直されても、同じ行として追える。
 */
export function flipRows(container: HTMLElement | null, mutate: () => void): void {
  if (!container) {
    mutate()
    return
  }
  const view = container.getBoundingClientRect()
  const visible = (r: DOMRect) => r.bottom >= view.top && r.top <= view.bottom
  const rows = () => Array.from(container.querySelectorAll<HTMLElement>('[data-row]'))

  // 変更前の見た目の位置(動いている最中なら、その途中の位置)
  const before = new Map<string, DOMRect>()
  for (const row of rows()) {
    const r = row.getBoundingClientRect()
    if (visible(r)) before.set(row.dataset.row ?? '', r)
  }

  flushSync(mutate)

  const ms = duration('tree')
  const after = rows()
  after.forEach(cancelAnimations)
  for (const row of after) {
    const r = row.getBoundingClientRect()
    if (!visible(r)) continue
    const old = before.get(row.dataset.row ?? '')
    if (!old) {
      fadeIn(row, { duration: ms, offsetY: -4 })
    } else if (Math.abs(old.top - r.top) >= 0.5 && !reducedMotion()) {
      row.animate([{ transform: `translateY(${old.top - r.top}px)` }, { transform: 'none' }], {
        duration: ms,
        easing: easing('move'),
      })
    }
  }
}
