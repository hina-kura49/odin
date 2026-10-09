import { useEffect, useRef, useState } from 'react'
import { flushSync } from 'react-dom'
import { backend } from '@/backend'
import { errorText } from '@/lib/error-text'
import { isComposing } from '@/lib/keyboard'
import { hideCurrentWindow } from '@/lib/platform'

/**
 * クイックキャプチャのウィンドウ(デザイン 5)。グローバルホットキーで画面中央に出る、タイトルバーのない小さなウィンドウ。
 * - Enter で Inbox に取り込んで閉じる。Shift+Enter で改行。Esc で閉じる。空のまま Enter なら何もせず閉じる
 * - 日本語の変換確定の Enter では取り込まない
 * - ウィンドウは閉じずに隠すだけなので、次に出たときは入力欄に入力位置があり、待たずに打てる
 */
export function CaptureWindow() {
  const ref = useRef<HTMLTextAreaElement>(null)
  const [text, setText] = useState('')
  const [error, setError] = useState<string | null>(null)

  // ウィンドウが前に出たら、入力欄に入力位置を置く(隠している間も入力位置は入力欄に残っている)
  useEffect(() => {
    const focus = () => ref.current?.focus()
    focus()
    window.addEventListener('focus', focus)
    return () => window.removeEventListener('focus', focus)
  }, [])

  /** 入力欄を空にしてから隠す(次に出たときに前の文字が一瞬見えないように) */
  const close = () => {
    flushSync(() => {
      setText('')
      setError(null)
    })
    hideCurrentWindow()
  }

  const capture = async (value: string) => {
    close()
    try {
      await backend().captureToInbox(value)
    } catch (e) {
      // 失敗したら文字を戻して知らせる(取り込めなかった文字を失わない)
      setText(value)
      setError(`取り込めませんでした。${errorText('capture', e)}`)
    }
  }

  return (
    <div className="flex h-full flex-col gap-2 rounded-xl border border-border bg-panel p-3" data-tauri-drag-region>
      <textarea
        ref={ref}
        autoFocus
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (isComposing(e.nativeEvent)) return
          if (e.key === 'Escape') {
            e.preventDefault()
            close()
          } else if (e.key === 'Enter' && !e.shiftKey && !e.metaKey && !e.altKey && !e.ctrlKey) {
            e.preventDefault()
            const value = e.currentTarget.value
            if (value.trim() === '') close()
            else void capture(value)
          }
        }}
        placeholder="今の考えを、ひとこと。"
        aria-label="クイックキャプチャ"
        className="min-h-0 flex-1 resize-none rounded-md border border-border bg-bg px-3 py-2 text-lg leading-relaxed outline-none placeholder:text-muted"
      />
      <p className="px-1 text-sm text-muted" data-tauri-drag-region>
        {error ?? 'Enter で保存して閉じる'}
      </p>
    </div>
  )
}
