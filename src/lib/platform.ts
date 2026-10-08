import { getCurrentWindow } from '@tauri-apps/api/window'

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/** クイックキャプチャのウィンドウか(Tauri の設定で index.html?window=capture を開く。ブラウザでも同じ URL で確かめられる) */
export const isCaptureWindow = () => typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('window') === 'capture'

/** いまのウィンドウを隠す(閉じずに残し、次に出すときに待たせない)。ブラウザでは何もしない */
export function hideCurrentWindow(): void {
  if (isTauri) void getCurrentWindow().hide()
}

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
