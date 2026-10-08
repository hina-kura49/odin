import { getCurrentWindow } from '@tauri-apps/api/window'

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/**
 * ウィンドウを閉じる時に handler を呼び、終わるまで閉じるのを待つ。
 * Tauri では閉じる要求を待てる。ブラウザ(vite dev)では待てないので、書き出しを始めるだけ。
 * 返り値は解除関数。
 */
export function onWindowClose(handler: () => Promise<void>): () => void {
  if (isTauri) {
    let unlisten: (() => void) | null = null
    let disposed = false
    void getCurrentWindow()
      .onCloseRequested(async () => {
        await handler()
      })
      .then((fn) => {
        if (disposed) fn()
        else unlisten = fn
      })
    return () => {
      disposed = true
      unlisten?.()
    }
  }
  const onHide = () => void handler()
  window.addEventListener('pagehide', onHide)
  return () => window.removeEventListener('pagehide', onHide)
}
