import { useEffect, useLayoutEffect, useRef } from 'react'

/**
 * IME の変換中か。WKWebView では変換確定の Enter が isComposing=false・keyCode=229 で届くことがあるので両方見る。
 * React のイベントは e.nativeEvent を渡す。
 */
export const isComposing = (e: KeyboardEvent) => e.isComposing || e.keyCode === 229

/** 「⌘\」のような組み合わせ。mod は macOS の ⌘ */
export type Combo = { key: string; mod?: boolean; shift?: boolean; alt?: boolean }

export const matchesCombo = (e: KeyboardEvent, c: Combo) =>
  e.key.toLowerCase() === c.key.toLowerCase() &&
  e.metaKey === (c.mod ?? false) &&
  e.shiftKey === (c.shift ?? false) &&
  e.altKey === (c.alt ?? false)

/** アプリ全体のショートカット。IME の変換中は反応しない */
export function useShortcut(combo: Combo, handler: () => void, enabled = true): void {
  const latest = useRef(handler)
  useLayoutEffect(() => {
    latest.current = handler
  })
  const { key, mod, shift, alt } = combo
  useEffect(() => {
    if (!enabled) return
    const onKeyDown = (e: KeyboardEvent) => {
      if (isComposing(e) || !matchesCombo(e, { key, mod, shift, alt })) return
      e.preventDefault()
      latest.current()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [key, mod, shift, alt, enabled])
}
