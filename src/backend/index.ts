import { MockBackend } from './mock'
import { TauriBackend } from './tauri'
import type { Backend } from './types'

export type { Backend, PageMeta, SearchHit, TreeNode, WriteResult } from './types'

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/**
 * vite dev では MockBackend を使う。URL の引数で状態を切り替えられる。
 *   ?vault=none   初回起動(フォルダ未選択)
 *   ?empty        ページが1つもない保管庫
 *   ?latency=300  すべての呼び出しを 300ms 遅らせる
 */
function createMock(): MockBackend {
  const params = new URLSearchParams(window.location.search)
  const mock = new MockBackend({
    vault: params.get('vault') === 'none' ? null : undefined,
    empty: params.has('empty'),
    latencyMs: Number(params.get('latency') ?? 0),
  })
  // 開発者ツールから外部変更を再現できるようにする: __mock.simulateExternalEdit(path, content)
  Object.assign(window, { __mock: mock })
  return mock
}

let current: Backend = isTauri ? new TauriBackend() : createMock()

export const backend = (): Backend => current

/** テスト用: バックエンドを差し替える */
export function setBackend(next: Backend): void {
  current = next
}
