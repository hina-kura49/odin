// いま表示しているエディタの保存の窓口。ストアはここを通して「保存し終える」を待つ(エディタとストアの循環参照を避ける)。

export interface ActiveSession {
  /** 未保存の変更を今すぐ書き出し、書き終えるまで待つ。書き出す内容はこの呼び出しの時点で決まる */
  flush(): Promise<void>
  /** 未保存の変更があるか */
  isDirty(): boolean
  /** 表示しているページの path と、エディタが知っている最新の version */
  current(): { path: string; version: string } | null
  /** 自分の変更で上書きする: 保存の基準を、ディスクの最新の version にしてから保存する */
  overwriteWith(version: string): Promise<void>
}

let active: ActiveSession | null = null

export const setActiveSession = (s: ActiveSession | null) => {
  active = s
}
export const activeSession = () => active

/** 未保存の変更を保存し終える(エディタがなければ何もしない) */
export const flushActive = () => active?.flush() ?? Promise.resolve()

// ---- タイトルと本文の行き来 ----

let focusEditorStart: (() => void) | null = null

/** エディタが「本文の先頭に入力位置を置く」処理を登録する */
export const setEditorStartFocuser = (fn: (() => void) | null) => {
  focusEditorStart = fn
}

/** 本文の先頭に入力位置を置く(タイトルで Enter / ↓ を押したとき) */
export const focusEditor = () => focusEditorStart?.()
