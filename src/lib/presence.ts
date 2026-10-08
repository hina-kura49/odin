import { useLayoutEffect, useRef, useState } from 'react'
import { duration, easing, exitRatio, reducedMotion } from './motion'

type Kind = 'menu' | 'palette'

/** 動き(Web Animations)を使えるか。使えない環境(テストなど)では、すぐに出し入れする */
const ANIMATABLE = typeof Element !== 'undefined' && typeof Element.prototype.animate === 'function'

/**
 * メニューやパレットの出入り。
 * - 入るときは減速しながら現れ、出るときは短く消える(出る動きは入る動きの exit-ratio 倍)
 * - 途中で逆の操作が来たら、いまの見た目(不透明度と拡大率)から続けて動く
 * - 動いている間も中身は操作できる(入力を止めない)
 * - 「視差効果を減らす」が有効なら、拡大・移動はやめてフェードだけにする
 */
export function usePresence<T extends HTMLElement>(open: boolean, kind: Kind) {
  const [mounted, setMounted] = useState(open)
  const ref = useRef<T>(null)
  if (open && !mounted) setMounted(true)
  if (!open && mounted && !ANIMATABLE) setMounted(false)

  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    if (!ANIMATABLE) return
    const hidden = { opacity: 0, transform: reducedMotion() ? 'none' : kind === 'palette' ? 'scale(0.98)' : 'translateY(-4px) scale(0.98)' }
    const shown = { opacity: 1, transform: 'none' }
    // いまの見た目から始める(動いている最中なら、その途中から)
    const cs = getComputedStyle(el)
    const running = el.getAnimations().length > 0
    const from = running ? { opacity: Number(cs.opacity), transform: cs.transform === 'none' ? 'none' : cs.transform } : open ? hidden : shown
    for (const a of el.getAnimations()) a.cancel()
    const ms = duration(kind) * (open ? 1 : exitRatio())
    const anim = el.animate([from, open ? shown : hidden], { duration: ms, easing: easing(open ? 'enter' : 'exit'), fill: 'forwards' })
    if (!open) {
      anim.onfinish = () => setMounted(false)
      // 裏のタブなどで動きが進まないときも、必ず片づける
      const fallback = setTimeout(() => setMounted(false), ms + 100)
      return () => clearTimeout(fallback)
    } else
      anim.onfinish = () => {
        anim.commitStyles?.()
        anim.cancel()
      }
  }, [open, kind, mounted])

  return { mounted, ref }
}
