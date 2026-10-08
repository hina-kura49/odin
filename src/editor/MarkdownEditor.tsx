import { Editor, editorViewCtx, parserCtx, rootCtx } from '@milkdown/kit/core'
import { clipboard } from '@milkdown/kit/plugin/clipboard'
import { history } from '@milkdown/kit/plugin/history'
import type { Node as PMNode } from '@milkdown/kit/prose/model'
import { EditorState } from '@milkdown/kit/prose/state'
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { configureMarkdown, splitFrontmatter } from './markdown'
import { codeHighlight, imageView, listItemView, markdownPaste, taskToggle } from './plugins'

type Props = {
  /** 表示するページ。path と loadId が変わったときだけ文書を差し替える(保存結果の反映では差し替えない) */
  path: string
  loadId: number
  content: string
  /** 最初の文書を表示できる状態になったら呼ぶ */
  onReady?: () => void
}

/** 解析済みの文書の控え。同じ内容のページを開き直すときは解析を省く。文書はそのエディタのスキーマに属するので、エディタごとに持つ */
const parsedCache = new WeakMap<Editor, Map<string, PMNode>>()

function parse(editor: Editor, content: string): PMNode {
  let cache = parsedCache.get(editor)
  if (!cache) parsedCache.set(editor, (cache = new Map()))
  const hit = cache.get(content)
  if (hit) return hit
  const doc = editor.action((ctx) => ctx.get(parserCtx)(splitFrontmatter(content).body))
  if (cache.size > 50) cache.clear()
  cache.set(content, doc)
  return doc
}

export function MarkdownEditor({ path, loadId, content, onReady }: Props) {
  const mountRef = useRef<HTMLDivElement>(null)
  const [editor, setEditor] = useState<Editor | null>(null)
  const shownKey = useRef<string | null>(null)

  // Milkdown の作成は非同期。作り終わるまでは何も表示しない
  useEffect(() => {
    const root = mountRef.current
    if (!root) return
    let cancelled = false
    let created: Editor | null = null
    void configureMarkdown(Editor.make().config((ctx) => ctx.set(rootCtx, root)))
      .use(history)
      .use(markdownPaste) // Milkdown の clipboard より先に判定する
      .use(clipboard)
      .use(codeHighlight)
      .use(imageView)
      .use(taskToggle)
      .use(listItemView)
      .create()
      .then((e) => {
        if (cancelled) void e.destroy()
        else {
          created = e
          setEditor(e)
        }
      })
    return () => {
      cancelled = true
      void created?.destroy()
    }
  }, [])

  // 描画の前に、解析済みの文書へ一度に差し替える。生の Markdown や書式が後から当たる様子は見えない
  const key = `${path}\n${loadId}`
  useLayoutEffect(() => {
    const root = mountRef.current
    if (!editor || !root || shownKey.current === key) return
    const doc = parse(editor, content)
    editor.action((ctx) => {
      const view = ctx.get(editorViewCtx)
      view.updateState(EditorState.create({ doc, plugins: view.state.plugins }))
    })
    const first = shownKey.current === null
    shownKey.current = key
    root.style.visibility = 'visible'
    if (first) onReady?.()
  }, [editor, key, content, onReady])

  return <div ref={mountRef} className="odin-editor" style={{ visibility: 'hidden' }} />
}
