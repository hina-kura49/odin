// @vitest-environment jsdom
import { Editor, schemaCtx, serializerCtx } from '@milkdown/kit/core'
import { describe, expect, it } from 'vitest'
import { encodeLinkDestination, relativePath, resolveInVault } from '@/lib/paths'
import { configureMarkdown } from '../markdown'
import { pageLinkDestination, pageLinkNode } from '../page-link'

/** リンク1つだけの段落を、Milkdown で Markdown に書き出す */
async function serializeLink(current: string, path: string, title: string): Promise<string> {
  const editor = await configureMarkdown(Editor.make()).create()
  const out = editor.action((ctx) => {
    const schema = ctx.get(schemaCtx)
    const para = schema.nodes.paragraph.create(null, pageLinkNode(schema, current, { path, title, modifiedAt: 0 }))
    return ctx.get(serializerCtx)(schema.topNodeType.create(null, para))
  })
  await editor.destroy()
  return out.trim()
}

describe('/page が挿入するリンクの行き先', () => {
  it.each([
    ['同じフォルダ', '仕事/企画/今のページ.md', '仕事/企画/新しいページ.md', '新しいページ.md'],
    ['子ページ(今のページと同名のフォルダ)', '仕事/企画/今のページ.md', '仕事/企画/今のページ/子.md', '今のページ/子.md'],
    ['別のフォルダ', '仕事/企画/今のページ.md', '仕事/会議メモ/週次.md', '../会議メモ/週次.md'],
    ['ルートのページから', '今のページ.md', '仕事/新しいページ.md', '仕事/新しいページ.md'],
    ['ルートのページへ', '仕事/企画/今のページ.md', '新しいページ.md', '../../新しいページ.md'],
  ])('%s', (_, current, target, expected) => {
    expect(pageLinkDestination(current, target)).toBe(expected)
  })
})

describe('行き先のエンコード: 空白 ( ) < > # % ? と制御文字だけ、16進数は大文字', () => {
  it.each([
    ['空白', '週次 定例.md', '週次%20定例.md'],
    ['括弧', 'メモ(下書き).md', 'メモ%28下書き%29.md'],
    ['#', 'C# の調べもの.md', 'C%23%20の調べもの.md'],
    ['< > % ?', 'a<b>c%d?.md', 'a%3Cb%3Ec%25d%3F.md'],
    ['日本語はそのまま', '日本語のタイトル.md', '日本語のタイトル.md'],
    ['「:」もそのまま', '10:00 打ち合わせ.md', '10:00%20打ち合わせ.md'],
    ['制御文字(タブ・改行)', 'a\tb\nc.md', 'a%09b%0Ac.md'],
    ['全角空白は空白として扱う', '全角　空白.md', '全角%E3%80%80空白.md'],
  ])('%s', (_, input, expected) => {
    expect(encodeLinkDestination(input)).toBe(expected)
  })
})

describe('Markdown に書き出したリンク: [表示文字列](行き先)、<...> で囲まない', () => {
  it.each([
    ['空白', '週次 定例', '仕事/企画/週次 定例.md', '[週次 定例](週次%20定例.md)'],
    ['括弧', 'メモ(下書き)', '仕事/企画/メモ(下書き).md', '[メモ(下書き)](メモ%28下書き%29.md)'],
    ['#', 'C# の調べもの', '仕事/企画/C# の調べもの.md', '[C# の調べもの](C%23%20の調べもの.md)'],
    ['日本語', '新しいプロダクトの考え方', '仕事/会議メモ/新しいプロダクトの考え方.md', '[新しいプロダクトの考え方](../会議メモ/新しいプロダクトの考え方.md)'],
  ])('%s', async (_, title, target, expected) => {
    expect(await serializeLink('仕事/企画/今のページ.md', target, title)).toBe(expected)
  })
})

describe('保管庫の中のパスの解決(assetUrl で使う)', () => {
  it('ページのフォルダからの相対パス', () => {
    expect(resolveInVault('仕事/企画', './images/a.png')).toBe('仕事/企画/images/a.png')
    expect(resolveInVault('仕事/企画', '../共有/a.png')).toBe('仕事/共有/a.png')
  })
  it('保管庫の外や絶対パスは null', () => {
    expect(resolveInVault('仕事', '../../a.png')).toBeNull()
    expect(resolveInVault('', '/etc/passwd')).toBeNull()
  })
  it('relativePath と resolveInVault は往復する', () => {
    const from = '仕事/企画'
    for (const to of ['仕事/企画/a.md', '仕事/b.md', 'c.md', '学び/技術/d.md']) {
      expect(resolveInVault(from, relativePath(from, to))).toBe(to)
    }
  })
})
