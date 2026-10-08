import { create } from 'zustand'
import { backend, isBackendError, toBackendError, type PageMeta, type TreeNode } from '@/backend'
import { activeSession, flushActive } from '@/editor/registry'

export const RECENT_LIMIT = 20
export const SEARCH_LIMIT = 50

type PageContent = { content: string; version: string }

/**
 * 本文に表示しているページ。
 * loadId はディスクから読んだ内容で文書を差し替えるたびに増える。保存結果の反映では増えない。
 * unreadable は開けないファイル(UTF-8 でないなど)。エディタを出さない。
 */
export type OpenPage = { path: string; loadId: number } & ({ content: string; version: string; unreadable?: undefined } | { unreadable: 'notUtf8' })

export type NoticeAction = { label: string; run: () => void | Promise<void> }
/** 画面上部の通知(デザイン 7)。warning は確認が必要なもの、info は知らせるだけのもの */
export type Notice = { id: number; tone: 'warning' | 'info'; title: string; body?: string; actions: NoticeAction[] }

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
  notice: Notice | null
  sidebarCollapsed: boolean

  init(): Promise<void>
  openVault(): Promise<void>
  refreshTree(): Promise<void>
  openPage(path: string): Promise<void>
  prefetch(path: string): void
  closePage(): void
  /** ディスクの内容で、開いているページを差し替える(未保存の変更がないときだけ呼ぶ) */
  replaceOpenPage(path: string, fresh: PageContent): void
  /** 保存できた内容を控えに反映する。文書は差し替えない */
  markSaved(path: string, content: string, version: string): void
  handleExternalChange(): Promise<void>
  createPage(parentPath: string | null, title: string): Promise<PageMeta | null>
  renamePage(path: string, newTitle: string): Promise<PageMeta | null>
  deletePage(path: string): Promise<void>
  showNotice(notice: Omit<Notice, 'id'>): void
  dismissNotice(id?: number): void
  toggleSidebar(): void
}

let openSeq = 0
let loadSeq = 0
let noticeSeq = 0
let noticeTimer: ReturnType<typeof setTimeout> | undefined

const findNode = (nodes: TreeNode[], path: string): TreeNode | null => {
  for (const n of nodes) {
    if (n.path === path) return n
    const hit = findNode(n.children, path)
    if (hit) return hit
  }
  return null
}

export const useApp = create<AppState>()((set, get) => ({
  status: 'loading',
  tree: [],
  recent: [],
  selectedPath: null,
  page: null,
  cache: {},
  notice: null,
  sidebarCollapsed: false,

  async init() {
    const vault = await backend().currentVault()
    if (vault === null) {
      set({ status: 'no-vault' })
      return
    }
    const [tree, recent] = await Promise.all([backend().listTree(), backend().recentPages(RECENT_LIMIT)])
    set({ status: 'ready', tree, recent })
    const first = recent[0]?.path
    if (first) await get().openPage(first)
  },

  async openVault() {
    const vault = await backend().openVault()
    if (vault !== null) await get().init()
  },

  async refreshTree() {
    const [tree, recent] = await Promise.all([backend().listTree(), backend().recentPages(RECENT_LIMIT)])
    set({ tree, recent })
  },

  async openPage(path) {
    if (get().page?.path === path && get().selectedPath === path) return
    // 前のページの未保存の変更を書き出し始める(書き出す内容はこの時点で決まる。待たずに切り替える)
    void flushActive()
    const seq = ++openSeq
    const cached = get().cache[path]
    set(cached ? { selectedPath: path, page: { path, ...cached, loadId: ++loadSeq } } : { selectedPath: path })
    try {
      const fresh = await backend().readPage(path)
      set((s) => ({ cache: { ...s.cache, [path]: fresh } }))
      // 読み込み中に別のページへ移っていたら、表示は差し替えない
      if (seq !== openSeq) return
      if (!cached || cached.version !== fresh.version) set({ page: { path, ...fresh, loadId: ++loadSeq } })
    } catch (e) {
      if (seq !== openSeq) return
      const err = toBackendError(e)
      if (err.kind === 'notUtf8') set({ page: { path, unreadable: 'notUtf8', loadId: ++loadSeq } })
      else if (err.kind === 'notFound') {
        set({ selectedPath: get().page?.path ?? null })
        get().showNotice({ tone: 'warning', title: 'このページは見つかりませんでした', body: '他のアプリで移動または削除された可能性があります。', actions: [] })
        void get().refreshTree()
      } else {
        get().showNotice({ tone: 'warning', title: 'ページを開けませんでした', body: err.message, actions: [] })
      }
      return
    }
    void backend()
      .recentPages(RECENT_LIMIT)
      .then((recent) => set({ recent }))
  },

  prefetch(path) {
    if (get().cache[path]) return
    void backend()
      .readPage(path)
      .then((fresh) => set((s) => ({ cache: { [path]: fresh, ...s.cache } })))
      .catch(() => {})
  },

  closePage() {
    set({ page: null, selectedPath: null })
  },

  replaceOpenPage(path, fresh) {
    set((s) => ({
      cache: { ...s.cache, [path]: fresh },
      page: s.page?.path === path ? { path, ...fresh, loadId: ++loadSeq } : s.page,
    }))
  },

  markSaved(path, content, version) {
    set((s) => {
      const page = s.page?.path === path && !s.page.unreadable ? { ...s.page, content, version } : s.page
      return { cache: { ...s.cache, [path]: { content, version } }, page }
    })
  },

  async handleExternalChange() {
    await get().refreshTree()
    const page = get().page
    if (!page || page.unreadable) return
    // 保存中なら書き終えてから比べる(自分の保存による変更を外部の変更と取り違えない)
    await flushActive()
    const session = activeSession()
    const known = session?.current()
    if (!known || known.path !== page.path) return
    let fresh: PageContent
    try {
      fresh = await backend().readPage(page.path)
    } catch (e) {
      if (isBackendError(e, 'notFound')) {
        if (session?.isDirty()) {
          // 未保存の変更がある: 保存時と同じ「作り直す / 破棄する」を尋ねる(エディタ側が次の保存で出す)
          void session.flush()
        } else {
          get().closePage()
        }
      }
      return
    }
    if (fresh.version === known.version) return
    if (!session?.isDirty()) {
      get().replaceOpenPage(page.path, fresh)
      return
    }
    get().showNotice({
      tone: 'warning',
      title: 'このファイルは、他のアプリで変更されました。',
      body: '最新の内容を読み込みますか？',
      actions: [
        { label: '再読み込み', run: () => get().replaceOpenPage(page.path, fresh) },
        // 自分の変更で上書きする: 保存の基準を最新の version にしてから保存する
        { label: '自分の変更を保持', run: () => activeSession()?.overwriteWith(fresh.version) },
      ],
    })
  },

  async createPage(parentPath, title) {
    await flushActive()
    try {
      const meta = await backend().createPage(parentPath, title)
      await get().refreshTree()
      return meta
    } catch (e) {
      get().showNotice({ tone: 'warning', title: 'ページを作れませんでした', body: toBackendError(e).message, actions: [] })
      return null
    }
  },

  async renamePage(path, newTitle) {
    await flushActive()
    try {
      const meta = await backend().renamePage(path, newTitle)
      await get().refreshTree()
      // ほかのページへのリンクが書き換えられ version が変わるので、開いているページを読み直す
      const open = get().page
      if (open) {
        const openPath = open.path === path ? meta.path : open.path
        const fresh = await backend().readPage(openPath)
        set((s) => ({ selectedPath: openPath, cache: { ...s.cache, [openPath]: fresh }, page: { path: openPath, ...fresh, loadId: ++loadSeq } }))
      }
      return meta
    } catch (e) {
      const err = toBackendError(e)
      // タイトルはツリーから表示しているので、取り直せば元に戻る
      await get().refreshTree()
      get().showNotice({
        tone: 'warning',
        title: err.kind === 'nameOccupied' ? '同じ名前のページがあります' : '名前を変えられませんでした',
        body: err.kind === 'nameOccupied' ? '別の名前にしてください。' : err.message,
        actions: [],
      })
      return null
    }
  },

  async deletePage(path) {
    await flushActive()
    try {
      await backend().deletePage(path)
    } catch (e) {
      get().showNotice({ tone: 'warning', title: '削除できませんでした', body: toBackendError(e).message, actions: [] })
      return
    }
    if (get().page?.path === path) get().closePage()
    set((s) => {
      const cache = { ...s.cache }
      delete cache[path]
      return { cache }
    })
    await get().refreshTree()
    get().showNotice({ tone: 'info', title: 'ゴミ箱に移動しました', body: '元に戻すときは、Finder のゴミ箱から戻してください。', actions: [] })
  },

  showNotice(notice) {
    const id = ++noticeSeq
    clearTimeout(noticeTimer)
    set({ notice: { ...notice, id } })
    // 知らせるだけの通知は、しばらくしたら消す
    if (notice.tone === 'info') noticeTimer = setTimeout(() => get().dismissNotice(id), 5000)
  },

  dismissNotice(id) {
    if (id === undefined || get().notice?.id === id) set({ notice: null })
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

export const isPage = (node: TreeNode) => node.kind === 'page'

/** ページのタイトル(バックエンドが返したもの)。ツリーにまだなければ null */
export const titleOf = (tree: TreeNode[], path: string): string | null => findNode(tree, path)?.title ?? null
