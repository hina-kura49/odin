// バックエンドとの契約。フロントはこのインターフェースだけを通してバックエンドと話す。
// 変更したいときは勝手に変えず、理由を添えて提案する。

export type PageMeta = { path: string; title: string; modifiedAt: number }
export type TreeNode = { path: string; title: string; children: TreeNode[] }
export type SearchHit = { path: string; title: string; snippet: string }
export type WriteResult = { ok: true; modifiedAt: number } | { ok: false; reason: 'conflict' }

export interface Backend {
  openVault(): Promise<string | null> // フォルダ選択。キャンセルならnull
  listTree(): Promise<TreeNode[]>
  readPage(path: string): Promise<{ content: string; modifiedAt: number }>
  writePage(path: string, content: string, baseModifiedAt: number): Promise<WriteResult>
  createPage(parentPath: string | null, title: string): Promise<PageMeta>
  renamePage(path: string, newTitle: string): Promise<PageMeta>
  deletePage(path: string): Promise<void>
  search(query: string): Promise<SearchHit[]>
  recentPages(): Promise<PageMeta[]>
  captureToInbox(text: string): Promise<void>
  onExternalChange(cb: (path: string) => void): () => void // 解除関数を返す
}
