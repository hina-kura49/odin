import { keymap } from '@milkdown/kit/prose/keymap'
import { $prose } from '@milkdown/kit/utils'
import { moveCurrentBlock } from './block-move'

/**
 * Milkdown の既定に加えるショートカット(デザイン 9)。行内のコード(⌘E)は Milkdown の既定のまま使う。
 * ProseMirror は IME の変換中のキーをショートカットとして扱わないので、変換中は反応しない。
 */
export const extraKeys = $prose(() =>
  keymap({
    // ブロック・リストの項目の並べ替え(ドラッグのキーボード版)
    'Mod-Shift-ArrowUp': (_state, _dispatch, view) => (view ? moveCurrentBlock(view, -1) : false),
    'Mod-Shift-ArrowDown': (_state, _dispatch, view) => (view ? moveCurrentBlock(view, 1) : false),
  }),
)
