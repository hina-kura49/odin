import { Plugin } from '@milkdown/kit/prose/state'
import { $prose } from '@milkdown/kit/utils'

/**
 * エディタの状態が変わったことを、React の部品(メニューなど)に伝える。
 * useSyncExternalStore(bridge.subscribe, bridge.version) で使う。
 */
export type ViewBridge = {
  subscribe: (cb: () => void) => () => void
  version: () => number
  /** マウスのボタンを押している間(範囲選択の途中)は書式メニューを出さない */
  mouseDown: () => boolean
  plugin: ReturnType<typeof $prose>
}

export function createViewBridge(): ViewBridge {
  const listeners = new Set<() => void>()
  let version = 0
  let mouseDown = false
  let queued = false
  // ProseMirror が入力を処理している途中で React が描き直す(そこで位置を読む)と、入力が取り消されることがある。
  // 伝えるのは、いまの処理が終わった後にまとめて1回
  const emit = () => {
    if (queued) return
    queued = true
    queueMicrotask(() => {
      queued = false
      version++
      listeners.forEach((cb) => cb())
    })
  }
  const onMouseUp = () => {
    if (!mouseDown) return
    mouseDown = false
    emit()
  }
  const plugin = $prose(
    () =>
      new Plugin({
        view: () => {
          window.addEventListener('mouseup', onMouseUp)
          // ページを差し替える(updateState で新しい EditorState にする)と、プラグインの view は作り直され、update は呼ばれない。
          // 作り直したときも伝える(空のページの案内などが、差し替えた文書で描き直されるように)
          emit()
          return {
            update: emit,
            destroy: () => window.removeEventListener('mouseup', onMouseUp),
          }
        },
        props: {
          handleDOMEvents: {
            mousedown: () => {
              mouseDown = true
              emit()
              return false
            },
            focus: () => {
              emit()
              return false
            },
            blur: () => {
              emit()
              return false
            },
            compositionstart: () => {
              emit()
              return false
            },
            compositionend: () => {
              setTimeout(emit, 0)
              return false
            },
          },
        },
      }),
  )
  return {
    subscribe: (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    version: () => version,
    mouseDown: () => mouseDown,
    plugin,
  }
}
