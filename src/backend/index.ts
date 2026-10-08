import { isCaptureWindow } from '@/lib/platform'
import { MockBackend } from './mock'
import { CaptureRelayBackend, serveMockCaptures } from './mock-relay'
import { TauriBackend } from './tauri'
import type { Backend } from './types'

export { BackendError, INBOX_PATH, isBackendError, toBackendError } from './types'
export type { Backend, BackendErrorKind, NodeKind, PageMeta, SearchHit, Snippet, TreeNode, WriteResult } from './types'

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
  // 開発者ツールから外部の変更を再現できるようにする:
  //   __mock.simulateExternalEdit(path, content) / __mock.simulateExternalDelete(path) / __mock.setReadOnly(path, true)
  Object.assign(window, { __mock: mock })
  return mock
}

/**
 * どのバックエンドを使うか。
 * - ブラウザ(vite dev)では MockBackend
 * - Tauri のアプリでも、core/ をつなぐまでは MockBackend(VITE_BACKEND=tauri のときだけ TauriBackend)
 *   Mock はウィンドウごとに別のメモリなので、クイックキャプチャのウィンドウはメインのウィンドウの Mock に取り込みを頼む
 */
function createBackend(): Backend {
  if (isTauri && import.meta.env.VITE_BACKEND === 'tauri') return new TauriBackend()
  if (isTauri && isCaptureWindow()) return new CaptureRelayBackend()
  const mock = createMock()
  if (isTauri) serveMockCaptures(mock)
  return mock
}

let current: Backend = createBackend()

export const backend = (): Backend => current

/** テスト用: バックエンドを差し替える */
export function setBackend(next: Backend): void {
  current = next
}
