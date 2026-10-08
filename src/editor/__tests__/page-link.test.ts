// @vitest-environment jsdom
import { Editor, parserCtx, schemaCtx, serializerCtx } from '@milkdown/kit/core'
import { Fragment, Slice } from '@milkdown/kit/prose/model'
import { describe, expect, it } from 'vitest'
import { encodeLinkDestination, relativePath, resolveInVault } from '@/lib/paths'
import { configureMarkdown, loadMarkdown, serializeMarkdown } from '../markdown'
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

describe('行き先のエンコード', () => {
  it.each([
    ['英字・数字・- _ . / はそのまま', 'a-Z_0.9/b.md', 'a-Z_0.9/b.md'],
    ['空白', '週次 定例.md', '週次%20定例.md'],
    ['括弧', 'メモ(下書き).md', 'メモ%28下書き%29.md'],
    ['#', 'C# の調べもの.md', 'C%23%20の調べもの.md'],
    ['< > % ?', 'a<b>c%d?.md', 'a%3Cb%3Ec%25d%3F.md'],
    ['|', 'A|B.md', 'A%7CB.md'],
    ['&', '調査&検討.md', '調査%26検討.md'],
    ['`', '`code`.md', '%60code%60.md'],
    [':', '10:00 打ち合わせ.md', '10%3A00%20打ち合わせ.md'],
    ['そのほかの ASCII 記号もすべて', "!\"$'*+,;=@[\\]^{}~.md", '%21%22%24%27%2A%2B%2C%3B%3D%40%5B%5C%5D%5E%7B%7D%7E.md'],
    ['制御文字(タブ・改行・DEL)', 'a\tb\nc\u007f.md', 'a%09b%0Ac%7F.md'],
    ['ASCII 以外の空白(全角空白・NBSP)', '全角　空白\u00a0.md', '全角%E3%80%80空白%C2%A0.md'],
    ['ASCII 以外の制御文字(C1)', 'a\u0085b.md', 'a%C2%85b.md'],
    ['日本語や全角の記号はそのまま', '日本語「タイトル」！.md', '日本語「タイトル」！.md'],
    ['フォルダ名にも同じ規則', '../会議 メモ/10:00/A|B.md', '../会議%20メモ/10%3A00/A%7CB.md'],
  ])('%s', (_, input, expected) => {
    expect(encodeLinkDestination(input)).toBe(expected)
  })
})

describe('Markdown に書き出したリンク: [表示文字列](行き先)、<...> で囲まない', () => {
  it.each([
    ['空白', '週次 定例', '仕事/企画/週次 定例.md', '[週次 定例](週次%20定例.md)'],
    ['括弧', 'メモ(下書き)', '仕事/企画/メモ(下書き).md', '[メモ(下書き)](メモ%28下書き%29.md)'],
    ['#', 'C# の調べもの', '仕事/企画/C# の調べもの.md', '[C# の調べもの](C%23%20の調べもの.md)'],
    ['| & ` :', 'A|B & `c` 10:00', '仕事/企画/A|B & `c` 10:00.md', '[A|B & \\`c\\` 10:00](A%7CB%20%26%20%60c%60%2010%3A00.md)'],
    ['日本語', '新しいプロダクトの考え方', '仕事/会議メモ/新しいプロダクトの考え方.md', '[新しいプロダクトの考え方](../会議メモ/新しいプロダクトの考え方.md)'],
    ['フォルダ名に空白や記号', '議事録', '仕事/会議 メモ (2026)/議事録.md', '[議事録](../会議%20メモ%20%282026%29/議事録.md)'],
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

describe('表のセルの中に挿入した場合', () => {
  const table = '| 項目 | リンク |\n| --- | --- |\n| 参考 | |\n'

  async function insertIntoCell(title: string, path: string) {
    const editor = await configureMarkdown(Editor.make()).create()
    const result = editor.action((ctx) => {
      const { doc, snapshot } = loadMarkdown(ctx, table)
      // 2行目・2列目のセル(空)の段落の中に入れる
      let at = -1
      doc.descendants((node, pos) => {
        if (node.type.name === 'table_cell' && node.textContent === '' && at === -1) at = pos + 2
      })
      const link = pageLinkNode(ctx.get(schemaCtx), '仕事/企画/今のページ.md', { path, title, modifiedAt: 0 })
      const edited = doc.replace(at, at, new Slice(Fragment.from(link), 0, 0))
      const markdown = serializeMarkdown(ctx, edited, snapshot)
      // 書き出した Markdown を読み直して、表の形とリンクを確かめる
      const reread = ctx.get(parserCtx)(markdown)
      const cells: string[] = []
      const links: { text: string; href: string }[] = []
      reread.descendants((node) => {
        if (node.type.name === 'table_cell' || node.type.name === 'table_header') cells.push(node.textContent)
        const mark = node.marks.find((m) => m.type.name === 'link')
        if (node.isText && mark) links.push({ text: node.text ?? '', href: String(mark.attrs.href) })
      })
      return { markdown, cells, links }
    })
    await editor.destroy()
    return result
  }

  it('タイトルに | があっても表の列が増えず、リンクの文字と行き先が元に戻る', async () => {
    const { markdown, cells, links } = await insertIntoCell('A|B & `c` 10:00', '仕事/企画/A|B & `c` 10:00.md')
    expect(markdown).toContain('[A\\|B & \\`c\\` 10:00](A%7CB%20%26%20%60c%60%2010%3A00.md)')
    expect(cells).toEqual(['項目', 'リンク', '参考', 'A|B & `c` 10:00'])
    expect(links).toEqual([{ text: 'A|B & `c` 10:00', href: 'A%7CB%20%26%20%60c%60%2010%3A00.md' }])
  })

  it('日本語と空白のタイトル', async () => {
    const { markdown, cells } = await insertIntoCell('週次 定例', '仕事/会議メモ/週次 定例.md')
    expect(markdown).toContain('| 参考 | [週次 定例](../会議メモ/週次%20定例.md) |')
    expect(cells).toEqual(['項目', 'リンク', '参考', '週次 定例'])
  })
})
