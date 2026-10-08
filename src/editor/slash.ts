import { commandsCtx, type CmdKey } from '@milkdown/kit/core'
import type { Ctx } from '@milkdown/kit/ctx'
import {
  createCodeBlockCommand,
  turnIntoTextCommand,
  wrapInBlockquoteCommand,
  wrapInBulletListCommand,
  wrapInHeadingCommand,
  wrapInOrderedListCommand,
} from '@milkdown/kit/preset/commonmark'
import { insertTableCommand } from '@milkdown/kit/preset/gfm'
import type { EditorState, Transaction } from '@milkdown/kit/prose/state'
import { Plugin, PluginKey } from '@milkdown/kit/prose/state'
import type { EditorView } from '@milkdown/kit/prose/view'
import { $prose } from '@milkdown/kit/utils'

// ---- スラッシュメニューの状態 ----
// 「/」(全角の「／」も)をブロックの先頭か空白の後に打つと開き、その後ろの文字で絞り込む。
// IME の変換中に打った「/」では開かない(確定した「／」では開く)。

export type SlashState = { active: false } | { active: true; from: number; query: string }

export const slashKey = new PluginKey<SlashState>('odin-slash')

type SlashMeta = { open: number } | { close: true }

const SLASHES = ['/', '／']
const MAX_QUERY = 30

/** pos の位置に「/」を置いたとき、メニューを開いてよいか(ブロックの先頭か、空白の直後。コードの中は除く) */
function canOpenAt(state: EditorState, pos: number): boolean {
  const $pos = state.doc.resolve(pos)
  if (!$pos.parent.isTextblock || $pos.parent.type.spec.code) return false
  if ($pos.marks().some((m) => m.type.spec.code)) return false
  if ($pos.parentOffset === 0) return true
  const before = $pos.parent.textBetween($pos.parentOffset - 1, $pos.parentOffset)
  return /\s/.test(before)
}

function nextState(tr: Transaction, prev: SlashState, state: EditorState): SlashState {
  const meta = tr.getMeta(slashKey) as SlashMeta | undefined
  if (meta && 'close' in meta) return { active: false }
  let from: number
  if (meta && 'open' in meta) from = meta.open
  else if (prev.active) from = tr.mapping.map(prev.from)
  else return prev
  const { selection, doc } = state
  const head = selection.head
  // カーソルが「/」より前に戻った、別のブロックへ移った、範囲選択になった、「/」が消えた → 閉じる
  if (!selection.empty || head <= from || from >= doc.content.size) return { active: false }
  const $from = doc.resolve(from)
  const $head = doc.resolve(head)
  if ($from.parent !== $head.parent) return { active: false }
  if (!SLASHES.includes(doc.textBetween(from, from + 1))) return { active: false }
  const query = doc.textBetween(from + 1, head)
  if (query.length > MAX_QUERY || query.includes('\n')) return { active: false }
  return { active: true, from, query }
}

export const slashPlugin = $prose(
  () =>
    new Plugin<SlashState>({
      key: slashKey,
      state: {
        init: () => ({ active: false }),
        apply: (tr, prev, _old, state) => nextState(tr, prev, state),
      },
      props: {
        handleTextInput(view, from, to, text) {
          // 変換中の入力は handleTextInput に来ない(来ても無視する)
          if (view.composing || !SLASHES.includes(text) || !canOpenAt(view.state, from)) return false
          view.dispatch(view.state.tr.insertText(text, from, to).setMeta(slashKey, { open: from } satisfies SlashMeta))
          return true
        },
        handleDOMEvents: {
          // 変換して確定した「／」で開く(変換中は開かない)
          compositionend(view) {
            setTimeout(() => {
              const { selection } = view.state
              if (!selection.empty || slashKey.getState(view.state)?.active) return
              const pos = selection.head - 1
              if (pos < 0 || view.state.doc.textBetween(pos, selection.head) !== '／') return
              if (!canOpenAt(view.state, pos)) return
              view.dispatch(view.state.tr.setMeta(slashKey, { open: pos } satisfies SlashMeta))
            }, 0)
            return false
          },
          blur(view) {
            if (slashKey.getState(view.state)?.active) view.dispatch(view.state.tr.setMeta(slashKey, { close: true } satisfies SlashMeta))
            return false
          },
        },
      },
    }),
)

export const closeSlash = (view: EditorView) => view.dispatch(view.state.tr.setMeta(slashKey, { close: true } satisfies SlashMeta))

// ---- メニューの項目 ----

export type SlashIcon = 'text' | 'h1' | 'h2' | 'h3' | 'bullet' | 'ordered' | 'task' | 'quote' | 'code' | 'table' | 'image' | 'page'

export type SlashItem = {
  id: SlashIcon
  label: string
  /** 項目の右に添える補足(「/page」など) */
  hint?: string
  /** 絞り込みに使う別名(ひらがな、英語) */
  aliases: string[]
}

/** デザイン 2「通常の表示」の並び */
export const SLASH_ITEMS: SlashItem[] = [
  { id: 'text', label: 'テキスト', aliases: ['てきすと', 'ほんぶん', '本文', 'text', 'paragraph', 'p'] },
  { id: 'h1', label: '見出し1', aliases: ['みだし', 'heading', 'h1'] },
  { id: 'h2', label: '見出し2', aliases: ['みだし', 'heading', 'h2'] },
  { id: 'h3', label: '見出し3', aliases: ['みだし', 'heading', 'h3'] },
  { id: 'bullet', label: 'リスト', aliases: ['りすと', '箇条書き', 'かじょうがき', 'list', 'bullet', 'ul'] },
  { id: 'ordered', label: '番号付きリスト', aliases: ['ばんごう', 'number', 'ordered', 'ol'] },
  { id: 'task', label: 'チェックリスト', aliases: ['ちぇっく', 'たすく', 'todo', 'task', 'check'] },
  { id: 'quote', label: '引用', aliases: ['いんよう', 'quote', 'blockquote'] },
  { id: 'code', label: 'コードブロック', aliases: ['こーど', 'code', 'pre'] },
  { id: 'table', label: '表', aliases: ['ひょう', 'てーぶる', 'table'] },
  { id: 'image', label: '画像', aliases: ['がぞう', 'image', 'img', 'picture'] },
  { id: 'page', label: 'ページを作る', hint: '/page', aliases: ['ぺーじ', 'page', 'new'] },
]

const normalize = (s: string) => s.normalize('NFKC').toLowerCase().trim()

/**
 * 打った文字で項目を絞り込む。表示名か別名に含まれていれば残す。
 * 「/page 議事録」のように、page の後ろの文字は作るページのタイトルとして扱う。
 */
export function filterSlashItems(query: string): SlashItem[] {
  const q = normalize(query)
  if (!q) return SLASH_ITEMS
  if (/^page(\s|$)/.test(q)) return SLASH_ITEMS.filter((i) => i.id === 'page')
  return SLASH_ITEMS.filter((item) => [item.label, ...item.aliases].some((name) => normalize(name).includes(q)))
}

/** 「/page 議事録」の「議事録」。なければ空文字 */
export const pageTitleFromQuery = (query: string): string => /^page\s+(.*)$/i.exec(query.normalize('NFKC').trim())?.[1]?.trim() ?? ''

// ---- 項目の実行 ----

/** 「/」と打った文字を消して、ブロックを変える。page と image は呼び出し側が受け持つ */
export function runBlockCommand(ctx: Ctx, view: EditorView, item: SlashItem, slash: { from: number; query: string }): void {
  const end = slash.from + 1 + slash.query.length
  view.dispatch(view.state.tr.delete(slash.from, end).setMeta(slashKey, { close: true } satisfies SlashMeta))
  const commands = ctx.get(commandsCtx)
  const call = <T>(key: CmdKey<T>, payload?: T) => commands.call(key, payload)
  switch (item.id) {
    case 'text':
      call(turnIntoTextCommand.key)
      break
    case 'h1':
    case 'h2':
    case 'h3':
      call(wrapInHeadingCommand.key, Number(item.id.slice(1)))
      break
    case 'bullet':
      call(wrapInBulletListCommand.key)
      break
    case 'ordered':
      call(wrapInOrderedListCommand.key)
      break
    case 'task': {
      call(wrapInBulletListCommand.key)
      // 箇条書きにした項目を、未完了のチェック項目にする
      const { $from } = view.state.selection
      for (let d = $from.depth; d > 0; d--) {
        const node = $from.node(d)
        if (node.type.name === 'list_item') {
          view.dispatch(view.state.tr.setNodeMarkup($from.before(d), undefined, { ...node.attrs, checked: false }))
          break
        }
      }
      break
    }
    case 'quote':
      call(wrapInBlockquoteCommand.key)
      break
    case 'code':
      call(createCodeBlockCommand.key)
      break
    case 'table':
      call(insertTableCommand.key, { row: 3, col: 3 })
      break
    case 'image':
    case 'page':
      break
  }
  view.focus()
}

/** 「/」と打った文字を消す(page と image で使う)。消した位置を返す */
export function removeSlashText(view: EditorView, slash: { from: number; query: string }): number {
  view.dispatch(view.state.tr.delete(slash.from, slash.from + 1 + slash.query.length).setMeta(slashKey, { close: true } satisfies SlashMeta))
  return slash.from
}
