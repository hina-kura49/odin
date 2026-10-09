import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { dirOf, hasScheme, resolveInVault } from '@/lib/paths'
import type { PageContent } from './generated/PageContent'
import type { PageMeta as RawPageMeta } from './generated/PageMeta'
import type { WriteResult as RawWriteResult } from './generated/WriteResult'
import { toBackendError, type Backend, type PageMeta, type SearchHit, type TreeNode, type WriteResult } from './types'

// Tauri の invoke とイベントを呼ぶだけの薄い層。Rust 側は src-tauri/src/vault.rs(core の Vault を呼ぶだけ)。
//
// Rust 側との取り決め:
// - コマンド名は snake_case。引数は Tauri 2 の既定どおり、JS の camelCase が Rust の snake_case に対応する
//   例: write_page(path: String, content: String, base_version: String)
// - 戻り値の型は Rust から作ったもの(./generated/)。契約と形が違うもの(PageMeta、WriteResult)は、ここで契約の形に変える
// - 失敗は { kind, message } の形(./generated/IpcError)。知らない kind でもフロントは落ちない
// - 外部変更は "external-change" イベント(payload なし)。保管庫の監視と、captureToInbox の後に Rust 側が出す
// - assetUrl は同期の関数なので invoke しない。保管庫の絶対パスと convertFileSrc で組み立てる
//   (Rust 側で、開いた保管庫だけを asset プロトコルで読めるようにしている)
export const EXTERNAL_CHANGE_EVENT = 'external-change'

/** そのまま表示に使ってよい URL(Web の画像と、埋め込みの画像)。file: などほかのスキームは保管庫の外を指しうるので使わない */
const PASS_THROUGH_SCHEME = /^(https?|data):/i

// Rust の u64 は JSON では数値で届く(型の上では bigint)。どちらでも数値にそろえる
const toPageMeta = (m: RawPageMeta): PageMeta => ({ path: m.path, title: m.title, modifiedAt: Number(m.modifiedAt) })

const toWriteResult = (r: RawWriteResult): WriteResult =>
  r.status === 'ok' ? { ok: true, version: r.version } : { ok: false, reason: 'conflict' }

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
  readPage(path: string): Promise<PageContent> {
    return call<PageContent>('read_page', { path })
  }
  async writePage(path: string, content: string, baseVersion: string): Promise<WriteResult> {
    return toWriteResult(await call<RawWriteResult>('write_page', { path, content, baseVersion }))
  }
  async createPage(parentPath: string | null, title: string): Promise<PageMeta> {
    return toPageMeta(await call<RawPageMeta>('create_page', { parentPath, title }))
  }
  async renamePage(path: string, newTitle: string): Promise<PageMeta> {
    return toPageMeta(await call<RawPageMeta>('rename_page', { path, newTitle }))
  }
  deletePage(path: string): Promise<void> {
    return call<void>('delete_page', { path })
  }
  search(query: string, limit: number): Promise<SearchHit[]> {
    return call<SearchHit[]>('search', { query, limit })
  }
  async recentPages(limit: number): Promise<PageMeta[]> {
    return (await call<RawPageMeta[]>('recent_pages', { limit })).map(toPageMeta)
  }
  captureToInbox(text: string): Promise<void> {
    // メインのウィンドウへの変更の知らせ(external-change)は、Rust 側が書き込みの後に出す
    return call<void>('capture_to_inbox', { text })
  }
  assetUrl(pagePath: string, src: string): string | null {
    if (hasScheme(src)) return PASS_THROUGH_SCHEME.test(src) ? src : null
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
