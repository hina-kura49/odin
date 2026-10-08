// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { setBackend, type SearchHit } from '@/backend'
import { MockBackend } from '@/backend/mock'
import { useApp } from '@/store/app'
import { CommandPalette } from '../CommandPalette'
import { closePalette, openPalette } from '../palette-store'
import { PaletteSearch, type PaletteResults } from '../palette-search'

// cmdk が使う、jsdom にない API
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
}
Element.prototype.scrollIntoView ??= function () {}

/** 検索の返事を、テストの好きな順番で返せる Mock */
class ControlledBackend extends MockBackend {
  pending: { query: string; resolve: (hits: SearchHit[]) => void; reject: (e: unknown) => void }[] = []
  override search(query: string): Promise<SearchHit[]> {
    return new Promise((resolve, reject) => this.pending.push({ query, resolve, reject }))
  }
  answer(query: string, hits: SearchHit[]) {
    const i = this.pending.findIndex((p) => p.query === query)
    this.pending.splice(i, 1)[0].resolve(hits)
  }
}

const hit = (path: string, title: string, before: string, h: string, after: string): SearchHit => ({ path, title, snippet: { before, hit: h, after } })
const flush = () => act(async () => {})

let mock: ControlledBackend
const initialState = useApp.getState()

beforeEach(async () => {
  mock = new ControlledBackend()
  setBackend(mock)
  useApp.setState(initialState, true)
  await useApp.getState().init()
})

afterEach(() => {
  closePalette()
  cleanup()
})

describe('PaletteSearch(検索の順番)', () => {
  const setup = () => {
    const seen: PaletteResults[] = []
    const s = new PaletteSearch(() => useApp.getState().tree, (r) => seen.push(r))
    return { s, seen }
  }

  it('古い検索の結果が後から届いても、最後の検索語の結果だけを知らせる', async () => {
    const { s, seen } = setup()
    s.run('プ')
    s.run('プロダクト')
    mock.answer('プロダクト', [hit('a.md', 'A', '新しい', 'プロダクト', 'の')])
    mock.answer('プ', [hit('b.md', 'B', '', 'プ', '')])
    await flush()
    expect(seen).toHaveLength(1)
    expect(seen[0]).toMatchObject({ kind: 'search', query: 'プロダクト' })
  })

  it('結果が届くまでは何も知らせない(表示は直前のまま)。失敗しても知らせない', async () => {
    const { s, seen } = setup()
    s.run('メモ')
    await flush()
    expect(seen).toHaveLength(0)
    mock.pending[0].reject(new Error('io'))
    await flush()
    expect(seen).toHaveLength(0)
  })

  it('空の入力では recentPages の結果を出す', async () => {
    const { s, seen } = setup()
    s.run('')
    await flush()
    expect(seen[0].kind).toBe('recent')
    if (seen[0].kind === 'recent') expect(seen[0].recent[0].path).toBe((await mock.recentPages(10))[0].path)
  })

  it('タイトルに当たったページと、本文に当たったページを分け、本文の前後はそのまま渡す', async () => {
    const { s, seen } = setup()
    s.run('プロダクト')
    mock.answer('プロダクト', [
      hit('仕事/企画/新しいプロダクトの考え方.md', '新しいプロダクトの考え方', '', 'プロダクト', ''),
      hit('学び/デザイン/デザインのメモ.md', 'デザインのメモ', '…シンプルな ', 'プロダクト', ' を目指して'),
    ])
    await flush()
    const r = seen[0]
    if (r.kind !== 'search') throw new Error('search の結果のはず')
    expect(r.pages.map((p) => p.path)).toContain('仕事/企画/新しいプロダクトの考え方.md')
    expect(r.pages.find((p) => p.path === '仕事/企画/新しいプロダクトの考え方.md')?.where).toBe('仕事 / 企画')
    expect(r.hits).toEqual([hit('学び/デザイン/デザインのメモ.md', 'デザインのメモ', '…シンプルな ', 'プロダクト', ' を目指して')])
  })
})

describe('コマンドパレット', () => {
  const input = () => screen.getByPlaceholderText('ページ・本文・コマンドを検索') as HTMLInputElement

  it('開くキーの処理が終わった時点で、入力欄に入力位置がある(出現の動きの途中に打った文字も取りこぼさない)', () => {
    render(<CommandPalette />)
    act(() => openPalette())
    expect(document.activeElement).toBe(input())
  })

  it('入力が空なら最近開いたページを出し、⌘1 で1件目を開く', async () => {
    render(<CommandPalette />)
    act(() => openPalette())
    await flush()
    expect(screen.getByText('最近開いたページ')).toBeTruthy()
    const first = useApp.getState().recent[0]
    const open = vi.spyOn(useApp.getState(), 'openPage')
    fireEvent.keyDown(input(), { key: '1', metaKey: true })
    expect(open).toHaveBeenCalledWith(first.path)
  })

  it('変換中は検索せず、確定した時点で検索する。変換確定の Enter では開かない', async () => {
    render(<CommandPalette />)
    act(() => openPalette())
    await flush()
    const search = vi.spyOn(mock, 'search')
    const open = vi.spyOn(useApp.getState(), 'openPage')
    fireEvent.compositionStart(input())
    fireEvent.change(input(), { target: { value: 'ぷろ' } })
    expect(search).not.toHaveBeenCalled()
    // 変換中の表示は、直前の結果(最近開いたページ)のまま
    expect(screen.getByText('最近開いたページ')).toBeTruthy()
    fireEvent.change(input(), { target: { value: 'プロ' } })
    fireEvent.keyDown(input(), { key: 'Enter', isComposing: true, keyCode: 229 })
    expect(open).not.toHaveBeenCalled()
    fireEvent.compositionEnd(input())
    expect(search).toHaveBeenCalledTimes(1)
    expect(search).toHaveBeenCalledWith('プロ', 50)
  })

  it('結果が届くまで直前の結果を残し、届いたら最後の検索語の結果に差し替える。Enter で選んだページを開く', async () => {
    render(<CommandPalette />)
    act(() => openPalette())
    await flush()
    fireEvent.change(input(), { target: { value: 'デ' } })
    fireEvent.change(input(), { target: { value: 'デザ' } })
    expect(screen.getByText('最近開いたページ')).toBeTruthy()
    await act(async () => mock.answer('デザ', [hit('仕事/企画/新しいプロダクトの考え方.md', '新しいプロダクトの考え方', '', 'デザ', 'イン')]))
    expect(screen.queryByText('最近開いたページ')).toBeNull()
    expect(screen.getByText('本文の検索結果')).toBeTruthy()
    // 古い検索の返事が後から届いても、表示は変わらない
    await act(async () => mock.answer('デ', []))
    expect(screen.getByText('本文の検索結果')).toBeTruthy()
    const open = vi.spyOn(useApp.getState(), 'openPage')
    fireEvent.keyDown(input(), { key: 'Enter' })
    expect(open).toHaveBeenCalledTimes(1)
  })

  it('当たったものがなければ「見つからない」を出す', async () => {
    render(<CommandPalette />)
    act(() => openPalette())
    fireEvent.change(input(), { target: { value: 'zzzz' } })
    await act(async () => mock.answer('zzzz', []))
    expect(screen.getByText('「zzzz」に関するページ・検索結果・コマンドはありません')).toBeTruthy()
  })

  it('Esc で閉じ、開く前の場所へ入力位置を戻す', async () => {
    const before = document.createElement('button')
    document.body.append(before)
    before.focus()
    render(<CommandPalette />)
    act(() => openPalette())
    fireEvent.keyDown(input(), { key: 'Escape' })
    expect(document.activeElement).toBe(before)
    before.remove()
  })
})
