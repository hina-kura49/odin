import { MOCK_FOLDERS, MOCK_PAGES, MOCK_RECENT, MOCK_VAULT } from './mock-data'
import type { Backend, PageMeta, SearchHit, TreeNode, WriteResult } from './types'

export type MockOptions = {
  /** 起動時に開いている保管庫。null なら初回起動(フォルダ未選択)の状態 */
  vault?: string | null
  /** 空の保管庫で始める */
  empty?: boolean
  /** すべての呼び出しに足す遅延(ミリ秒)。ページ切り替えの確認用 */
  latencyMs?: number
}

const INBOX = 'Inbox.md'

const titleOf = (path: string) => (path.split('/').pop() ?? path).replace(/\.md$/, '')
const parentOf = (path: string) => path.split('/').slice(0, -1).join('/')
const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name)

/** メモリ上のダミーデータで動くバックエンド。ブラウザ単体(vite dev)で全画面を確認するためのもの。 */
export class MockBackend implements Backend {
  private vault: string | null
  private folders: string[]
  private pages = new Map<string, { content: string; modifiedAt: number }>()
  private recent: string[]
  private listeners = new Set<(path: string) => void>()
  private clock = Date.now()
  private latencyMs: number

  constructor(options: MockOptions = {}) {
    this.vault = options.vault === undefined ? MOCK_VAULT : options.vault
    this.latencyMs = options.latencyMs ?? 0
    const empty = options.empty ?? false
    this.folders = empty ? [] : [...MOCK_FOLDERS]
    if (!empty) for (const [path, content] of MOCK_PAGES) this.pages.set(path, { content, modifiedAt: this.tick() })
    this.recent = empty ? [] : [...MOCK_RECENT]
  }

  private tick(): number {
    this.clock = Math.max(this.clock + 1, Date.now())
    return this.clock
  }

  private async delay(): Promise<void> {
    if (this.latencyMs > 0) await new Promise((r) => setTimeout(r, this.latencyMs))
  }

  private meta(path: string): PageMeta {
    const page = this.pages.get(path)
    if (!page) throw new Error(`ページが見つかりません: ${path}`)
    return { path, title: titleOf(path), modifiedAt: page.modifiedAt }
  }

  private touch(path: string): void {
    this.recent = [path, ...this.recent.filter((p) => p !== path)].slice(0, 20)
  }

  private uniquePath(dir: string, title: string): string {
    const safe = title.replace(/[/\\:]/g, '-').trim() || '無題'
    let path = join(dir, `${safe}.md`)
    for (let n = 2; this.pages.has(path); n++) path = join(dir, `${safe} ${n}.md`)
    return path
  }

  async openVault(): Promise<string | null> {
    await this.delay()
    this.vault = MOCK_VAULT
    return this.vault
  }

  async listTree(): Promise<TreeNode[]> {
    await this.delay()
    // 契約には「いま開いている保管庫」を問い合わせる手段がないため、未選択なら失敗させる(提案中)
    if (this.vault === null) throw new Error('保管庫が選ばれていません')
    const nodes = new Map<string, TreeNode>()
    const root: TreeNode[] = []
    const add = (path: string) => {
      const node: TreeNode = { path, title: titleOf(path), children: [] }
      nodes.set(path, node)
      const parent = parentOf(path)
      ;(parent ? (nodes.get(parent)?.children ?? root) : root).push(node)
    }
    for (const folder of this.folders) add(folder)
    for (const path of this.pages.keys()) add(path)
    return root
  }

  async readPage(path: string): Promise<{ content: string; modifiedAt: number }> {
    await this.delay()
    const page = this.pages.get(path)
    if (!page) throw new Error(`ページが見つかりません: ${path}`)
    this.touch(path)
    return { ...page }
  }

  async writePage(path: string, content: string, baseModifiedAt: number): Promise<WriteResult> {
    await this.delay()
    const page = this.pages.get(path)
    if (!page || page.modifiedAt !== baseModifiedAt) return { ok: false, reason: 'conflict' }
    const modifiedAt = this.tick()
    this.pages.set(path, { content, modifiedAt })
    this.touch(path)
    return { ok: true, modifiedAt }
  }

  async createPage(parentPath: string | null, title: string): Promise<PageMeta> {
    await this.delay()
    // 親がページ(…/名前.md)なら、同じ名前のフォルダを作って子ページを入れる
    let dir = parentPath ?? ''
    if (dir.endsWith('.md')) {
      dir = dir.replace(/\.md$/, '')
      if (!this.folders.includes(dir)) this.folders.push(dir)
    }
    const path = this.uniquePath(dir, title)
    this.pages.set(path, { content: '', modifiedAt: this.tick() })
    this.touch(path)
    return this.meta(path)
  }

  async renamePage(path: string, newTitle: string): Promise<PageMeta> {
    await this.delay()
    const page = this.pages.get(path)
    if (!page) throw new Error(`ページが見つかりません: ${path}`)
    const next = this.uniquePath(parentOf(path), newTitle)
    // Map の並び順(=サイドバーの並び)を保ったまま差し替える
    this.pages = new Map([...this.pages].map(([p, v]) => (p === path ? [next, v] : [p, v])))
    // 子ページ用のフォルダがあれば一緒に名前を変える
    const oldDir = path.replace(/\.md$/, '')
    const newDir = next.replace(/\.md$/, '')
    const moved = (p: string) => (p === oldDir || p.startsWith(`${oldDir}/`) ? newDir + p.slice(oldDir.length) : p)
    this.folders = this.folders.map(moved)
    this.pages = new Map([...this.pages].map(([p, v]) => [moved(p), v]))
    this.recent = this.recent.map((p) => (p === path ? next : moved(p)))
    return this.meta(next)
  }

  async deletePage(path: string): Promise<void> {
    await this.delay()
    this.pages.delete(path)
    this.recent = this.recent.filter((p) => p !== path)
  }

  async search(query: string): Promise<SearchHit[]> {
    await this.delay()
    const q = query.trim().toLowerCase()
    if (!q) return []
    const hits: SearchHit[] = []
    for (const [path, { content }] of this.pages) {
      const title = titleOf(path)
      const at = content.toLowerCase().indexOf(q)
      if (!title.toLowerCase().includes(q) && at === -1) continue
      const snippet = at === -1 ? '' : `…${content.slice(Math.max(0, at - 10), at + q.length + 20).replace(/\s+/g, ' ')}…`
      hits.push({ path, title, snippet })
    }
    return hits
  }

  async recentPages(): Promise<PageMeta[]> {
    await this.delay()
    return this.recent.filter((p) => this.pages.has(p)).map((p) => this.meta(p))
  }

  async captureToInbox(text: string): Promise<void> {
    await this.delay()
    const inbox = this.pages.get(INBOX)
    const content = inbox ? `${inbox.content.replace(/\n*$/, '\n')}- ${text}\n` : `- ${text}\n`
    this.pages.set(INBOX, { content, modifiedAt: this.tick() })
    this.listeners.forEach((cb) => cb(INBOX))
  }

  onExternalChange(cb: (path: string) => void): () => void {
    this.listeners.add(cb)
    return () => this.listeners.delete(cb)
  }

  // ---- 開発用: 「他のアプリでファイルが変更された」状態を再現する ----

  /** 外部のアプリがファイルを書き換えたことにする */
  simulateExternalEdit(path: string, content: string): void {
    this.pages.set(path, { content, modifiedAt: this.tick() })
    this.listeners.forEach((cb) => cb(path))
  }
}
