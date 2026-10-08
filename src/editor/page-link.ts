import type { Schema, Node as PMNode } from '@milkdown/kit/prose/model'
import type { EditorView } from '@milkdown/kit/prose/view'
import type { PageMeta } from '@/backend'
import { dirOf, encodeLinkDestination, relativePath } from '@/lib/paths'

/**
 * /page が挿入するリンクの行き先。バックエンドの改名処理と一致させる必要がある。
 * - いま開いているページのあるフォルダから見た相対パス
 * - 空白 ( ) < > # % ? と制御文字だけをパーセントエンコード(16進数は大文字)。日本語などはそのまま。<...> で囲まない
 */
export const pageLinkDestination = (currentPagePath: string, targetPath: string): string =>
  encodeLinkDestination(relativePath(dirOf(currentPagePath), targetPath))

/** 表示文字列は createPage が返した title と完全に同じにする */
export function pageLinkNode(schema: Schema, currentPagePath: string, target: PageMeta): PMNode {
  const link = schema.marks.link.create({ href: pageLinkDestination(currentPagePath, target.path), title: null })
  return schema.text(target.title, [link])
}

/** カーソルの位置(選択範囲があれば置き換え)にページへのリンクを入れる */
export function insertPageLink(view: EditorView, currentPagePath: string, target: PageMeta): void {
  const node = pageLinkNode(view.state.schema, currentPagePath, target)
  view.dispatch(view.state.tr.replaceSelectionWith(node, false).scrollIntoView())
}
