import { flushSync } from 'react-dom'
import { create } from 'zustand'
import { useApp } from '@/store/app'
import { PaletteSearch, recentResults, type PaletteResults } from './palette-search'

type PaletteState = {
  open: boolean
  /** 入力欄の文字(変換中の文字を含む) */
  query: string
  /** 表示している結果。検索の結果が届くまでは直前のまま */
  results: PaletteResults
  /** 選んでいる項目(cmdk の value) */
  selected: string
}

export const usePalette = create<PaletteState>()(() => ({
  open: false,
  query: '',
  results: { kind: 'recent', recent: [] },
  selected: '',
}))

/** 結果の各項目の値。種類ごとに分けて、同じページが別の段に出ても区別する */
export const itemValues = (r: PaletteResults): string[] =>
  r.kind === 'recent'
    ? r.recent.map((p) => `recent:${p.path}`)
    : [...r.pages.map((p) => `page:${p.path}`), ...r.hits.map((h) => `hit:${h.path}`), ...r.commands.map((c) => `cmd:${c.id}`)]

const showResults = (results: PaletteResults) =>
  usePalette.setState((s) => {
    const values = itemValues(results)
    // 選んでいる項目が新しい結果にもあれば、そのまま選んでおく
    return { results, selected: values.includes(s.selected) ? s.selected : (values[0] ?? '') }
  })

export const search = new PaletteSearch(() => useApp.getState().tree, showResults)

let inputEl: HTMLInputElement | null = null
export const setPaletteInput = (el: HTMLInputElement | null) => {
  inputEl = el
}
let returnFocus: HTMLElement | null = null

/**
 * パレットを開く。
 * 開くキーの処理の中で描画まで済ませて入力欄に入力位置を移すので、続けて打った文字は(出現の動きの途中でも)すべて入力欄に入る。
 */
export function openPalette(): void {
  const app = useApp.getState()
  if (app.status !== 'ready' || usePalette.getState().open) return
  returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  const results = recentResults(app.tree, app.recent)
  flushSync(() => usePalette.setState({ open: true, query: '', results, selected: itemValues(results)[0] ?? '' }))
  inputEl?.focus()
  // 手元の「最近開いたページ」をすぐに見せ、バックエンドの最新の一覧が届いたら差し替える
  search.reset()
  search.run('')
}

export function closePalette(): void {
  if (!usePalette.getState().open) return
  usePalette.setState({ open: false })
  search.reset()
  if (returnFocus?.isConnected) returnFocus.focus()
  returnFocus = null
}

export const togglePalette = () => (usePalette.getState().open ? closePalette() : openPalette())
