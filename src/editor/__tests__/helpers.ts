import { Editor, parserCtx, serializerCtx } from '@milkdown/kit/core'
import type { Ctx } from '@milkdown/kit/ctx'
import { Fragment, type Node as PMNode } from '@milkdown/kit/prose/model'
import { configureMarkdown, loadMarkdown, serializeMarkdown } from '../markdown'

type Edit = (doc: PMNode, h: { ctx: Ctx; parse: (md: string) => PMNode[] }) => PMNode

/** 読み込み → (編集) → 保存。plain は Milkdown だけで読み書きした結果(無編集、比較用)。 */
export async function roundTrip(markdown: string, edit?: Edit): Promise<{ preserved: string; plain: string }> {
  const editor = await configureMarkdown(Editor.make()).create()
  const result = editor.action((ctx) => {
    const { doc, snapshot } = loadMarkdown(ctx, markdown)
    const parse = (md: string) => children(ctx.get(parserCtx)(md))
    const edited = edit ? edit(doc, { ctx, parse }) : doc
    return {
      preserved: serializeMarkdown(ctx, edited, snapshot),
      plain: ctx.get(serializerCtx)(ctx.get(parserCtx)(markdown)),
    }
  })
  await editor.destroy()
  return result
}

/** 保存結果を読み直したときの最上位ノードの種類の並び。 */
export async function topLevelTypes(markdown: string): Promise<string[]> {
  const editor = await configureMarkdown(Editor.make()).create()
  const types = editor.action((ctx) => children(ctx.get(parserCtx)(markdown)).map((n) => n.type.name))
  await editor.destroy()
  return types
}

export const children = (doc: PMNode): PMNode[] => {
  const out: PMNode[] = []
  doc.forEach((n) => out.push(n))
  return out
}

export const withChildren = (doc: PMNode, nodes: PMNode[]): PMNode => doc.copy(Fragment.fromArray(nodes))

/** 配列の from 番目を to 番目へ移す(ドラッグでの並べ替えと同じ結果)。 */
export function move<T>(items: T[], from: number, to: number): T[] {
  const out = [...items]
  const [item] = out.splice(from, 1)
  out.splice(to, 0, item)
  return out
}
