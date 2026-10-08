import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Backend, PageMeta, SearchHit, TreeNode, WriteResult } from './types'

// Tauri の invoke とイベントを呼ぶだけの薄い層。中身(Rust 側)は別担当が接続する。
//
// Rust 側との取り決め(案):
// - コマンド名は snake_case。引数は Tauri 2 の既定どおり、JS の camelCase が Rust の snake_case に対応する
//   例: write_page(path: String, content: String, base_modified_at: u64)
// - 戻り値は types.ts の型を serde で camelCase にしたもの(#[serde(rename_all = "camelCase")])
// - 外部変更は "external-change" イベントで、payload は { path: string }
// - path は保管庫(vault)のルートからの相対パス。区切りは "/"
export const EXTERNAL_CHANGE_EVENT = 'external-change'

export class TauriBackend implements Backend {
  openVault(): Promise<string | null> {
    return invoke<string | null>('open_vault')
  }
  listTree(): Promise<TreeNode[]> {
    return invoke<TreeNode[]>('list_tree')
  }
  readPage(path: string): Promise<{ content: string; modifiedAt: number }> {
    return invoke('read_page', { path })
  }
  writePage(path: string, content: string, baseModifiedAt: number): Promise<WriteResult> {
    return invoke<WriteResult>('write_page', { path, content, baseModifiedAt })
  }
  createPage(parentPath: string | null, title: string): Promise<PageMeta> {
    return invoke<PageMeta>('create_page', { parentPath, title })
  }
  renamePage(path: string, newTitle: string): Promise<PageMeta> {
    return invoke<PageMeta>('rename_page', { path, newTitle })
  }
  deletePage(path: string): Promise<void> {
    return invoke<void>('delete_page', { path })
  }
  search(query: string): Promise<SearchHit[]> {
    return invoke<SearchHit[]>('search', { query })
  }
  recentPages(): Promise<PageMeta[]> {
    return invoke<PageMeta[]>('recent_pages')
  }
  captureToInbox(text: string): Promise<void> {
    return invoke<void>('capture_to_inbox', { text })
  }
  onExternalChange(cb: (path: string) => void): () => void {
    // listen は Promise で解除関数を返すので、登録前に解除されても漏れないようにする
    let unlisten: (() => void) | null = null
    let disposed = false
    void listen<{ path: string }>(EXTERNAL_CHANGE_EVENT, (e) => cb(e.payload.path)).then((fn) => {
      if (disposed) fn()
      else unlisten = fn
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }
}
