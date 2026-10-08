import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { dirOf, hasScheme, resolveInVault } from '@/lib/paths'
import { toBackendError, type Backend, type PageMeta, type SearchHit, type TreeNode, type WriteResult } from './types'

// Tauri の invoke とイベントを呼ぶだけの薄い層。中身(Rust 側)は別担当が接続する。
//
// Rust 側との取り決め(案):
// - コマンド名は snake_case。引数は Tauri 2 の既定どおり、JS の camelCase が Rust の snake_case に対応する
//   例: write_page(path: String, content: String, base_version: String)
// - 戻り値は types.ts の型を serde で camelCase にしたもの(#[serde(rename_all = "camelCase")])
// - 失敗は { kind: BackendErrorKind, message: string } の形で返す(知らない kind でもフロントは落ちない)
// - 外部変更は "external-change" イベント(payload なし)
// - assetUrl は同期の関数なので invoke しない。保管庫の絶対パスと convertFileSrc で組み立てる
//   (Rust 側で asset プロトコルの scope に保管庫を入れておく必要がある)
export const EXTERNAL_CHANGE_EVENT = 'external-change'

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (e) {
    throw toBackendError(e)
  }
}

export class TauriBackend implements Backend {
  /** 保管庫の絶対パス。assetUrl の組み立てに使う */
  private vault: string | null = null

  async currentVault(): Promise<string | null> {
    this.vault = await call<string | null>('current_vault')
    return this.vault
  }
  async openVault(): Promise<string | null> {
    const vault = await call<string | null>('open_vault')
    if (vault !== null) this.vault = vault
    return vault
  }
  listTree(): Promise<TreeNode[]> {
    return call<TreeNode[]>('list_tree')
  }
  readPage(path: string): Promise<{ content: string; version: string }> {
    return call('read_page', { path })
  }
  writePage(path: string, content: string, baseVersion: string): Promise<WriteResult> {
    return call<WriteResult>('write_page', { path, content, baseVersion })
  }
  createPage(parentPath: string | null, title: string): Promise<PageMeta> {
    return call<PageMeta>('create_page', { parentPath, title })
  }
  renamePage(path: string, newTitle: string): Promise<PageMeta> {
    return call<PageMeta>('rename_page', { path, newTitle })
  }
  deletePage(path: string): Promise<void> {
    return call<void>('delete_page', { path })
  }
  search(query: string, limit: number): Promise<SearchHit[]> {
    return call<SearchHit[]>('search', { query, limit })
  }
  recentPages(limit: number): Promise<PageMeta[]> {
    return call<PageMeta[]>('recent_pages', { limit })
  }
  captureToInbox(text: string): Promise<void> {
    return call<void>('capture_to_inbox', { text })
  }
  assetUrl(pagePath: string, src: string): string | null {
    if (hasScheme(src)) return src
    if (this.vault === null) return null
    let decoded = src
    try {
      decoded = decodeURI(src)
    } catch {
      // そのまま使う
    }
    const path = resolveInVault(dirOf(pagePath), decoded)
    return path === null ? null : convertFileSrc(`${this.vault.replace(/\/$/, '')}/${path}`)
  }
  onExternalChange(cb: () => void): () => void {
    // listen は Promise で解除関数を返すので、登録前に解除されても漏れないようにする
    let unlisten: (() => void) | null = null
    let disposed = false
    void listen(EXTERNAL_CHANGE_EVENT, () => cb()).then((fn) => {
      if (disposed) fn()
      else unlisten = fn
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }
}
