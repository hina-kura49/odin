// @vitest-environment jsdom
import { Editor, editorViewCtx } from '@milkdown/kit/core'
import { history, undoCommand } from '@milkdown/kit/plugin/history'
import { EditorState, TextSelection } from '@milkdown/kit/prose/state'
import type { EditorView } from '@milkdown/kit/prose/view'
import { callCommand } from '@milkdown/kit/utils'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { moveBlock } from '../block-move'
import { configureMarkdown, loadMarkdown, serializeMarkdown } from '../markdown'
import { filterSlashItems, pageTitleFromQuery, runBlockCommand, slashKey, slashPlugin, SLASH_ITEMS } from '../slash'

let editor: Editor
let view: EditorView

beforeEach(async () => {
  editor = await configureMarkdown(Editor.make()).use(history).use(slashPlugin).create()
  view = editor.action((ctx) => ctx.get(editorViewCtx))
})
afterEach(async () => {
  await editor.destroy()
})

function setDoc(markdown: string) {
  const { doc, snapshot } = editor.action((ctx) => loadMarkdown(ctx, markdown))
  view.updateState(EditorState.create({ doc, plugins: view.state.plugins }))
  return snapshot
}

/** 文字を打ったのと同じ経路(handleTextInput)を通す */
function typeText(text: string) {
  const { from, to } = view.state.selection
  const handled = view.someProp('handleTextInput', (f) => f(view, from, to, text, () => view.state.tr.insertText(text, from, to)))
  if (!handled) view.dispatch(view.state.tr.insertText(text, from, to))
}

const cursorAt = (pos: number) => view.dispatch(view.state.tr.setSelection(TextSelection.create(view.state.doc, pos)))
const slash = () => slashKey.getState(view.state)

describe('スラッシュメニューを開く条件', () => {
  it('ブロックの先頭で「/」を打つと開き、続けて打った文字が絞り込みの文字になる', () => {
    setDoc('\n')
    cursorAt(1)
    typeText('/')
    expect(slash()).toEqual({ active: true, from: 1, query: '' })
    typeText('見')
    expect(slash()).toMatchObject({ active: true, query: '見' })
  })

  it('空白の後なら文の途中でも開く。単語の途中(URL など)では開かない', () => {
    setDoc('メモ https:\n')
    cursorAt(view.state.doc.content.size - 1)
    typeText('/')
    expect(slash()?.active).toBe(false)
    setDoc('メモ\n')
    cursorAt(view.state.doc.content.size - 1)
    typeText(' ')
    typeText('/')
    expect(slash()?.active).toBe(true)
  })

  it('IME の変換中に打った「/」では開かない', () => {
    setDoc('\n')
    cursorAt(1)
    Object.defineProperty(view, 'composing', { get: () => true, configurable: true })
    typeText('/')
    expect(slash()?.active).toBe(false)
  })

  it('コードブロックの中では開かない', () => {
    setDoc('```\ncode\n```\n')
    cursorAt(2)
    typeText('/')
    expect(slash()?.active).toBe(false)
  })

  it('「/」を消すと閉じる', () => {
    setDoc('\n')
    cursorAt(1)
    typeText('/')
    view.dispatch(view.state.tr.delete(1, 2))
    expect(slash()?.active).toBe(false)
  })
})

describe('絞り込み', () => {
  it('何も打っていなければ、デザインの並びのまま全部', () => {
    expect(filterSlashItems('').map((i) => i.label)).toEqual(SLASH_ITEMS.map((i) => i.label))
  })
  it('「見」で見出し1〜3', () => {
    expect(filterSlashItems('見').map((i) => i.label)).toEqual(['見出し1', '見出し2', '見出し3'])
  })
  it('ひらがなや英語の別名でも見つかる', () => {
    expect(filterSlashItems('こーど').map((i) => i.id)).toEqual(['code'])
    expect(filterSlashItems('todo').map((i) => i.id)).toEqual(['task'])
    expect(filterSlashItems('ｈ２').map((i) => i.id)).toEqual(['h2']) // 全角英数字も
  })
  it('一致しなければ空', () => {
    expect(filterSlashItems('xyz')).toEqual([])
  })
  it('「page 議事録」はページを作る項目だけになり、後ろの文字がタイトルになる', () => {
    expect(filterSlashItems('page 議事録').map((i) => i.id)).toEqual(['page'])
    expect(pageTitleFromQuery('page 議事録')).toBe('議事録')
    expect(pageTitleFromQuery('page')).toBe('')
  })
})

describe('項目の実行', () => {
  it('「/見」から見出し2を選ぶと、打った文字が消えて見出しになる', () => {
    setDoc('\n')
    cursorAt(1)
    typeText('/')
    typeText('見')
    const state = slash()
    if (!state?.active) throw new Error('メニューが開いていない')
    editor.action((ctx) => runBlockCommand(ctx, view, SLASH_ITEMS[2], state))
    const block = view.state.doc.child(0)
    expect(block.type.name).toBe('heading')
    expect(block.attrs.level).toBe(2)
    expect(block.textContent).toBe('')
    expect(slash()?.active).toBe(false)
  })

  it('チェックリストは、未完了のチェック項目になる', () => {
    setDoc('\n')
    cursorAt(1)
    typeText('/')
    const state = slash()
    if (!state?.active) throw new Error('メニューが開いていない')
    editor.action((ctx) => runBlockCommand(ctx, view, SLASH_ITEMS[6], state))
    const list = view.state.doc.child(0)
    expect(list.type.name).toBe('bullet_list')
    expect(list.child(0).attrs.checked).toBe(false)
  })
})

describe('ブロックの並べ替え', () => {
  const source = '# 見出し\n\n| a | b |\n| :--- | ---: |\n| 1 | 2 |\n\n***\n\n最後の段落。\n'

  it('動かしたブロックは原文のまま保存される(表の区切り行も ***)', () => {
    const snapshot = setDoc(source)
    moveBlock(view, 1, 3)
    const out = editor.action((ctx) => serializeMarkdown(ctx, view.state.doc, snapshot))
    expect(out).toBe('# 見出し\n\n***\n\n最後の段落。\n\n| a | b |\n| :--- | ---: |\n| 1 | 2 |\n')
  })

  it('1回の操作なので、取り消しで元に戻る', () => {
    const snapshot = setDoc(source)
    moveBlock(view, 3, 0)
    editor.action(callCommand(undoCommand.key))
    expect(editor.action((ctx) => serializeMarkdown(ctx, view.state.doc, snapshot))).toBe(source)
  })

  it('カーソルが動かしたブロックの中にあれば、移した先の同じ場所に置く', () => {
    setDoc(source)
    const lastStart = view.state.doc.content.size - view.state.doc.lastChild!.nodeSize
    cursorAt(lastStart + 3)
    moveBlock(view, 3, 0)
    expect(view.state.selection.head).toBe(3)
    expect(view.state.doc.child(0).textContent).toBe('最後の段落。')
  })
})
