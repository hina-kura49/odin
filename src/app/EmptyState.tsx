import type { ReactNode } from 'react'

/** 空の状態・例外の状態(デザイン 6)。アイコン、見出し、説明、操作を中央に置く */
export function EmptyState({ icon, title, children, action }: { icon: ReactNode; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center px-6 py-24 text-center">
      <div className="mb-5 text-muted">{icon}</div>
      <p className="mb-2 text-base font-semibold">{title}</p>
      {children && <div className="text-sm leading-relaxed text-muted">{children}</div>}
      {action && <div className="mt-5">{action}</div>}
    </div>
  )
}
