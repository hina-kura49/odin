import type { Ctx } from '@milkdown/kit/ctx'
import { Editor, parserCtx, remarkCtx, remarkStringifyOptionsCtx, schemaCtx, serializerCtx } from '@milkdown/kit/core'
import type { Node as PMNode } from '@milkdown/kit/prose/model'
import { commonmark } from '@milkdown/kit/preset/commonmark'
import { gfm, remarkGFMPlugin } from '@milkdown/kit/preset/gfm'
import { $nodeSchema, $remark } from '@milkdown/kit/utils'
import type { Image, Nodes, Parents, Root } from 'mdast'
import remarkCjkFriendly from 'remark-cjk-friendly'
import { SKIP, visit } from 'unist-util-visit'

/** Milkdown 7.22 の image ノードは title に null を受け付けず、タイトルなし画像が壊れる。読み込み時に '' へ直す。 */
const imageTitleFix = $remark('imageTitleFix', () => () => (tree: Root) => {
  visit(tree, 'image', (node: Image) => {
    node.title ??= ''
  })
})

/** 「**「強調」**です」のように括弧と隣り合う強調を、CommonMark の規則でも強調として読む。 */
const cjkFriendly = $remark('cjkFriendly', () => remarkCjkFriendly)

// ---- エディタが表現できない記法 ----
// ブロック単位(HTML のブロック、脚注の本文、数式など)は、編集できないコード風のブロックとして原文のまま表示する。
// 行内(脚注の参照 [^1]、行内 HTML など)は、Milkdown の html ノード(編集できない行内の部品)として原文のまま表示する。
// どちらも保存時は原文をそのまま書き戻す。

/** ブロック単位で原文のまま扱う mdast の種類 */
const RAW_BLOCK_TYPES = new Set(['html', 'footnoteDefinition', 'math', 'toml', 'yaml'])
/** 行内で原文のまま扱う mdast の種類 */
const RAW_INLINE_TYPES = new Set(['footnoteReference', 'inlineMath'])
/** 子にブロックを持つ mdast の種類(ここに直接入っている html はブロック) */
const FLOW_PARENTS = new Set(['root', 'blockquote', 'listItem', 'footnoteDefinition'])

type RawBlock = { type: 'rawBlock'; value: string }

const rawSyntax = $remark('rawSyntax', () => () => (tree: Root, file) => {
  const source = String(file.value ?? '')
  const original = (node: Nodes) => {
    const start = node.position?.start.offset
    const end = node.position?.end.offset
    if (start !== undefined && end !== undefined && source) return source.slice(start, end)
    return 'value' in node && typeof node.value === 'string' ? node.value : ''
  }
  visit(tree, (node: Nodes, index, parent: Parents | undefined) => {
    if (!parent || index === undefined) return
    const flow = FLOW_PARENTS.has(parent.type)
    // Milkdown が先に段落で包んだブロックの HTML(段落の中に html が1つだけ)もブロックとして扱う
    const wrappedHtml =
      flow && node.type === 'paragraph' && node.children.length === 1 && node.children[0].type === 'html'
    if ((flow && RAW_BLOCK_TYPES.has(node.type)) || wrappedHtml) {
      const raw: RawBlock = { type: 'rawBlock', value: original(node) }
      ;(parent.children as unknown[])[index] = raw
      return SKIP
    }
    if (!flow && RAW_INLINE_TYPES.has(node.type)) {
      ;(parent.children as unknown[])[index] = { type: 'html', value: original(node) }
      return SKIP
    }
  })
})

/** 編集できない、原文のままのブロック */
export const rawBlockSchema = $nodeSchema('raw_block', () => ({
  group: 'block',
  atom: true,
  selectable: true,
  draggable: true,
  marks: '',
  attrs: { value: { default: '', validate: 'string' } },
  parseDOM: [{ tag: 'pre[data-raw-block]', getAttrs: (dom) => ({ value: dom.textContent ?? '' }) }],
  toDOM: (node) => ['pre', { 'data-raw-block': '', class: 'raw-block', contenteditable: 'false' }, String(node.attrs.value)],
  parseMarkdown: {
    match: (node) => node.type === 'rawBlock',
    runner: (state, node, type) => {
      state.addNode(type, { value: String(node.value ?? '') })
    },
  },
  toMarkdown: {
    match: (node) => node.type.name === 'raw_block',
    runner: (state, node) => {
      state.addNode('html', undefined, String(node.attrs.value))
    },
  },
}))

/** エディタと往復テストで共有する Milkdown の構成。 */
export function configureMarkdown(editor: Editor): Editor {
  return editor
    .config((ctx) => {
      ctx.update(remarkStringifyOptionsCtx, (prev) => ({ ...prev, bullet: '-' as const, rule: '-' as const }))
      ctx.set(remarkGFMPlugin.options.key, { tablePipeAlign: false })
    })
    .use(rawSyntax)
    .use(commonmark)
    .use(gfm)
    .use(rawBlockSchema)
    .use(imageTitleFix)
    .use(cjkFriendly)
}

/** 元ファイルの最上位ブロック1つ分。nodes はその原文を単独で読み込んだ結果。 */
type SourceBlock = { source: string; gapBefore: string; nodes: PMNode[] }

/** 読み込み時の原文の控え。保存時に、変わっていないブロックは原文をそのまま書き戻すのに使う。 */
export type SourceSnapshot = {
  /** 先頭のフロントマター(閉じの `---` 行の改行まで)。エディタには出さず、保存時にそのまま戻す。 */
  frontmatter: string
  blocks: SourceBlock[]
  trailing: string
}

export type LoadedMarkdown = { doc: PMNode; snapshot: SourceSnapshot }

const FRONTMATTER = /^---\r?\n[\s\S]*?\r?\n---[ \t]*(?:\r?\n|$)/

/** 先頭の BOM(U+FEFF)を外して控える。保存時に付け直す */
export function splitBom(content: string): { bom: string; rest: string } {
  return content.startsWith('\ufeff') ? { bom: '\ufeff', rest: content.slice(1) } : { bom: '', rest: content }
}

/** フロントマターと本文に分ける。フロントマターはエディタに出さない */
export function splitFrontmatter(markdown: string): { frontmatter: string; body: string } {
  const frontmatter = FRONTMATTER.exec(markdown)?.[0] ?? ''
  return { frontmatter, body: markdown.slice(frontmatter.length) }
}

/** Markdown を読み込み、エディタに渡す文書と、保存用の原文の控えを作る。 */
export function loadMarkdown(ctx: Ctx, markdown: string): LoadedMarkdown {
  const { frontmatter, body } = splitFrontmatter(markdown)
  const remark = ctx.get(remarkCtx)
  const parse = ctx.get(parserCtx)
  const doc = parse(body)
  // 位置は変換前の構文木から取る(Milkdown の変換は参照リンクの定義を取り除いてしまう)
  const tree = remark.parse(body)
  // 参照形式のリンク([文書][tauri])は定義がないと読めないので、ブロックを単独で読むときも定義を添える
  const definitions = tree.children
    .filter((c) => c.type === 'definition')
    .map((c) => body.slice(c.position?.start.offset ?? 0, c.position?.end.offset ?? 0))
    .join('\n')
  const blocks: SourceBlock[] = []
  let cursor = 0
  for (const child of tree.children) {
    const start = child.position?.start.offset
    const end = child.position?.end.offset
    // 位置が取れないときは控えを諦める(すべて書き出し直しになるが、内容は失われない)
    if (start === undefined || end === undefined) return { doc, snapshot: { frontmatter, blocks: [], trailing: '' } }
    const source = body.slice(start, end)
    const nodes = child.type === 'definition' ? [] : children(parse(`${source}\n\n${definitions}`))
    blocks.push({ source, gapBefore: body.slice(cursor, start), nodes })
    cursor = end
  }
  return { doc, snapshot: { frontmatter, blocks, trailing: body.slice(cursor) } }
}

function children(doc: PMNode): PMNode[] {
  const out: PMNode[] = []
  doc.forEach((n) => out.push(n))
  return out
}

/** 原文ブロックが、現在の文書の index 番目から始まる最上位ノード列と一致するか。 */
function matches(block: SourceBlock, doc: PMNode, index: number): boolean {
  if (block.nodes.length === 0 || index + block.nodes.length > doc.childCount) return false
  return block.nodes.every((n, k) => n.eq(doc.child(index + k)))
}

type ListKind = 'bullet' | 'ordered'
const BULLETS = ['-', '*', '+'] as const
const DELIMITERS = ['.', ')'] as const

function listKind(node: PMNode): ListKind | null {
  if (node.type.name === 'bullet_list') return 'bullet'
  if (node.type.name === 'ordered_list') return 'ordered'
  return null
}

/** リストの原文から、先頭項目の記号(箇条書きは `-*+`、番号つきは `.)`)を読む。 */
function listMarker(kind: ListKind, text: string): string | null {
  const m = kind === 'bullet' ? /^\s*([-*+])(?:\s|$)/.exec(text) : /^\s*\d{1,9}([.)])(?:\s|$)/.exec(text)
  return m?.[1] ?? null
}

/** 書き出したリストの最上位の項目記号を、marker に置き換える(入れ子の項目は字下げされているので対象外)。 */
function replaceListMarkers(kind: ListKind, text: string, marker: string): string {
  return kind === 'bullet'
    ? text.replace(/^[-*+](?=\s|$)/gm, marker)
    : text.replace(/^(\d{1,9})[.)](?=\s|$)/gm, `$1${marker}`)
}

/** 出力の1区切り。原文をそのまま使うもの(block あり)と、書き出し直したもの。 */
type Segment = { node: PMNode; nodeCount: number; text: string; block: number | null }

/**
 * 文書を Markdown にする。変わっていないブロックは原文を、変わったブロックだけ Milkdown の書き出しを使う。
 * 何も編集していなければ元のファイルと完全に一致する。
 */
export function serializeMarkdown(ctx: Ctx, doc: PMNode, snapshot: SourceSnapshot): string {
  const serializeDoc = ctx.get(serializerCtx)
  const schema = ctx.get(schemaCtx)
  const serializeNode = (node: PMNode) => serializeDoc(schema.topNodeType.create(null, node)).replace(/\n+$/, '')
  const { blocks } = snapshot

  // 1. 最上位ノードごとに、未使用の原文ブロックと突き合わせる。並べ替えにも対応するため、順番は問わない。
  const used = new Set<number>()
  const segments: Segment[] = []
  let last = -1
  for (let i = 0; i < doc.childCount; ) {
    const order = [...blocks.keys()].filter((b) => b > last).concat([...blocks.keys()].filter((b) => b <= last))
    const found = order.find((b) => !used.has(b) && matches(blocks[b], doc, i))
    if (found === undefined) {
      const node = doc.child(i)
      const text = serializeNode(node)
      // 空の文書(空段落1つだけ)は何も書かない
      if (!(doc.childCount === 1 && node.type.name === 'paragraph' && node.content.size === 0)) {
        segments.push({ node, nodeCount: 1, text, block: null })
      }
      i++
      continue
    }
    used.add(found)
    segments.push({ node: doc.child(i), nodeCount: blocks[found].nodes.length, text: blocks[found].source, block: found })
    last = found
    i += blocks[found].nodes.length
  }

  // 2. 同じ種類のリストが空行1つで隣り合うと、読み直したときに1つのリストにつながる。
  //    片方の記号を前後と違うものに変えて、別のリストのまま保つ。変えるのは書き出し直したほうを優先し、
  //    両方とも原文のときだけ後ろ側の原文を諦める。
  const isList = (seg: Segment | undefined, kind: ListKind) => seg !== undefined && seg.nodeCount === 1 && listKind(seg.node) === kind
  for (let s = 1; s < segments.length; s++) {
    const kind = listKind(segments[s].node)
    if (kind === null || !isList(segments[s], kind) || !isList(segments[s - 1], kind)) continue
    if (listMarker(kind, segments[s - 1].text) !== listMarker(kind, segments[s].text)) continue
    const t = segments[s].block === null || segments[s - 1].block !== null ? s : s - 1
    const neighbors = [segments[t - 1], segments[t + 1]]
      .filter((seg) => isList(seg, kind))
      .map((seg) => (seg ? listMarker(kind, seg.text) : null))
    const choices: readonly string[] = kind === 'bullet' ? BULLETS : DELIMITERS
    const marker = choices.find((c) => !neighbors.includes(c))
    if (marker === undefined) continue
    segments[t] = { ...segments[t], text: replaceListMarkers(kind, serializeNode(segments[t].node), marker), block: null }
  }

  // 3. 区切りの空行を決めてつなぐ。
  //    連続していた原文ブロック同士は元の空行を、間のブロックが消えたときは消えたブロックの前にあった空行を使う。
  //    本文に現れない原文ブロック(リンクの参照定義など)は、直前の原文ブロックの後ろに置いて残す。
  const parts: string[] = [snapshot.frontmatter]
  const emitted = new Set<number>()
  let wrote = false
  let lastBlock: number | null = null // 直前に書いた原文ブロック。書き出し直したものの後は null
  const push = (gap: string, text: string) => {
    parts.push(wrote ? gap : (blocks[0]?.gapBefore ?? ''), text)
    wrote = true
  }
  const pushInvisibleAfter = (b: number) => {
    for (let k = b + 1; k < blocks.length && blocks[k].nodes.length === 0; k++) {
      push(blocks[k].gapBefore, blocks[k].source)
      emitted.add(k)
      lastBlock = k
    }
  }
  pushInvisibleAfter(-1)
  for (const seg of segments) {
    const gap = seg.block !== null && lastBlock !== null && seg.block > lastBlock ? blocks[lastBlock + 1].gapBefore : '\n\n'
    push(gap, seg.text)
    lastBlock = seg.block
    if (seg.block !== null) pushInvisibleAfter(seg.block)
  }
  blocks.forEach((b, k) => {
    if (b.nodes.length === 0 && !emitted.has(k)) {
      push('\n\n', b.source)
      lastBlock = k
    }
  })

  if (!wrote) parts.push(blocks.length === 0 ? snapshot.trailing : '')
  else parts.push(lastBlock !== null ? snapshot.trailing : '\n')
  return parts.join('')
}
