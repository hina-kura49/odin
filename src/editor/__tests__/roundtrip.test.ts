// @vitest-environment jsdom
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { schemaCtx } from '@milkdown/kit/core'
import { describe, expect, it } from 'vitest'
import { children, move, roundTrip, topLevelTypes, withChildren } from './helpers'

const dir = join(import.meta.dirname, 'fixtures')
const files = readdirSync(dir).filter((f) => f.endsWith('.md')).sort()
const read = (file: string) => readFileSync(join(dir, file), 'utf8')

describe('Markdown の往復(読み込み → 無編集で保存)', () => {
  it.each(files)('%s', async (file) => {
    const original = read(file)
    expect((await roundTrip(original)).preserved).toBe(original)
  })
})

// Milkdown の書き出しだけでは元に戻らないファイル。原文を残す仕組みが必要な理由の記録。
const plainSerializerDiffers = new Set([
  '04-code-table.md', // 表の区切り行が `:---` → `:-` になる
  '05-media-links.md', // `***` と `---` の区切り線が片方に揃う
  '06-design-sample.md', // 表の区切り行
  '07-japanese-edge.md', // 改行の `  ` → `\\`、URL が `<...>` に、`1)` → `1.`
  '08-frontmatter.md', // フロントマターが区切り線と見出しとして読まれ、壊れる
  '09-reference-links.md', // 参照形式のリンクが展開され、定義が消える
  '11-unsupported.md', // 空行で途切れた HTML のブロックの後半に、空行が足される
])

describe('Milkdown の書き出しだけを使った場合(参考)', () => {
  it.each(files)('%s', async (file) => {
    const original = read(file)
    const { plain } = await roundTrip(original)
    expect(plain === original).toBe(!plainSerializerDiffers.has(file))
  })
})

describe('フロントマター', () => {
  it('エディタには出さず、保存時はそのまま残す', async () => {
    const original = read('08-frontmatter.md')
    const types = await topLevelTypes(original.replace(/^---\n[\s\S]*?\n---\n/, ''))
    expect(types).toEqual(['heading', 'paragraph'])
    const { preserved } = await roundTrip(original, (doc, { parse }) => withChildren(doc, [...children(doc), ...parse('追記。')]))
    expect(preserved).toBe(`${original}\n追記。\n`)
  })
})

describe('エディタが表現できない記法', () => {
  it('HTML のブロックと脚注の本文は編集できないブロック、脚注の参照と行内 HTML は行内の部品になる', async () => {
    const src = read('11-unsupported.md')
    const found: { type: string; value: string }[] = []
    await roundTrip(src, (doc) => {
      doc.descendants((n) => {
        if (n.type.name === 'raw_block' || n.type.name === 'html') found.push({ type: n.type.name, value: String(n.attrs.value) })
      })
      return doc
    })
    expect(found).toContainEqual({ type: 'raw_block', value: '<details>\n<summary>折りたたみ</summary>' })
    expect(found).toContainEqual({ type: 'raw_block', value: '<div align="center">中央</div>' })
    expect(found).toContainEqual({ type: 'raw_block', value: '[^1]: 脚注の本文。\n    2行目も続く。' })
    expect(found).toContainEqual({ type: 'html', value: '[^1]' })
    expect(found).toContainEqual({ type: 'html', value: '<kbd>' })
  })

  it('ほかの段落を編集しても、原文のまま書き戻される', async () => {
    const src = read('11-unsupported.md')
    const { preserved } = await roundTrip(src, (doc, { parse }) => withChildren(doc, [...children(doc), ...parse('追記。')]))
    expect(preserved).toBe(`${src}\n追記。\n`)
  })

  it('原文のブロックを含む段落を書き出し直しても、記法は元のまま', async () => {
    const { preserved } = await roundTrip('本文[^1]と<kbd>K</kbd>。\n\n[^1]: 脚注。\n', (doc, { parse }) =>
      withChildren(doc, [...parse('先頭に追加。'), ...children(doc)]),
    )
    expect(preserved).toBe('先頭に追加。\n\n本文[^1]と<kbd>K</kbd>。\n\n[^1]: 脚注。\n')
  })
})

describe('一部だけ編集した場合、ほかのブロックの書き方は変わらない', () => {
  it('段落を1つ書き換えても、表や区切り線や太字の原文は残る', async () => {
    const original = read('06-design-sample.md')
    const { preserved } = await roundTrip(original, (doc, { ctx }) => {
      const schema = ctx.get(schemaCtx)
      const para = schema.nodes.paragraph.create(null, schema.text('書き換えた段落。'))
      return doc.copy(doc.content.replaceChild(1, para)) // 0: 見出し, 1: 「シンプルでありながら…」
    })
    expect(preserved).toBe(original.replace(/^シンプルでありながら.*$/m, '書き換えた段落。'))
  })
})

describe('ドラッグでの並べ替え', () => {
  const original = read('06-design-sample.md')
  const table = original.slice(original.indexOf('| 観点'), original.indexOf('\n\n![静か'))
  const tableIndex = (doc: Parameters<typeof children>[0]) => children(doc).findIndex((n) => n.type.name === 'table')

  it('動かしただけの表は、区切り行も含めて原文のまま保たれる', async () => {
    const { preserved } = await roundTrip(original, (doc) => withChildren(doc, move(children(doc), tableIndex(doc), 1)))
    const expected = original
      .replace(`\n\n${table}`, '')
      .replace('# 新しいプロダクトの考え方\n\n', `# 新しいプロダクトの考え方\n\n${table}\n\n`)
    expect(preserved).toBe(expected)
    expect(preserved).toContain('| --- | --- | --- |')
  })

  it('末尾へ動かしても、ファイル末尾の改行は1つのまま', async () => {
    const { preserved } = await roundTrip(original, (doc) => withChildren(doc, move(children(doc), tableIndex(doc), children(doc).length - 1)))
    expect(preserved.endsWith(`${table}\n`)).toBe(true)
    expect(preserved.endsWith('\n\n')).toBe(false)
    expect(preserved.replace(`\n\n${table}`, '')).toBe(original.replace(`\n\n${table}`, ''))
  })

  it('2つのブロックを入れ替えても、どちらも原文のまま', async () => {
    const src = '***\n\n| a | b |\n| :--- | ---: |\n| 1 | 2 |\n'
    const { preserved } = await roundTrip(src, (doc) => withChildren(doc, move(children(doc), 1, 0)))
    expect(preserved).toBe('| a | b |\n| :--- | ---: |\n| 1 | 2 |\n\n***\n')
  })
})

describe('ブロックの削除', () => {
  const original = read('06-design-sample.md')
  const without = (index: number) => (doc: Parameters<typeof children>[0]) => withChildren(doc, children(doc).filter((_, i) => i !== index))

  it('途中のブロックを消しても、前後の空行は1行のまま', async () => {
    const { preserved } = await roundTrip(original, (doc) => without(children(doc).findIndex((n) => n.type.name === 'blockquote'))(doc))
    expect(preserved).toBe(original.replace('\n\n> シンプルさは、複雑さよりも難しい。\n> — スティーブ・ジョブズ', ''))
  })

  it('先頭のブロックを消しても、ファイル先頭に空行ができない', async () => {
    const { preserved } = await roundTrip(original, without(0))
    expect(preserved).toBe(original.replace('# 新しいプロダクトの考え方\n\n', ''))
  })

  it('末尾のブロックを消しても、末尾の改行は1つのまま', async () => {
    const { preserved } = await roundTrip(original, (doc) => withChildren(doc, children(doc).slice(0, -1)))
    expect(preserved).toBe(original.replace('\n\n関連ページ: [プロダクトの方向性](./プロダクトの方向性.md)', ''))
  })

  it('空行が2行ある箇所: 消したブロックの「後ろ」の空行ごと消え、前の空行は残る', async () => {
    const src = '段落A\n\n\n段落B\n\n段落C\n'
    expect((await roundTrip(src, without(1))).preserved).toBe('段落A\n\n\n段落C\n')
    expect((await roundTrip(src, without(2))).preserved).toBe('段落A\n\n\n段落B\n')
  })

  it('参照形式のリンクの定義は、本文を消しても失われない', async () => {
    const src = read('09-reference-links.md')
    const { preserved } = await roundTrip(src, without(1))
    expect(preserved).toContain('[tauri]: https://tauri.app "Tauri"')
    expect(preserved).toContain('[脚注のない参照]: https://example.com')
  })
})

describe('リストが隣り合っても、別のリストのまま保存される', () => {
  const cases = [
    { name: '箇条書き', existing: '- 既存1\n- 既存2', added: '- 追加' },
    { name: '番号つき', existing: '1. 既存1\n2. 既存2', added: '1. 追加' },
    { name: 'チェックリスト', existing: '- [ ] 既存1\n- [x] 既存2', added: '- [ ] 追加' },
    { name: '* を使った既存の箇条書き', existing: '* 既存1\n* 既存2', added: '- 追加' },
  ]

  it.each(cases)('$name: 既存のリストの直後に新しいリストを置く', async ({ existing, added }) => {
    const src = `# 見出し\n\n${existing}\n`
    const { preserved } = await roundTrip(src, (doc, { parse }) => withChildren(doc, [...children(doc), ...parse(added)]))
    expect(preserved.startsWith(`# 見出し\n\n${existing}\n\n`)).toBe(true)
    const types = await topLevelTypes(preserved)
    expect(types).toHaveLength(3)
    expect(types[1]).toBe(types[2])
  })

  it.each(cases)('$name: 既存のリストの直前に新しいリストを置く', async ({ existing, added }) => {
    const src = `# 見出し\n\n${existing}\n`
    const { preserved } = await roundTrip(src, (doc, { parse }) => {
      const [heading, list] = children(doc)
      return withChildren(doc, [heading, ...parse(added), list])
    })
    expect(preserved.endsWith(`\n\n${existing}\n`)).toBe(true)
    expect(await topLevelTypes(preserved)).toHaveLength(3)
  })

  it('間の段落を消して既存のリスト同士が隣り合っても、つながらない', async () => {
    const src = '- 上のリスト\n\n間の段落\n\n- 下のリスト\n'
    const { preserved } = await roundTrip(src, (doc) => withChildren(doc, children(doc).filter((n) => n.type.name !== 'paragraph')))
    expect(preserved.startsWith('- 上のリスト\n\n')).toBe(true)
    expect(await topLevelTypes(preserved)).toEqual(['bullet_list', 'bullet_list'])
  })

  it('新しいリストが同じ種類の既存リスト2つに挟まれても、3つのまま', async () => {
    const src = '- 上\n\n段落\n\n* 下\n'
    const { preserved } = await roundTrip(src, (doc, { parse }) => {
      const [top, , bottom] = children(doc)
      return withChildren(doc, [top, ...parse('- 新'), bottom])
    })
    expect(await topLevelTypes(preserved)).toEqual(['bullet_list', 'bullet_list', 'bullet_list'])
  })
})
