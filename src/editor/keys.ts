import { keymap } from '@milkdown/kit/prose/keymap'
import { Selection } from '@milkdown/kit/prose/state'
import { $prose } from '@milkdown/kit/utils'
import { useApp } from '@/store/app'
import { moveCurrentBlock } from './block-move'
import { activeSession } from './registry'

/**
 * Milkdown の既定に加えるショートカット(デザイン 9)。行内のコード(⌘E)は Milkdown の既定のまま使う。
 * ProseMirror は IME の変換中のキーをショートカットとして扱わないので、変換中は反応しない。
 */
export const extraKeys = $prose(() =>
  keymap({
    // ブロック・リストの項目の並べ替え(ドラッグのキーボード版)
    'Mod-Shift-ArrowUp': (_state, _dispatch, view) => (view ? moveCurrentBlock(view, -1) : false),
    'Mod-Shift-ArrowDown': (_state, _dispatch, view) => (view ? moveCurrentBlock(view, 1) : false),
    // 本文の最初の行で ↑ を押したら、タイトルへ(キーボードだけで名前を変えられるように)
    ArrowUp: (state, _dispatch, view) => {
      const { selection } = state
      // 文書の最初のテキストの、最初の行にいるときだけ
      const first = Selection.atStart(state.doc).$head
      if (!view || !selection.empty || selection.$head.start() !== first.start() || !view.endOfTextblock('up')) return false
      const current = activeSession()?.current()
      if (!current) return false
      useApp.getState().focusTitle(current.path)
      return true
    },
  }),
)
