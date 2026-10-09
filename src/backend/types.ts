// バックエンドとの契約。フロントはこのインターフェースだけを通してバックエンドと話す。
// 変更したいときは勝手に変えず、理由を添えて提案する。
//
// 約束ごと
// - version は中身の分からない文字列。受け取った値をそのまま次の writePage に渡す。
// - path は「/」区切りで、「:」を含むことがある。識別子としてそのまま使い、加工しない。
// - title はバックエンドが返したものを使う。createPage と renamePage は渡したタイトルを整えて返すことがある。
//   返ってきた title と path を正とする。
// - kind が "folder" の節点はページのないフォルダ。開閉だけで、readPage を呼ばない。createPage の parentPath には渡せる。
// - 知らないエラーの種類が来ても落ちない。
// - onExternalChange が呼ばれたら、listTree を取り直し、開いているページは readPage で version を比べる。

/** captureToInbox の書き込み先。バックエンドが保管庫の直下に作る */
export const INBOX_PATH = 'Inbox.md'

// 契約の型のうち、Rust の型から作ったもの(src/backend/generated/。npm run gen:types で作り直す)と形が一致するものは、それをそのまま使う。
// titleMatched: タイトルに検索語が当たったか。パレットの「ページ」と「本文の検索結果」の振り分けはこの値だけで行う
import type { SearchHit } from './generated/SearchHit'
import type { TreeNode } from './generated/TreeNode'
export type { NodeKind } from './generated/NodeKind'
export type { Snippet } from './generated/Snippet'
export type { SearchHit, TreeNode }

// 次の3つは、Rust から作った型と契約の形が違う(報告済み。決まるまで契約のままにし、TauriBackend の境目で変換する)
// - PageMeta.modifiedAt: 生成では bigint(Rust の u64)。JSON では数値で届く
// - WriteResult: 生成では { status: "ok", version } | { status: "conflict" }
// - BackendErrorKind: 生成(ErrorKind)には core の IndexOverlapsVault に当たる "indexOverlapsVault" がある。届いたら 'io' として扱う
export type PageMeta = { path: string; title: string; modifiedAt: number } // UNIX時刻のミリ秒
export type WriteResult = { ok: true; version: string } | { ok: false; reason: 'conflict' }
export type BackendErrorKind = 'notFound' | 'invalidPath' | 'notAPage' | 'notUtf8' | 'readOnly' | 'nameOccupied' | 'io'

const KNOWN_KINDS: readonly string[] = ['notFound', 'invalidPath', 'notAPage', 'notUtf8', 'readOnly', 'nameOccupied', 'io']

export class BackendError extends Error {
  readonly kind: BackendErrorKind
  /** 知らない種類が来たときの元の値(kind は 'io' として扱う) */
  readonly rawKind: string

  constructor(kind: string, message?: string) {
    super(message ?? kind)
    this.name = 'BackendError'
    this.rawKind = kind
    this.kind = KNOWN_KINDS.includes(kind) ? (kind as BackendErrorKind) : 'io'
  }
}

/** 呼び出しの失敗を BackendError にそろえる。知らない形のエラーでも落ちない */
export function toBackendError(e: unknown): BackendError {
  if (e instanceof BackendError) return e
  if (typeof e === 'object' && e !== null && 'kind' in e && typeof e.kind === 'string') {
    const message = 'message' in e && typeof e.message === 'string' ? e.message : undefined
    return new BackendError(e.kind, message)
  }
  return new BackendError('io', e instanceof Error ? e.message : String(e))
}

export const isBackendError = (e: unknown, kind: BackendErrorKind): boolean => toBackendError(e).kind === kind

export interface Backend {
  currentVault(): Promise<string | null> // 前回開いた保管庫。なければ null
  openVault(): Promise<string | null> // フォルダ選択。キャンセルなら null
  listTree(): Promise<TreeNode[]> // 並び順はバックエンドが決める。並べ替えない
  readPage(path: string): Promise<{ content: string; version: string }>
  writePage(path: string, content: string, baseVersion: string): Promise<WriteResult>
  createPage(parentPath: string | null, title: string): Promise<PageMeta>
  renamePage(path: string, newTitle: string): Promise<PageMeta>
  deletePage(path: string): Promise<void>
  search(query: string, limit: number): Promise<SearchHit[]>
  recentPages(limit: number): Promise<PageMeta[]>
  captureToInbox(text: string): Promise<void>
  assetUrl(pagePath: string, src: string): string | null // ページ内の相対パスを表示用URLに。保管庫の外なら null
  onExternalChange(cb: () => void): () => void
}
