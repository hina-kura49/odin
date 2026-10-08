import { Plugin, PluginKey } from '@milkdown/kit/prose/state'
import type { EditorView } from '@milkdown/kit/prose/view'
import { $prose } from '@milkdown/kit/utils'

// 非同期の処理(ページを作るなど)を待つ間に、文書が編集されても位置がずれないよう、位置を追い続ける。

type Tracked = Map<number, number | null>
type Meta = { add: [number, number] } | { remove: number }

const key = new PluginKey<Tracked>('odin-tracked-pos')
let seq = 0

export const trackedPosPlugin = $prose(
  () =>
    new Plugin<Tracked>({
      key,
      state: {
        init: () => new Map(),
        apply(tr, prev) {
          const meta = tr.getMeta(key) as Meta | undefined
          let next = prev
          if (tr.docChanged) {
            next = new Map()
            for (const [id, pos] of prev) {
              // 位置を含む範囲が消されたら null(挿入先がなくなった)
              const mapped = pos === null ? null : tr.mapping.mapResult(pos, -1)
              next.set(id, mapped === null || mapped.deletedAcross ? null : mapped.pos)
            }
          }
          if (meta && 'add' in meta) next = new Map(next).set(meta.add[0], meta.add[1])
          if (meta && 'remove' in meta) {
            next = new Map(next)
            next.delete(meta.remove)
          }
          return next
        },
      },
    }),
)

/** pos を追い始める。release() でいまの位置を受け取り、追うのをやめる(消されていたら null) */
export function trackPosition(view: EditorView, pos: number): { release: () => number | null } {
  const id = ++seq
  view.dispatch(view.state.tr.setMeta(key, { add: [id, pos] } satisfies Meta))
  return {
    release: () => {
      const at = key.getState(view.state)?.get(id) ?? null
      view.dispatch(view.state.tr.setMeta(key, { remove: id } satisfies Meta))
      return at
    },
  }
}
