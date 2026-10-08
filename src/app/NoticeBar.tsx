import { Check, X } from 'lucide-react'
import { useLayoutEffect, useRef } from 'react'
import { useApp } from '@/store/app'

/** 画面上部の通知(デザイン 7「外部状態の通知」)。尋ねる通知(warning)と、知らせるだけの通知(info)を同じ形で出す */
export function NoticeBar() {
  const notice = useApp((s) => s.notice)
  const dismiss = useApp((s) => s.dismissNotice)
  const ref = useRef<HTMLDivElement>(null)
  const id = notice?.id
  const wantsFocus = notice?.focus ?? false

  // 確認の通知は、出たら最初のボタンへ入力位置を移し、閉じたら元の場所へ戻す
  useLayoutEffect(() => {
    const el = ref.current
    if (!wantsFocus || !el) return
    const returnTo = document.activeElement instanceof HTMLElement ? document.activeElement : null
    el.querySelector<HTMLButtonElement>('button')?.focus()
    return () => {
      const now = document.activeElement
      if (returnTo?.isConnected && (now === null || now === document.body || el.contains(now))) returnTo.focus()
    }
  }, [id, wantsFocus])

  if (!notice) return null
  const warning = notice.tone === 'warning'

  return (
    <div
      ref={ref}
      role={warning ? 'alertdialog' : 'status'}
      aria-live="polite"
      className="notice mx-auto flex max-w-[var(--measure)] items-center gap-3 rounded-lg border px-4 py-3 shadow-sm"
      data-tone={notice.tone}
      onKeyDown={(e) => {
        if (e.key === 'Escape' && !e.nativeEvent.isComposing) {
          e.preventDefault()
          dismiss(notice.id)
        }
      }}
    >
      <span className="notice-icon flex size-7 shrink-0 items-center justify-center rounded-full text-white">
        {warning ? <span className="text-base font-bold leading-none">!</span> : <Check size={16} />}
      </span>
      <div className="min-w-0 flex-1 text-sm leading-relaxed">
        <p className="font-semibold">{notice.title}</p>
        {notice.body && <p className="text-muted">{notice.body}</p>}
      </div>
      {notice.actions.map((a) => (
        <button
          key={a.label}
          type="button"
          className="hover-fade shrink-0 rounded-md border border-border bg-panel px-3 py-1.5 text-sm hover:bg-bg"
          onClick={() => {
            dismiss(notice.id)
            void a.run()
          }}
        >
          {a.label}
        </button>
      ))}
      <button type="button" aria-label="閉じる" className="hover-fade shrink-0 rounded-sm p-1 text-muted hover:bg-border/60" onClick={() => dismiss(notice.id)}>
        <X size={16} />
      </button>
    </div>
  )
}
