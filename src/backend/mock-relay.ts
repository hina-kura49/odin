import { emit, listen } from '@tauri-apps/api/event'
import type { MockBackend } from './mock'
import { BackendError, type Backend, type PageMeta, type SearchHit, type TreeNode, type WriteResult } from './types'

// Tauri のアプリを MockBackend のまま動かすときの中継。
// Mock はウィンドウごとに別のメモリを持つので、クイックキャプチャのウィンドウはメインのウィンドウの Mock に取り込みを頼む。
// core/ をつないだら(TauriBackend にしたら)使わない。

const CAPTURE_REQUEST = 'mock:capture'
const CAPTURE_REPLY = 'mock:capture-reply'
/** メインのウィンドウが返事をしないとき(閉じているなど)に、失敗として扱うまでの時間 */
const REPLY_TIMEOUT_MS = 3000

type CaptureRequest = { id: string; text: string }
type CaptureReply = { id: string; ok: boolean; message?: string }

/** メインのウィンドウで呼ぶ: 取り込みの頼みを受けて、自分の Mock に書き込む(Mock が外部の変更として知らせる) */
export function serveMockCaptures(mock: MockBackend): void {
  void listen<CaptureRequest>(CAPTURE_REQUEST, async ({ payload }) => {
    try {
      await mock.captureToInbox(payload.text)
      await emit(CAPTURE_REPLY, { id: payload.id, ok: true } satisfies CaptureReply)
    } catch (e) {
      await emit(CAPTURE_REPLY, { id: payload.id, ok: false, message: String(e) } satisfies CaptureReply)
    }
  })
}

const unsupported = (name: string) => Promise.reject(new BackendError('io', `クイックキャプチャのウィンドウでは ${name} を使えません`))

/** クイックキャプチャのウィンドウのバックエンド。captureToInbox だけを、メインのウィンドウへ中継する */
export class CaptureRelayBackend implements Backend {
  async captureToInbox(text: string): Promise<void> {
    const id = crypto.randomUUID()
    let resolveReply: (reply: CaptureReply) => void = () => {}
    const reply = new Promise<CaptureReply>((resolve, reject) => {
      resolveReply = resolve
      setTimeout(() => reject(new BackendError('io', 'メインのウィンドウから返事がありません')), REPLY_TIMEOUT_MS)
    })
    // 返事を受ける準備ができてから頼む(返事を取りこぼさない)
    const unlisten = await listen<CaptureReply>(CAPTURE_REPLY, ({ payload }) => {
      if (payload.id === id) resolveReply(payload)
    })
    try {
      await emit(CAPTURE_REQUEST, { id, text } satisfies CaptureRequest)
      const { ok, message } = await reply
      if (!ok) throw new BackendError('io', message)
    } finally {
      unlisten()
    }
  }

  currentVault(): Promise<string | null> {
    return unsupported('currentVault')
  }
  openVault(): Promise<string | null> {
    return unsupported('openVault')
  }
  listTree(): Promise<TreeNode[]> {
    return unsupported('listTree')
  }
  readPage(): Promise<{ content: string; version: string }> {
    return unsupported('readPage')
  }
  writePage(): Promise<WriteResult> {
    return unsupported('writePage')
  }
  createPage(): Promise<PageMeta> {
    return unsupported('createPage')
  }
  renamePage(): Promise<PageMeta> {
    return unsupported('renamePage')
  }
  deletePage(): Promise<void> {
    return unsupported('deletePage')
  }
  search(): Promise<SearchHit[]> {
    return unsupported('search')
  }
  recentPages(): Promise<PageMeta[]> {
    return unsupported('recentPages')
  }
  assetUrl(): string | null {
    return null
  }
  onExternalChange(): () => void {
    return () => {}
  }
}
