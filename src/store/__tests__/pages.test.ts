// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { BackendError, setBackend } from '@/backend'
import { MockBackend } from '@/backend/mock'
import { countPages, findParentFolder, firstPage, useApp } from '../app'

const initialState = useApp.getState()
let mock: MockBackend

beforeEach(async () => {
  mock = new MockBackend()
  setBackend(mock)
  useApp.setState(initialState, true)
  await useApp.getState().init()
})

const app = () => useApp.getState()
const treeHas = (path: string) => JSON.stringify(app().tree).includes(`"path":"${path}"`)

describe('新規ページ', () => {
  it('⌘N / 新規ページ: 開いているページと同じフォルダに「無題」を作って開き、タイトルを選んだ状態にする', async () => {
    await app().openPage('仕事/企画/新しいプロダクトの考え方.md')
    await app().newPage('sibling')
    expect(app().page?.path).toBe('仕事/企画/無題.md')
    expect(treeHas('仕事/企画/無題.md')).toBe(true)
    expect(app().titleFocus).toMatchObject({ path: '仕事/企画/無題.md', select: true })
  })

  it('もう一度作ると、バックエンドが連番を付けたタイトルを正とする', async () => {
    await app().openPage('仕事/企画/新しいプロダクトの考え方.md')
    await app().newPage('sibling')
    await app().newPage('sibling')
    expect(app().page?.path).toBe('仕事/企画/無題 2.md')
  })

  it('⌘⇧N: 開いているページの子ページを作る', async () => {
    await app().openPage('仕事/企画/新しいプロダクトの考え方.md')
    await app().newPage('child')
    expect(app().page?.path).toBe('仕事/企画/新しいプロダクトの考え方/無題.md')
  })

  it('フォルダを指定して作る(ツリーのメニュー)', async () => {
    await app().newPage({ folder: '学び/技術' })
    expect(app().page?.path).toBe('学び/技術/無題.md')
  })

  it('ページを開いていなければ、ルートに作る', async () => {
    app().closePage()
    await app().newPage('sibling')
    expect(app().page?.path).toBe('無題.md')
  })
})

describe('削除', () => {
  it('開いているページを消したら「ゴミ箱に移動しました」と知らせ、最近開いたページを開く', async () => {
    await app().openPage('日記/2025/今日の振り返り.md')
    await app().deletePage('日記/2025/今日の振り返り.md')
    expect(app().notice?.title).toBe('ゴミ箱に移動しました')
    expect(treeHas('日記/2025/今日の振り返り.md')).toBe(false)
    expect(app().page?.path).toBe('仕事/企画/新しいプロダクトの考え方.md')
  })

  it('開いていないページを消しても、開いているページはそのまま', async () => {
    await app().openPage('日記/2025/今日の振り返り.md')
    await app().deletePage('学び/技術/技術調査メモ.md')
    expect(app().page?.path).toBe('日記/2025/今日の振り返り.md')
  })
})

describe('削除の確認', () => {
  const PARENT = '検証/子ページのあるページ.md'

  it('子ページを持つページは、件数を示して確認する(まだ消さない)', async () => {
    await app().requestDelete(PARENT)
    expect(app().notice).toMatchObject({ tone: 'warning', title: 'このページと子ページ 3 件をゴミ箱に移動します', focus: true })
    expect(app().notice?.actions.map((a) => a.label)).toEqual(['ゴミ箱に移動', 'キャンセル'])
    expect(treeHas(PARENT)).toBe(true)
  })

  it('「ゴミ箱に移動」で子ページごと消え、開いていた子ページも閉じて最近のページを開く', async () => {
    await app().openPage('検証/子ページのあるページ/子ページ2/孫ページ.md')
    await app().requestDelete(PARENT)
    await app().notice?.actions[0].run()
    expect(treeHas(PARENT)).toBe(false)
    expect(treeHas('検証/子ページのあるページ/子ページ2/孫ページ.md')).toBe(false)
    expect(app().notice?.title).toBe('ゴミ箱に移動しました')
    expect(app().page?.path).not.toMatch(/^検証\/子ページのあるページ/)
  })

  it('「キャンセル」では何も消さない', async () => {
    await app().requestDelete(PARENT)
    await app().notice?.actions[1].run()
    expect(treeHas(PARENT)).toBe(true)
  })

  it('子を持たないページは、確認なしで削除する', async () => {
    await app().requestDelete('学び/技術/技術調査メモ.md')
    expect(treeHas('学び/技術/技術調査メモ.md')).toBe(false)
    expect(app().notice?.title).toBe('ゴミ箱に移動しました')
  })

  it('ツリーでは、同じ名前のフォルダの中身がページの子として並ぶ(バックエンドと同じ形)', () => {
    const parent = app().tree.find((n) => n.path === '検証')?.children.find((n) => n.path === PARENT)
    expect(parent?.kind).toBe('page')
    expect(countPages(parent?.children ?? [])).toBe(3)
  })
})

describe('ツリーとの連携', () => {
  it('ページを開くと、ツリーにそのページの行を見せる合図が出る', async () => {
    await app().openPage('学び/技術/技術調査メモ.md')
    expect(app().reveal?.path).toBe('学び/技術/技術調査メモ.md')
  })

  it('ページの入っているフォルダは、path を加工せずツリーの親子関係から求める', () => {
    const tree = app().tree
    expect(findParentFolder(tree, '仕事/会議メモ/10:00 打ち合わせ.md')).toBe('仕事/会議メモ')
    expect(findParentFolder(tree, 'Inbox.md')).toBeNull()
  })
})

describe('クイックキャプチャのホットキー', () => {
  it('登録できなかったことを、既存の通知で知らせる', () => {
    useApp.getState().warnCaptureShortcut()
    expect(useApp.getState().notice).toMatchObject({
      tone: 'warning',
      title: '⌃⌥Space はほかのアプリが使っているため、クイックキャプチャのショートカットを登録できませんでした',
      actions: [],
    })
  })
})

describe('開くページがないとき', () => {
  it('最近開いたページがなければ、ツリーの最初のページを開く', async () => {
    const m = new MockBackend()
    vi.spyOn(m, 'recentPages').mockResolvedValue([])
    setBackend(m)
    useApp.setState(initialState, true)
    await useApp.getState().init()
    expect(useApp.getState().page?.path).toBe(firstPage(useApp.getState().tree))
  })
})

describe('保管庫の選び直し', () => {
  it('未保存の変更を保存してから選び直し、前の保管庫のページを残さない', async () => {
    const m = new MockBackend()
    setBackend(m)
    useApp.setState(initialState, true)
    await useApp.getState().init()
    expect(useApp.getState().page).not.toBeNull()
    const order: string[] = []
    vi.spyOn(m, 'openVault').mockImplementation(async () => {
      order.push('open')
      expect(useApp.getState().page).not.toBeNull()
      return '/別の保管庫'
    })
    vi.spyOn(m, 'listTree').mockResolvedValue([])
    vi.spyOn(m, 'recentPages').mockResolvedValue([])
    await useApp.getState().openVault()
    expect(order).toEqual(['open'])
    expect(useApp.getState()).toMatchObject({ status: 'ready', page: null, tree: [], cache: {} })
  })

  it('キャンセルしたら、いまの保管庫のまま', async () => {
    const m = new MockBackend()
    setBackend(m)
    useApp.setState(initialState, true)
    await useApp.getState().init()
    const page = useApp.getState().page
    vi.spyOn(m, 'openVault').mockResolvedValue(null)
    await useApp.getState().openVault()
    expect(useApp.getState().page).toBe(page)
  })
})

describe('ツリーを取り直せなかったとき', () => {
  it('外部の変更で取り直せなくても、投げずに通知し、いまのツリーを残す', async () => {
    const tree = useApp.getState().tree
    vi.spyOn(mock, 'listTree').mockRejectedValueOnce(new BackendError('io', 'not yet implemented'))
    await useApp.getState().handleExternalChange()
    expect(useApp.getState().tree).toBe(tree)
    expect(useApp.getState().notice).toMatchObject({ tone: 'warning', title: 'ページの一覧を読み直せませんでした' })
  })
})
