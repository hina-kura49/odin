import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/** クイックキャプチャのウィンドウか(Tauri の設定で index.html?window=capture を開く。ブラウザでも同じ URL で確かめられる) */
export const isCaptureWindow = () => typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('window') === 'capture'

/** いまのウィンドウを隠す(閉じずに残し、次に出すときに待たせない)。ブラウザでは何もしない */
export function hideCurrentWindow(): void {
  if (isTauri) void getCurrentWindow().hide()
}

// Rust 側(src-tauri/src/lib.rs)との取り決め
const HIDE_REQUESTED_EVENT = 'app:hide-requested'
const QUIT_REQUESTED_EVENT = 'app:quit-requested'
const OPEN_VAULT_REQUESTED_EVENT = 'app:open-vault-requested'

type Lifecycle = {
  /** 未保存の変更を保存し終える */
  save: () => Promise<void>
  /** 保存したあとも、まだ保存できていない変更があるか(あれば終了をやめる) */
  hasUnsaved: () => boolean
}

/**
 * メインのウィンドウの「隠す」「終了する」の前に保存する。返り値は解除関数。
 * - 閉じるボタン・⌘W: 保存してから隠す(閉じない。Dock のアイコンで出し直せる)
 * - ⌘Q: 保存してから終了する。保存できなかったら終了をやめる(知らせは保存の処理が出す)
 * - アプリが後ろに回ったとき(ほかのアプリに移った、Dock のメニューを開いたなど)も保存する。
 *   Dock の「終了」やログアウトでは、終了の前に保存を待てないため
 * ブラウザ(vite dev)では、タブを隠す・閉じるときに保存を始めるだけ。
 */
export function watchAppLifecycle({ save, hasUnsaved }: Lifecycle): () => void {
  const saveQuietly = () => save().catch(() => {})
  const onHidden = () => {
    if (document.visibilityState === 'hidden') void saveQuietly()
  }
  const onBlur = () => void saveQuietly()
  document.addEventListener('visibilitychange', onHidden)
  window.addEventListener('blur', onBlur)
  window.addEventListener('pagehide', onBlur)
  const disposers = [
    () => document.removeEventListener('visibilitychange', onHidden),
    () => window.removeEventListener('blur', onBlur),
    () => window.removeEventListener('pagehide', onBlur),
  ]
  if (isTauri) {
    disposers.push(
      subscribe(HIDE_REQUESTED_EVENT, async () => {
        await saveQuietly()
        await getCurrentWindow().hide()
      }),
      subscribe(QUIT_REQUESTED_EVENT, async () => {
        await saveQuietly()
        await invoke('finish_quit', { exit: !hasUnsaved() })
      }),
    )
  }
  return () => disposers.forEach((d) => d())
}

/** メニューの「保管庫を開く…」を選んだとき。ブラウザでは何もしない。返り値は解除関数 */
export function onOpenVaultRequested(handler: () => Promise<void>): () => void {
  return isTauri ? subscribe(OPEN_VAULT_REQUESTED_EVENT, handler) : () => {}
}

/** クイックキャプチャのホットキーを登録できなかったか。登録できていれば(ブラウザでも) false */
export async function captureShortcutFailed(): Promise<boolean> {
  if (!isTauri) return false
  return (await invoke<string | null>('capture_shortcut_error')) !== null
}

/** Tauri のイベントを受ける。listen は Promise で解除関数を返すので、登録前に解除されても漏れないようにする */
function subscribe(event: string, handler: () => Promise<void>): () => void {
  let unlisten: (() => void) | null = null
  let disposed = false
  void listen(event, () => void handler()).then((fn) => {
    if (disposed) fn()
    else unlisten = fn
  })
  return () => {
    disposed = true
    unlisten?.()
  }
}
