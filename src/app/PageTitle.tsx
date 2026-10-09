import { useLayoutEffect, useRef } from 'react'
import { focusEditor } from '@/editor/registry'
import { isComposing } from '@/lib/keyboard'
import { useApp } from '@/store/app'

/**
 * ページのタイトル。その場で書き換えると renamePage を呼ぶ。
 * - Enter: 決定して本文の先頭へ / Esc: 取り消して本文へ / 入力位置が外れたら決定
 * - IME の変換中の Enter と Esc には反応しない
 * - 表示するのはバックエンドが返したタイトル(ツリーのもの)。改名できなかったときは元に戻る
 */
export function PageTitle({ path, title, visible }: { path: string; title: string; visible: boolean }) {
  const ref = useRef<HTMLHeadingElement>(null)
  // 決定(Enter)と、入力位置が外れたときの決定が重ならないように
  const settled = useRef(false)
  const renamePage = useApp((s) => s.renamePage)
  const titleFocus = useApp((s) => s.titleFocus)

  // 書き換えている途中でなければ、バックエンドのタイトルを表示する
  useLayoutEffect(() => {
    const el = ref.current
    if (el && document.activeElement !== el) el.textContent = title
  }, [title, path])

  // ページを作った直後などに、タイトルへ入力位置を移す。
  // 最初のエディタの準備ができるまでは隠している(隠れた要素には入力位置を移せない)ので、出てから移す
  useLayoutEffect(() => {
    const el = ref.current
    if (!el || !visible || titleFocus?.path !== path) return
    settled.current = false
    el.focus()
    const range = document.createRange()
    range.selectNodeContents(el)
    if (!titleFocus.select) range.collapse(false)
    const sel = window.getSelection()
    sel?.removeAllRanges()
    sel?.addRange(range)
  }, [titleFocus, path, visible])

  const revert = () => {
    if (ref.current) ref.current.textContent = title
  }

  const commit = async () => {
    const el = ref.current
    if (!el || settled.current) return
    settled.current = true
    const next = (el.textContent ?? '').replace(/\s+/g, ' ').trim()
    if (!next || next === title) {
      revert()
      return
    }
    const meta = await renamePage(path, next)
    // 改名できなかったとき(名前が重なったなど)は、元のタイトルに戻す。できたときはバックエンドが整えたタイトルになる
    if (!meta) revert()
  }

  return (
    <h1
      ref={ref}
      className="page-title outline-none"
      contentEditable="plaintext-only"
      suppressContentEditableWarning
      role="textbox"
      aria-label="ページのタイトル"
      spellCheck={false}
      onKeyDown={(e) => {
        if (isComposing(e.nativeEvent)) return
        if (e.key === 'Enter' || (e.key === 'ArrowDown' && !e.shiftKey)) {
          e.preventDefault()
          // blur のイベントに頼らず、その場で決定する(ウィンドウが裏にあると blur が届かないことがある)
          void commit()
          focusEditor()
        } else if (e.key === 'Escape') {
          e.preventDefault()
          settled.current = true
          revert()
          focusEditor()
        }
      }}
      onFocus={() => {
        settled.current = false
      }}
      onInput={() => {
        settled.current = false
      }}
      onBlur={() => void commit()}
    />
  )
}
