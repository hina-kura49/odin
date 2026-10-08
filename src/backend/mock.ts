import { dirOf, hasScheme, resolveInVault } from '@/lib/paths'
import { MOCK_FOLDERS, MOCK_PAGES, MOCK_RECENT, MOCK_SPECIAL, MOCK_VAULT } from './mock-data'
import { BackendError, type Backend, type PageMeta, type SearchHit, type TreeNode, type WriteResult } from './types'

export type MockOptions = {
  /** 前回開いた保管庫。null なら初回起動(フォルダ未選択)の状態 */
  vault?: string | null
  /** 空の保管庫で始める */
  empty?: boolean
  /** すべての呼び出しに足す遅延(ミリ秒)。ページ切り替えの確認用 */
  latencyMs?: number
}

const INBOX = 'Inbox.md'

type MockPage = { content: string; version: string; modifiedAt: number; readOnly?: boolean; notUtf8?: boolean }

const titleOf = (path: string) => (path.split('/').pop() ?? path).replace(/\.md$/, '')
const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name)

const safeDecode = (s: string) => {
  try {
    return decodeURI(s)
  } catch {
    return s
  }
}

/** バックエンドと同じように、タイトルを整える(空白の除去、使えない文字の置き換え) */
const normalizeTitle = (title: string) => title.replace(/[/\\]/g, '-').replace(/\s+/g, ' ').trim() || '無題'

/** メモリ上のダミーデータで動くバックエンド。ブラウザ単体(vite dev)で全画面を確認するためのもの。 */
export class MockBackend implements Backend {
  private vault: string | null
  private folders: string[]
  private pages = new Map<string, MockPage>()
  private recent: string[]
  private listeners = new Set<() => void>()
  private clock = Date.now()
  private versionSeq = 0
  private latencyMs: number

  constructor(options: MockOptions = {}) {
    this.vault = options.vault === undefined ? MOCK_VAULT : options.vault
    this.latencyMs = options.latencyMs ?? 0
    const empty = options.empty ?? false
    this.folders = empty ? [] : [...MOCK_FOLDERS]
    if (!empty) {
      for (const [path, content] of MOCK_PAGES) this.put(path, content)
      for (const [path, content, flags] of MOCK_SPECIAL) this.put(path, content, flags)
    }
    this.recent = empty ? [] : [...MOCK_RECENT]
  }

  private put(path: string, content: string, flags: Partial<Pick<MockPage, 'readOnly' | 'notUtf8'>> = {}): MockPage {
    this.clock = Math.max(this.clock + 1, Date.now())
    const page = { content, version: `v${++this.versionSeq}`, modifiedAt: this.clock, ...flags }
    this.pages.set(path, page)
    return page
  }

  private async delay(): Promise<void> {
    if (this.latencyMs > 0) await new Promise((r) => setTimeout(r, this.latencyMs))
  }

  private page(path: string): MockPage {
    const page = this.pages.get(path)
    if (!page) throw new BackendError('notFound', `ページが見つかりません: ${path}`)
    return page
  }

  private meta(path: string): PageMeta {
    return { path, title: titleOf(path), modifiedAt: this.page(path).modifiedAt }
  }

  private touch(path: string): void {
    this.recent = [path, ...this.recent.filter((p) => p !== path)].slice(0, 100)
  }

  private uniquePath(dir: string, title: string): string {
    let path = join(dir, `${title}.md`)
    for (let n = 2; this.pages.has(path); n++) path = join(dir, `${title} ${n}.md`)
    return path
  }

  private notify(): void {
    this.listeners.forEach((cb) => cb())
  }

  async currentVault(): Promise<string | null> {
    await this.delay()
    return this.vault
  }

  async openVault(): Promise<string | null> {
    await this.delay()
    this.vault = MOCK_VAULT
    return this.vault
  }

  async listTree(): Promise<TreeNode[]> {
    await this.delay()
    // 本物のバックエンドと同じく、ページと同じ名前のフォルダの中身は、そのページの子として並べる
    const nodes = new Map<string, TreeNode>()
    const root: TreeNode[] = []
    const container = (dir: string) => (dir ? (nodes.get(`${dir}.md`) ?? nodes.get(dir))?.children : root)
    const add = (path: string, kind: TreeNode['kind']) => {
      const node: TreeNode = { path, title: titleOf(path), kind, children: [] }
      nodes.set(path, node)
      ;(container(dirOf(path)) ?? root).push(node)
    }
    const pageDirs = new Set([...this.pages.keys()].map((p) => p.replace(/\.md$/, '')))
    // フォルダは浅い順に足す(親が先にあるように)。同じ名前のページがあるフォルダは、ページに合わせる
    for (const folder of [...this.folders].sort((a, b) => a.split('/').length - b.split('/').length))
      if (!pageDirs.has(folder)) add(folder, 'folder')
    for (const path of [...this.pages.keys()].sort((a, b) => a.split('/').length - b.split('/').length)) add(path, 'page')
    return root
  }

  async readPage(path: string): Promise<{ content: string; version: string }> {
    await this.delay()
    const page = this.page(path)
    if (page.notUtf8) throw new BackendError('notUtf8', `UTF-8 ではありません: ${path}`)
    this.touch(path)
    return { content: page.content, version: page.version }
  }

  async writePage(path: string, content: string, baseVersion: string): Promise<WriteResult> {
    await this.delay()
    const page = this.page(path)
    if (page.readOnly) throw new BackendError('readOnly', `読み取り専用です: ${path}`)
    if (page.version !== baseVersion) return { ok: false, reason: 'conflict' }
    const next = this.put(path, content)
    this.touch(path)
    return { ok: true, version: next.version }
  }

  async createPage(parentPath: string | null, title: string): Promise<PageMeta> {
    await this.delay()
    // 親がページ(…/名前.md)なら、同じ名前のフォルダを作って子ページを入れる
    let dir = parentPath ?? ''
    if (this.pages.has(dir)) {
      dir = dir.replace(/\.md$/, '')
      if (!this.folders.includes(dir)) this.folders.push(dir)
    } else if (dir && !this.folders.includes(dir)) {
      throw new BackendError('notFound', `フォルダが見つかりません: ${dir}`)
    }
    const path = this.uniquePath(dir, normalizeTitle(title))
    this.put(path, '')
    this.touch(path)
    return this.meta(path)
  }

  async renamePage(path: string, newTitle: string): Promise<PageMeta> {
    await this.delay()
    this.page(path)
    const next = join(dirOf(path), `${normalizeTitle(newTitle)}.md`)
    if (next === path) return this.meta(path)
    if (this.pages.has(next)) throw new BackendError('nameOccupied', `同じ名前のページがあります: ${next}`)
    // Map の並び順(=サイドバーの並び)を保ったまま差し替える。子ページ用のフォルダも一緒に名前を変える
    const oldDir = path.replace(/\.md$/, '')
    const newDir = next.replace(/\.md$/, '')
    const moved = (p: string) => (p === path ? next : p === oldDir || p.startsWith(`${oldDir}/`) ? newDir + p.slice(oldDir.length) : p)
    this.pages = new Map([...this.pages].map(([p, v]) => [moved(p), v]))
    this.folders = this.folders.map(moved)
    this.recent = this.recent.map(moved)
    // 本物のバックエンドはほかのページからのリンクを書き換える。Mock では version を進めて同じ状況を再現する
    const renamed = this.page(next)
    this.put(next, renamed.content, { readOnly: renamed.readOnly })
    return this.meta(next)
  }

  async deletePage(path: string): Promise<void> {
    await this.delay()
    this.page(path)
    // 子ページ(同じ名前のフォルダの中身)も一緒にゴミ箱へ移す
    const dir = path.replace(/\.md$/, '')
    const gone = (p: string) => p === path || p === dir || p.startsWith(`${dir}/`)
    for (const p of [...this.pages.keys()]) if (gone(p)) this.pages.delete(p)
    this.folders = this.folders.filter((f) => !gone(f))
    this.recent = this.recent.filter((p) => !gone(p))
  }

  async search(query: string, limit: number): Promise<SearchHit[]> {
    await this.delay()
    const q = query.trim().toLowerCase()
    if (!q) return []
    const hits: SearchHit[] = []
    for (const [path, page] of this.pages) {
      if (hits.length >= limit) break
      if (page.notUtf8) continue
      const title = titleOf(path)
      const text = page.content.replace(/\s+/g, ' ')
      const at = text.toLowerCase().indexOf(q)
      if (at === -1 && !title.toLowerCase().includes(q)) continue
      const snippet =
        at === -1
          ? { before: text.slice(0, 40), hit: '', after: '' }
          : { before: text.slice(Math.max(0, at - 20), at), hit: text.slice(at, at + q.length), after: text.slice(at + q.length, at + q.length + 30) }
      hits.push({ path, title, snippet })
    }
    return hits
  }

  async recentPages(limit: number): Promise<PageMeta[]> {
    await this.delay()
    return this.recent
      .filter((p) => this.pages.has(p))
      .slice(0, limit)
      .map((p) => this.meta(p))
  }

  async captureToInbox(text: string): Promise<void> {
    await this.delay()
    const inbox = this.pages.get(INBOX)
    const content = inbox ? `${inbox.content.replace(/\n*$/, '\n')}- ${text}\n` : `- ${text}\n`
    this.put(INBOX, content)
    this.notify()
  }

  assetUrl(pagePath: string, src: string): string | null {
    if (hasScheme(src)) return src
    const path = resolveInVault(dirOf(pagePath), safeDecode(src))
    if (path === null) return null
    // Mock には画像ファイルがないので、パスごとに決まった仮の画像を返す
    return `https://picsum.photos/seed/${encodeURIComponent(path)}/1200/800`
  }

  onExternalChange(cb: () => void): () => void {
    this.listeners.add(cb)
    return () => this.listeners.delete(cb)
  }

  // ---- 開発用: 外部のアプリによる変更を再現する(開発者ツールから __mock.xxx で呼べる) ----

  /** 外部のアプリがファイルを書き換えた */
  simulateExternalEdit(path: string, content: string): void {
    this.put(path, content, { readOnly: this.pages.get(path)?.readOnly })
    this.notify()
  }

  /** 外部のアプリがファイルを消した */
  simulateExternalDelete(path: string): void {
    this.pages.delete(path)
    this.notify()
  }

  /** 読み取り専用にする / 戻す */
  setReadOnly(path: string, readOnly: boolean): void {
    const page = this.pages.get(path)
    if (page) page.readOnly = readOnly
  }
}
