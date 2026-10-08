import { backend, type PageMeta, type SearchHit, type TreeNode } from '@/backend'
import { ancestorsOf, SEARCH_LIMIT } from '@/store/app'

export const PALETTE_RECENT_LIMIT = 10

export type PaletteCommand = { id: string; label: string; keys?: string; aliases: string[] }

/** パレットのコマンド(ショートカットのある主要な操作) */
export const PALETTE_COMMANDS: PaletteCommand[] = [
  { id: 'new-page', label: '新規ページを作成', keys: '⌘N', aliases: ['new', 'page', 'しんき', 'ぺーじ', 'ページ', '作成', '新規'] },
  { id: 'new-child', label: '子ページを作成', keys: '⌘⇧N', aliases: ['child', 'page', 'こ', 'ぺーじ', 'ページ', '作成'] },
  { id: 'toggle-sidebar', label: 'サイドバーを開閉', keys: '⌘\\', aliases: ['sidebar', 'さいどばー', '開く', '閉じる'] },
]

/** ページの一覧に出す1行。where はツリー上の場所(祖先のタイトルを「 / 」でつないだもの) */
export type PageRow = { path: string; title: string; where: string }

/**
 * パレットに表示する結果の組。いつも1回の問い合わせの結果だけで作り、まとめて差し替える
 * (ページ・本文・コマンドが別々の検索語の結果になることはない)。
 */
export type PaletteResults =
  | { kind: 'recent'; recent: PageRow[] }
  | { kind: 'search'; query: string; pages: PageRow[]; hits: SearchHit[]; commands: PaletteCommand[] }

const normalize = (s: string) => s.normalize('NFKC').toLowerCase()

export const pageRow = (tree: TreeNode[], meta: Pick<PageMeta, 'path' | 'title'>): PageRow => ({
  path: meta.path,
  title: meta.title,
  where: ancestorsOf(tree, meta.path)
    .slice(0, -1)
    .map((n) => n.title)
    .join(' / '),
})

export function matchCommands(query: string): PaletteCommand[] {
  const q = normalize(query.trim())
  return PALETTE_COMMANDS.filter((c) => [c.label, ...c.aliases].some((s) => normalize(s).includes(q)))
}

/** 入力が空のときの結果(最近開いたページ) */
export const recentResults = (tree: TreeNode[], recent: PageMeta[]): PaletteResults => ({
  kind: 'recent',
  recent: recent.slice(0, PALETTE_RECENT_LIMIT).map((m) => pageRow(tree, m)),
})

/**
 * パレットの検索の順番を管理する。
 * - 最後に頼んだ検索語の結果だけを知らせる(古い問い合わせの結果が後から届いても捨てる)
 * - 結果が届くまでは何も知らせない(表示は直前の結果のまま)
 * - 失敗したときも知らせない(直前の結果を残す)
 */
export class PaletteSearch {
  private seq = 0
  private last: string | null = null
  private readonly tree: () => TreeNode[]
  private readonly onResults: (results: PaletteResults) => void

  constructor(tree: () => TreeNode[], onResults: (results: PaletteResults) => void) {
    this.tree = tree
    this.onResults = onResults
  }

  /** 検索語を確定して問い合わせる。前と同じ検索語なら何もしない */
  run(query: string): void {
    if (query === this.last) return
    this.last = query
    const seq = ++this.seq
    const q = query.trim()
    const done = (results: PaletteResults) => {
      if (seq === this.seq) this.onResults(results)
    }
    if (!q) {
      backend()
        .recentPages(PALETTE_RECENT_LIMIT)
        .then((recent) => done(recentResults(this.tree(), recent)))
        .catch(() => {})
      return
    }
    backend()
      .search(q, SEARCH_LIMIT)
      .then((found) => {
        // 振り分けはバックエンドの titleMatched だけで行う(並び順もバックエンドのまま)
        const tree = this.tree()
        const pages = found.filter((h) => h.titleMatched).map((h) => pageRow(tree, h))
        const hits = found.filter((h) => !h.titleMatched)
        done({ kind: 'search', query, pages, hits, commands: matchCommands(q) })
      })
      .catch(() => {})
  }

  /** パレットを開き直したときに、次の run を必ず走らせる */
  reset(): void {
    this.last = null
    this.seq++
  }
}
