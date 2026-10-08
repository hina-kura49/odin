// @vitest-environment jsdom
import { Editor, editorViewCtx } from '@milkdown/kit/core'
import { EditorState } from '@milkdown/kit/prose/state'
import type { EditorView } from '@milkdown/kit/prose/view'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { setBackend, toBackendError } from '@/backend'
import { MockBackend } from '@/backend/mock'
import { useApp } from '@/store/app'
import { configureMarkdown } from '../markdown'
import { setActiveSession } from '../registry'
import { EditorSession } from '../session'

const DELAY = 20
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))
const initialState = useApp.getState()

let mock: MockBackend
let editor: Editor
let session: EditorSession
let view: EditorView
let composing = false

/** エディタでページを開いたのと同じ状態にする */
async function open(path: string) {
  const { content, version } = await mock.readPage(path)
  const doc = session.load(path, content, version)
  view.updateState(EditorState.create({ doc, plugins: view.state.plugins }))
  useApp.setState({ page: { path, content, version, loadId: Date.now() }, selectedPath: path })
}

/** 本文の先頭に打ち込む(実際のエディタではプラグインが markEdited を呼ぶ) */
function type(text: string) {
  view.dispatch(view.state.tr.insertText(text, 1))
  session.markEdited()
}

const raw = async (path: string) => (await mock.readPage(path)).content
const notice = () => useApp.getState().notice

beforeEach(async () => {
  mock = new MockBackend()
  setBackend(mock)
  useApp.setState(initialState, true)
  await useApp.getState().refreshTree()
  editor = await configureMarkdown(Editor.make()).create()
  view = editor.action((ctx) => ctx.get(editorViewCtx))
  composing = false
  Object.defineProperty(view, 'composing', { get: () => composing, configurable: true })
  session = new EditorSession(editor, DELAY)
  setActiveSession(session)
})

afterEach(async () => {
  setActiveSession(null)
  await editor.destroy()
})

describe('自動保存', () => {
  it('最後の入力から一定時間後に writePage を呼び、未保存の変更がなくなる', async () => {
    await open('日記/2025/今日の振り返り.md')
    type('追記')
    expect(session.isDirty()).toBe(true)
    await wait(DELAY + 30)
    expect(await raw('日記/2025/今日の振り返り.md')).toContain('追記今日はよく書けた。')
    expect(session.isDirty()).toBe(false)
  })

  it('編集していなければ書かない(version が変わらない)', async () => {
    await open('日記/2025/今日の振り返り.md')
    const before = (await mock.readPage('日記/2025/今日の振り返り.md')).version
    await session.flush()
    expect((await mock.readPage('日記/2025/今日の振り返り.md')).version).toBe(before)
  })

  it('BOM は外して表示し、保存時に付け直す', async () => {
    await open('検証/BOMつき.md')
    expect(view.state.doc.textContent.startsWith('﻿')).toBe(false)
    type('追記')
    await session.flush()
    const saved = await raw('検証/BOMつき.md')
    expect(saved.startsWith('﻿追記ファイルの先頭に')).toBe(true)
  })

  it('IME の変換中は保存せず、変換が終わってから保存する', async () => {
    await open('日記/2025/今日の振り返り.md')
    composing = true
    type('へんかんちゅう')
    await session.flush()
    expect(await raw('日記/2025/今日の振り返り.md')).not.toContain('へんかんちゅう')
    composing = false
    session.compositionEnded()
    await session.flush()
    expect(await raw('日記/2025/今日の振り返り.md')).toContain('へんかんちゅう')
  })
})

describe('外部の変更(onExternalChange)', () => {
  it('未保存の変更がなければ、黙って読み直す', async () => {
    await open('日記/2025/今日の振り返り.md')
    const loadId = useApp.getState().page?.loadId
    mock.simulateExternalEdit('日記/2025/今日の振り返り.md', '他のアプリで書き換えた。\n')
    await useApp.getState().handleExternalChange()
    const page = useApp.getState().page
    expect(page && !page.unreadable && page.content).toBe('他のアプリで書き換えた。\n')
    expect(page?.loadId).not.toBe(loadId)
    expect(notice()).toBeNull()
  })

  it('未保存の変更があれば、通知で尋ねる', async () => {
    await open('日記/2025/今日の振り返り.md')
    type('自分の変更')
    mock.simulateExternalEdit('日記/2025/今日の振り返り.md', '他のアプリで書き換えた。\n')
    await useApp.getState().handleExternalChange()
    expect(notice()?.title).toBe('このファイルは、他のアプリで変更されました。')
    expect(notice()?.actions.map((a) => a.label)).toEqual(['再読み込み', '自分の変更を保持'])
  })

  it('「自分の変更を保持」で、最新の version を基準に自分の内容で上書きする', async () => {
    await open('日記/2025/今日の振り返り.md')
    type('自分の変更')
    mock.simulateExternalEdit('日記/2025/今日の振り返り.md', '他のアプリで書き換えた。\n')
    await useApp.getState().handleExternalChange()
    await notice()?.actions[1].run()
    expect(await raw('日記/2025/今日の振り返り.md')).toContain('自分の変更')
    expect(session.isDirty()).toBe(false)
  })
})

describe('保存できなかったとき', () => {
  it('notFound: 内容を保持したまま「作り直す / 破棄する」を尋ね、作り直すと同じ場所に内容が書かれる', async () => {
    await open('日記/2025/今日の振り返り.md')
    type('消される前の追記')
    mock.simulateExternalDelete('日記/2025/今日の振り返り.md')
    await session.flush()
    expect(notice()?.actions.map((a) => a.label)).toEqual(['作り直す', '破棄する'])
    expect(session.isDirty()).toBe(true)
    await notice()?.actions[0].run()
    expect(await raw('日記/2025/今日の振り返り.md')).toContain('消される前の追記')
  })

  it('readOnly: 内容を保持したまま通知する', async () => {
    await open('検証/読み取り専用.md')
    type('書けない追記')
    await session.flush()
    expect(notice()?.title).toContain('読み取り専用')
    expect(session.isDirty()).toBe(true)
    expect(view.state.doc.textContent).toContain('書けない追記')
  })
})

describe('ページの操作', () => {
  it('notUtf8 のページはエディタを出さない状態になる', async () => {
    await useApp.getState().openPage('検証/文字コードが違うファイル.md')
    expect(useApp.getState().page?.unreadable).toBe('notUtf8')
  })

  it('createPage / renamePage / deletePage の前に、未保存の変更を保存し終える', async () => {
    await open('日記/2025/今日の振り返り.md')
    type('保存してから作る')
    await useApp.getState().createPage('日記', '新しいページ')
    expect(await raw('日記/2025/今日の振り返り.md')).toContain('保存してから作る')
  })

  it('renamePage が nameOccupied を返したら通知し、ツリーのタイトルは元のまま', async () => {
    await open('仕事/企画/新しいプロダクトの考え方.md')
    const result = await useApp.getState().renamePage('仕事/企画/新しいプロダクトの考え方.md', 'プロダクトの方向性')
    expect(result).toBeNull()
    expect(notice()?.title).toBe('同じ名前のページがあります')
    const titles = JSON.stringify(useApp.getState().tree)
    expect(titles).toContain('新しいプロダクトの考え方')
  })

  it('renamePage の後は、開いているページを読み直す(version が変わる)', async () => {
    await open('仕事/企画/新しいプロダクトの考え方.md')
    const before = useApp.getState().page
    const meta = await useApp.getState().renamePage('仕事/企画/新しいプロダクトの考え方.md', '  プロダクトの  考え方 ')
    expect(meta?.title).toBe('プロダクトの 考え方') // バックエンドが整えたタイトルを正とする
    const after = useApp.getState().page
    expect(after?.path).toBe('仕事/企画/プロダクトの 考え方.md')
    expect(after && !after.unreadable && before && !before.unreadable && after.version !== before.version).toBe(true)
  })

  it('deletePage の直後に「ゴミ箱に移動しました」と知らせる', async () => {
    await open('日記/2025/今日の振り返り.md')
    await useApp.getState().deletePage('日記/2025/今日の振り返り.md')
    expect(notice()?.title).toBe('ゴミ箱に移動しました')
    // 開いていたページを消したら、最近開いたページを開く(本文を空にしない)
    expect(useApp.getState().page?.path).not.toBe('日記/2025/今日の振り返り.md')
    expect(useApp.getState().page).not.toBeNull()
  })
})

describe('契約の約束ごと', () => {
  it('知らないエラーの種類が来ても落ちない(io として扱う)', () => {
    const err = toBackendError({ kind: 'somethingNew', message: '新しい種類' })
    expect(err.kind).toBe('io')
    expect(err.rawKind).toBe('somethingNew')
  })

  it('「:」を含む path をそのまま使える', async () => {
    await open('仕事/会議メモ/10:00 打ち合わせ.md')
    type('追記')
    await session.flush()
    expect(await raw('仕事/会議メモ/10:00 打ち合わせ.md')).toContain('追記')
  })
})
