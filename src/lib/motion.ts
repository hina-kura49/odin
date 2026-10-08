// アニメーションの共通処理。長さとイージングは CSS 変数(index.css)から読む。
// 動かすのは transform と opacity だけ。動いている最中に次の操作が来たら、今の位置から続けて動く。

const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(`--${name}`).trim()

/** CSS 変数の長さ(例: --duration-sidebar)をミリ秒で返す */
export function duration(name: string): number {
  const value = css(`duration-${name}`)
  const n = Number.parseFloat(value)
  if (!Number.isFinite(n)) return 0
  return value.endsWith('ms') ? n : n * 1000
}

export const easing = (name: 'enter' | 'exit' | 'move' | 'spring') => css(`ease-${name}`) || 'ease'

export const exitRatio = () => Number.parseFloat(css('exit-ratio')) || 0.6

/** 「視差効果を減らす」が有効か。有効なら位置の動きをやめ、短いフェードだけにする */
export const reducedMotion = () => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false

/** アニメーションを使えるか(テスト用の環境などでは使えないので、動きなしで進める) */
export const canAnimate = (el: Element): boolean => typeof el.animate === 'function' && !reducedMotion()

/** 要素で動いているアニメーションを止める(止める前の見た目の位置は getBoundingClientRect で読んでおく) */
export function cancelAnimations(el: Element): void {
  if (typeof el.getAnimations !== 'function') return
  for (const a of el.getAnimations()) a.cancel()
}

/**
 * FLIP: レイアウトは一度に切り替え、見た目だけを元の位置から新しい位置へ transform で滑らせる。
 * before はレイアウト変更前の見た目の位置(動いている最中なら、その途中の位置)。
 */
export function slideFrom(el: HTMLElement, before: DOMRect, opts: { duration: number; easing: string; axis?: 'x' | 'y' }): void {
  cancelAnimations(el)
  const after = el.getBoundingClientRect()
  const dx = opts.axis === 'y' ? 0 : before.left - after.left
  const dy = opts.axis === 'x' ? 0 : before.top - after.top
  if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) return
  if (!canAnimate(el)) return
  el.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'none' }], {
    duration: opts.duration,
    easing: opts.easing,
  })
}

/** 現れる要素: 短いフェードと、わずかな移動(視差効果を減らす設定ではフェードだけ) */
export function fadeIn(el: HTMLElement, opts: { duration: number; offsetY?: number; scale?: number }): void {
  cancelAnimations(el)
  if (typeof el.animate !== 'function') return
  const from = reducedMotion() ? 'none' : `translateY(${opts.offsetY ?? 0}px) scale(${opts.scale ?? 1})`
  el.animate([{ opacity: 0, transform: from }, { opacity: 1, transform: 'none' }], {
    duration: opts.duration,
    easing: easing('enter'),
  })
}
