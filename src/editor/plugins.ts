import { parserCtx } from '@milkdown/kit/core'
import { imageSchema, listItemSchema } from '@milkdown/kit/preset/commonmark'
import type { Node as PMNode } from '@milkdown/kit/prose/model'
import { Slice } from '@milkdown/kit/prose/model'
import { Plugin, PluginKey } from '@milkdown/kit/prose/state'
import { Decoration, DecorationSet, type EditorView } from '@milkdown/kit/prose/view'
import { $prose, $view } from '@milkdown/kit/utils'
import type { ElementContent, RootContent } from 'hast'
import { common, createLowlight } from 'lowlight'
import { backend } from '@/backend'

// ---- コードの色付け ----
// 文書と同時に(同期的に)計算するので、ページを開いた最初の描画から色が付いている。
// 付けるのは文字色だけ。太さや字形を変えないので、色が付いても行の高さや幅は変わらない。

const lowlight = createLowlight(common)

type Span = { from: number; to: number; className: string }

function collectSpans(nodes: (RootContent | ElementContent)[], offset: number, classes: string[], out: Span[]): number {
  let pos = offset
  for (const n of nodes) {
    if (n.type === 'text') {
      if (classes.length > 0) out.push({ from: pos, to: pos + n.value.length, className: classes.join(' ') })
      pos += n.value.length
    } else if (n.type === 'element') {
      const own = n.properties.className
      const next = Array.isArray(own) ? [...classes, ...own.map(String)] : classes
      pos = collectSpans(n.children, pos, next, out)
    }
  }
  return pos
}

/** コードブロックごとの色付け結果。ProseMirror のノードは変わらなければ同じオブジェクトなので、それをキーに使い回す */
const spanCache = new WeakMap<PMNode, Span[]>()

function spansOf(node: PMNode): Span[] {
  const cached = spanCache.get(node)
  if (cached) return cached
  const language = String(node.attrs.language ?? '')
  const spans: Span[] = []
  if (language && lowlight.registered(language)) {
    collectSpans(lowlight.highlight(language, node.textContent).children, 0, [], spans)
  }
  spanCache.set(node, spans)
  return spans
}

function highlightDecorations(doc: PMNode): DecorationSet {
  const decorations: Decoration[] = []
  doc.descendants((node, pos) => {
    if (node.type.name !== 'code_block') return true
    for (const s of spansOf(node)) decorations.push(Decoration.inline(pos + 1 + s.from, pos + 1 + s.to, { class: s.className }))
    return false
  })
  return DecorationSet.create(doc, decorations)
}

const highlightKey = new PluginKey<DecorationSet>('odin-code-highlight')

export const codeHighlight = $prose(
  () =>
    new Plugin<DecorationSet>({
      key: highlightKey,
      state: {
        init: (_, { doc }) => highlightDecorations(doc),
        apply: (tr, set) => (tr.docChanged ? highlightDecorations(tr.doc) : set),
      },
      props: { decorations: (state) => highlightKey.getState(state) },
    }),
)

// ---- 画像 ----
// 読み込み前から枠を確保し、読み込み後に本文がずれないようにする。
// 一度表示した画像は実際の幅と縦横比を覚えておき、次からはその大きさの枠で表示する。
// 初めての画像は「文章の幅・16:9」の枠で表示し、表示中は枠の大きさを変えない(画像は枠の中に収める)。

const SIZE_STORAGE_KEY = 'odin:image-sizes'
const DEFAULT_RATIO = 16 / 9

type ImageSize = { width: number; ratio: number }

function loadSizes(): Record<string, ImageSize> {
  try {
    const raw = localStorage.getItem(SIZE_STORAGE_KEY)
    const parsed: unknown = raw ? JSON.parse(raw) : {}
    return typeof parsed === 'object' && parsed !== null ? (parsed as Record<string, ImageSize>) : {}
  } catch {
    return {}
  }
}

const sizes = loadSizes()

function rememberSize(src: string, img: HTMLImageElement): void {
  const size = { width: img.naturalWidth, ratio: img.naturalWidth / img.naturalHeight }
  if (!Number.isFinite(size.ratio) || size.ratio <= 0) return
  if (sizes[src]?.width === size.width && sizes[src]?.ratio === size.ratio) return
  sizes[src] = size
  try {
    localStorage.setItem(SIZE_STORAGE_KEY, JSON.stringify(sizes))
  } catch {
    // 保存できなくても表示には影響しない
  }
}

/** 画像の相対パスを解決するための、いま表示しているページ */
let imagePagePath = ''
export const setImagePagePath = (path: string) => {
  imagePagePath = path
}

export const imageView = $view(imageSchema.node, () => (node) => {
  const src = String(node.attrs.src)
  // 表示用の URL はバックエンドが決める。保管庫の外を指す画像は表示しない(null)。文書の src は変えない
  const url = backend().assetUrl(imagePagePath, src)
  const alt = String(node.attrs.alt ?? '')
  const dom = document.createElement('span')
  dom.className = 'image-block'
  dom.contentEditable = 'false'

  const known = sizes[src]
  if (known) dom.style.setProperty('--image-width', `${known.width}px`)

  const frame = document.createElement('span')
  frame.className = 'image-frame'
  frame.style.aspectRatio = String(known?.ratio ?? DEFAULT_RATIO)

  const img = document.createElement('img')
  img.alt = alt
  img.decoding = 'async'
  img.addEventListener('load', () => rememberSize(src, img))
  if (url !== null) {
    img.src = url
    frame.append(img)
  } else {
    const note = document.createElement('span')
    note.className = 'image-unavailable'
    note.textContent = '保管庫の外にある画像は表示できません'
    frame.append(note)
  }
  dom.append(frame)

  if (alt) {
    const caption = document.createElement('span')
    caption.className = 'image-caption'
    caption.textContent = alt
    dom.append(caption)
  }

  return {
    dom,
    update: (next) => next.type === node.type && next.attrs.src === src && next.attrs.alt === alt,
    ignoreMutation: () => true,
  }
})

// ---- チェックリスト ----
// チェックボックスは CSS で描き、行頭の左側をクリックしたらチェックを切り替える。
// 項目の要素はチェックを切り替えても作り直さず、属性だけを書き換える(CSS のトランジションが効くように)。

function applyListItemAttrs(li: HTMLElement, node: PMNode): void {
  const set = (name: string, value: unknown) => {
    if (value == null) li.removeAttribute(name)
    else li.setAttribute(name, String(value))
  }
  const task = node.attrs.checked != null
  set('data-item-type', task ? 'task' : null)
  set('data-label', node.attrs.label)
  set('data-list-type', node.attrs.listType)
  set('data-spread', node.attrs.spread)
  set('data-checked', task ? node.attrs.checked : null)
}

export const listItemView = $view(listItemSchema.node, () => (initial) => {
  const dom = document.createElement('li')
  applyListItemAttrs(dom, initial)
  return {
    dom,
    contentDOM: dom,
    update: (node) => {
      if (node.type !== initial.type) return false
      applyListItemAttrs(dom, node)
      return true
    },
    // 属性の書き換えは自分で行ったものなので、ProseMirror に再描画させない
    ignoreMutation: (m) => m.type === 'attributes' && m.target === dom,
  }
})

const CHECKBOX_HIT_WIDTH = 28

function toggleTaskAt(view: EditorView, li: HTMLElement): boolean {
  const pos = view.posAtDOM(li, 0)
  const $pos = view.state.doc.resolve(pos)
  for (let depth = $pos.depth; depth > 0; depth--) {
    const node = $pos.node(depth)
    if (node.type.name === 'list_item' && node.attrs.checked != null) {
      view.dispatch(view.state.tr.setNodeMarkup($pos.before(depth), undefined, { ...node.attrs, checked: !node.attrs.checked }))
      return true
    }
  }
  return false
}

export const taskToggle = $prose(
  () =>
    new Plugin({
      props: {
        handleDOMEvents: {
          mousedown: (view, event) => {
            const target = event.target
            if (!(target instanceof HTMLElement)) return false
            const li = target.closest<HTMLElement>('li[data-item-type="task"]')
            if (!li || event.clientX - li.getBoundingClientRect().left > CHECKBOX_HIT_WIDTH) return false
            event.preventDefault()
            return toggleTaskAt(view, li)
          },
        },
      },
    }),
)

// ---- Markdown の貼り付け ----
// Markdown の文字列を貼り付けたら、書式つきのブロックとして入れる。
// Web ページなどからの書式つき(HTML)の貼り付けは、Milkdown の clipboard プラグインに任せる。

const BLOCK_SYNTAX = /^(#{1,6}\s|[-*+]\s|\d{1,9}[.)]\s|>\s?|```|~~~|\|.*\|\s*$|(?:-{3,}|\*{3,})\s*$)/m
const INLINE_SYNTAX = /\*\*[^*\n]+\*\*|`[^`\n]+`|!?\[[^\]\n]*\]\([^)\n]+\)|~~[^~\n]+~~/

export const looksLikeMarkdown = (text: string) => BLOCK_SYNTAX.test(text) || INLINE_SYNTAX.test(text)

export const markdownPaste = $prose(
  (ctx) =>
    new Plugin({
      props: {
        handlePaste: (view, event) => {
          const data = event.clipboardData
          const text = data?.getData('text/plain') ?? ''
          if (!data || !text || view.state.selection.$from.parent.type.spec.code) return false
          if (!looksLikeMarkdown(text)) return false
          const doc = ctx.get(parserCtx)(text)
          view.dispatch(view.state.tr.replaceSelection(Slice.maxOpen(doc.content)).scrollIntoView())
          return true
        },
      },
    }),
)
