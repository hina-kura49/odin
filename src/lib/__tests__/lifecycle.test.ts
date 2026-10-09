// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Tauri のアプリの中として読み込む(isTauri はモジュールを読むときに決まる)
const tauri = vi.hoisted(() => {
  ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}
  return {
    handlers: new Map<string, () => void>(),
    invoke: vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>(),
    hide: vi.fn(() => Promise.resolve()),
  }
})
vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: () => void) => {
    tauri.handlers.set(event, handler)
    return Promise.resolve(() => tauri.handlers.delete(event))
  },
}))
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ hide: tauri.hide }) }))

const { captureShortcutFailed, watchAppLifecycle } = await import('../platform')

/** Rust 側からイベントが届いたことにして、その処理が終わるまで待つ */
async function fire(event: string) {
  await Promise.resolve()
  tauri.handlers.get(event)?.()
  await vi.waitFor(() => expect(tauri.invoke.mock.calls.length + tauri.hide.mock.calls.length).toBeGreaterThan(0))
}

let stop: () => void = () => {}
beforeEach(() => {
  tauri.invoke.mockReset().mockResolvedValue(undefined)
  tauri.hide.mockClear()
})
afterEach(() => stop())

describe('メインのウィンドウを隠す・アプリを終了する', () => {
  it('閉じる要求(閉じるボタン・⌘W)では、保存し終えてから隠す', async () => {
    const order: string[] = []
    tauri.hide.mockImplementation(() => {
      order.push('hide')
      return Promise.resolve()
    })
    const save = () => new Promise<void>((r) => setTimeout(() => (order.push('save'), r()), 10))
    stop = watchAppLifecycle({ save, hasUnsaved: () => false })
    await fire('app:hide-requested')
    expect(order).toEqual(['save', 'hide'])
    expect(tauri.invoke).not.toHaveBeenCalledWith('finish_quit', expect.anything())
  })

  it('⌘Q では、保存し終えてから終了を頼む', async () => {
    const save = vi.fn(() => Promise.resolve())
    stop = watchAppLifecycle({ save, hasUnsaved: () => false })
    await fire('app:quit-requested')
    expect(save).toHaveBeenCalled()
    expect(tauri.invoke).toHaveBeenCalledWith('finish_quit', { exit: true })
  })

  it('保存できない変更が残っていたら、終了をやめる', async () => {
    stop = watchAppLifecycle({ save: () => Promise.reject(new Error('衝突')), hasUnsaved: () => true })
    await fire('app:quit-requested')
    expect(tauri.invoke).toHaveBeenCalledWith('finish_quit', { exit: false })
  })

  it('アプリが後ろに回ったときも保存する', () => {
    const save = vi.fn(() => Promise.resolve())
    stop = watchAppLifecycle({ save, hasUnsaved: () => false })
    window.dispatchEvent(new Event('blur'))
    expect(save).toHaveBeenCalledTimes(1)
  })
})

describe('ホットキーの登録', () => {
  it('登録できなかった理由が返ってきたら true', async () => {
    tauri.invoke.mockResolvedValue('HotKey already registered')
    expect(await captureShortcutFailed()).toBe(true)
    expect(tauri.invoke).toHaveBeenCalledWith('capture_shortcut_error')
  })

  it('登録できていれば false', async () => {
    tauri.invoke.mockResolvedValue(null)
    expect(await captureShortcutFailed()).toBe(false)
  })
})
