import { create } from 'zustand'
import { backend, type PageMeta, type TreeNode } from '@/backend'

type PageContent = { content: string; modifiedAt: number }
/** loadId: ディスクから読んだ内容で文書を差し替えるたびに増える。保存結果の反映では増えない */
export type OpenPage = PageContent & { path: string; loadId: number }

type AppState = {
  /** loading: 起動中 / no-vault: 初回起動(保管庫未選択) / ready: 使える */
  status: 'loading' | 'no-vault' | 'ready'
  tree: TreeNode[]
  recent: PageMeta[]
  /** サイドバーで選ばれているページ。クリックした瞬間に変わる */
  selectedPath: string | null
  /** 本文に表示しているページ。読み込みが終わるまでは前のページのまま(画面を空にしない) */
  page: OpenPage | null
  /** 一度読んだページの控え。再訪問は待たずに表示する */
  cache: Record<string, PageContent>
  sidebarCollapsed: boolean

  init(): Promise<void>
  openVault(): Promise<void>
  refreshTree(): Promise<void>
  openPage(path: string): Promise<void>
  prefetch(path: string): void
  toggleSidebar(): void
}

let openSeq = 0
let loadSeq = 0

export const useApp = create<AppState>()((set, get) => ({
  status: 'loading',
  tree: [],
  recent: [],
  selectedPath: null,
  page: null,
  cache: {},
  sidebarCollapsed: false,

  async init() {
    try {
      const [tree, recent] = await Promise.all([backend().listTree(), backend().recentPages()])
      set({ status: 'ready', tree, recent })
      const first = recent[0]?.path
      if (first) await get().openPage(first)
    } catch {
      // 契約に「いまの保管庫」を問い合わせる手段がないため、listTree の失敗を初回起動とみなす(提案中)
      set({ status: 'no-vault' })
    }
  },

  async openVault() {
    const vault = await backend().openVault()
    if (vault !== null) await get().init()
  },

  async refreshTree() {
    const [tree, recent] = await Promise.all([backend().listTree(), backend().recentPages()])
    set({ tree, recent })
  },

  async openPage(path) {
    const seq = ++openSeq
    const cached = get().cache[path]
    set(cached ? { selectedPath: path, page: { path, ...cached, loadId: ++loadSeq } } : { selectedPath: path })
    const fresh = await backend().readPage(path)
    set((s) => ({ cache: { ...s.cache, [path]: fresh } }))
    // 読み込み中に別のページへ移っていたら、表示は差し替えない
    if (seq !== openSeq) return
    if (!cached || cached.modifiedAt !== fresh.modifiedAt) set({ page: { path, ...fresh, loadId: ++loadSeq } })
    void backend()
      .recentPages()
      .then((recent) => set({ recent }))
  },

  prefetch(path) {
    if (get().cache[path]) return
    void backend()
      .readPage(path)
      .then((fresh) => set((s) => ({ cache: { [path]: fresh, ...s.cache } })))
      .catch(() => {})
  },

  toggleSidebar() {
    set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed }))
  },
}))

/** ツリーから path までの祖先(自分を含む)をたどる。パンくずに使う。 */
export function ancestorsOf(tree: TreeNode[], path: string): TreeNode[] {
  for (const node of tree) {
    if (node.path === path) return [node]
    const below = ancestorsOf(node.children, path)
    if (below.length > 0) return [node, ...below]
  }
  return []
}

/** 契約の TreeNode にはフォルダかページかの区別がないので、拡張子で見分ける(提案中) */
export const isPage = (node: TreeNode) => node.path.endsWith('.md')
