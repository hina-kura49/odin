import { Editor, editorViewCtx, rootCtx } from '@milkdown/kit/core'
import { clipboard } from '@milkdown/kit/plugin/clipboard'
import { history } from '@milkdown/kit/plugin/history'
import { Plugin } from '@milkdown/kit/prose/state'
import { EditorState, Selection } from '@milkdown/kit/prose/state'
import { $prose } from '@milkdown/kit/utils'
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { onWindowClose } from '@/lib/platform'
import { configureMarkdown } from './markdown'
import { codeHighlight, imageView, listItemView, markdownPaste, setImagePagePath, taskToggle } from './plugins'
import { setActiveSession } from './registry'
import { EditorSession } from './session'

type Props = {
  /** 表示するページ。path と loadId が変わったときだけ文書を差し替える(保存結果の反映では差し替えない) */
  path: string
  loadId: number
  content: string
  version: string
  /** 最初の文書を表示できる状態になったら呼ぶ */
  onReady?: () => void
}

/** 入力(文書が変わるトランザクション)と、IME の変換の終わりを保存の仕組みに伝える */
const sessionBridge = (getSession: () => EditorSession | null) =>
  $prose(
    () =>
      new Plugin({
        // updateState での差し替えはここを通らないので、ユーザーの編集だけを数えられる
        appendTransaction: (trs) => {
          if (trs.some((tr) => tr.docChanged)) getSession()?.markEdited()
          return null
        },
        props: {
          handleDOMEvents: {
            compositionend: () => {
              // ProseMirror が確定した文字を反映し終えてから
              setTimeout(() => getSession()?.compositionEnded(), 0)
              return false
            },
          },
        },
      }),
  )

export function MarkdownEditor({ path, loadId, content, version, onReady }: Props) {
  const mountRef = useRef<HTMLDivElement>(null)
  const [editor, setEditor] = useState<Editor | null>(null)
  const sessionRef = useRef<EditorSession | null>(null)
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
      .use(sessionBridge(() => sessionRef.current))
      .create()
      .then((e) => {
        if (cancelled) {
          void e.destroy()
          return
        }
        created = e
        sessionRef.current = new EditorSession(e)
        setActiveSession(sessionRef.current)
        setEditor(e)
      })
    return () => {
      cancelled = true
      const session = sessionRef.current
      sessionRef.current = null
      setActiveSession(null)
      // 未保存の変更を書き出してから片づける
      void (session?.flush() ?? Promise.resolve()).finally(() => void created?.destroy())
    }
  }, [])

  // ウィンドウを閉じる時に保存する
  useEffect(() => onWindowClose(() => sessionRef.current?.flush() ?? Promise.resolve()), [])

  // 描画の前に、解析済みの文書へ一度に差し替える。生の Markdown や書式が後から当たる様子は見えない
  const key = `${path}\n${loadId}`
  useLayoutEffect(() => {
    const root = mountRef.current
    const session = sessionRef.current
    if (!editor || !root || !session || shownKey.current === key) return
    const swap = () => {
      const samePage = shownKey.current?.startsWith(`${path}\n`) ?? false
      setImagePagePath(path)
      const doc = session.load(path, content, version)
      editor.action((ctx) => {
        const view = ctx.get(editorViewCtx)
        // 同じページの読み直しでは、カーソルの位置をできるだけ保つ
        const prev = view.state.selection
        let state = EditorState.create({ doc, plugins: view.state.plugins })
        if (samePage) {
          const pos = Math.min(prev.from, doc.content.size)
          state = state.apply(state.tr.setSelection(Selection.near(doc.resolve(pos))))
        }
        view.updateState(state)
      })
      const first = shownKey.current === null
      shownKey.current = key
      root.style.visibility = 'visible'
      if (first) onReady?.()
    }
    swap()
  }, [editor, key, path, content, version, onReady])

  return <div ref={mountRef} className="odin-editor" style={{ visibility: 'hidden' }} />
}
