import { beforeEach, describe, expect, it, vi } from 'vitest'

const tauri = vi.hoisted(() => ({
  invoke: vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>(),
  emit: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke, convertFileSrc: (p: string) => `asset://localhost${p}` }))
vi.mock('@tauri-apps/api/event', () => ({ listen: () => Promise.resolve(() => {}), emit: tauri.emit }))

const { TauriBackend } = await import('../tauri')
const { BackendError } = await import('../types')

let backend: InstanceType<typeof TauriBackend>
beforeEach(() => {
  tauri.invoke.mockReset()
  tauri.emit.mockReset()
  backend = new TauriBackend()
})

describe('TauriBackend', () => {
  it('WriteResult は生成された形のまま返す', async () => {
    tauri.invoke.mockResolvedValueOnce({ status: 'ok', version: 'v2' })
    expect(await backend.writePage('a.md', '本文', 'v1')).toEqual({ status: 'ok', version: 'v2' })
    expect(tauri.invoke).toHaveBeenCalledWith('write_page', { path: 'a.md', content: '本文', baseVersion: 'v1' })
    tauri.invoke.mockResolvedValueOnce({ status: 'conflict' })
    expect(await backend.writePage('a.md', '本文', 'v1')).toEqual({ status: 'conflict' })
  })

  it('PageMeta の modifiedAt は数値にそろえる', async () => {
    tauri.invoke.mockResolvedValueOnce([{ path: 'a.md', title: 'a', modifiedAt: 1700000000000 }])
    expect(await backend.recentPages(10)).toEqual([{ path: 'a.md', title: 'a', modifiedAt: 1700000000000 }])
    tauri.invoke.mockResolvedValueOnce({ path: 'b.md', title: 'b', modifiedAt: 5n })
    expect((await backend.createPage(null, 'b')).modifiedAt).toBe(5)
  })

  it('失敗は BackendError にする。indexOverlapsVault もそのままの種類で返す', async () => {
    tauri.invoke.mockRejectedValueOnce({ kind: 'nameOccupied', message: 'name occupied: a' })
    await expect(backend.renamePage('a.md', 'b')).rejects.toMatchObject({ kind: 'nameOccupied' })
    tauri.invoke.mockRejectedValueOnce({ kind: 'indexOverlapsVault', message: 'index dir overlaps vault' })
    const err = await backend.openVault().catch((e: unknown) => e)
    expect(err).toBeInstanceOf(BackendError)
    expect(err).toMatchObject({ kind: 'indexOverlapsVault' })
    tauri.invoke.mockRejectedValueOnce({ kind: 'somethingNew', message: '新しい種類' })
    await expect(backend.listTree()).rejects.toMatchObject({ kind: 'io', rawKind: 'somethingNew' })
  })

  it('captureToInbox は変更の知らせを自分では出さない(Rust 側が出す)', async () => {
    tauri.invoke.mockResolvedValueOnce(null)
    await backend.captureToInbox('メモ')
    expect(tauri.invoke).toHaveBeenCalledWith('capture_to_inbox', { text: 'メモ' })
    expect(tauri.emit).not.toHaveBeenCalled()
  })

  it('assetUrl は保管庫の中だけを表示用の URL にする', async () => {
    tauri.invoke.mockResolvedValueOnce('/Users/a/notes')
    await backend.currentVault()
    expect(backend.assetUrl('日記/今日.md', '画像/空.png')).toBe('asset://localhost/Users/a/notes/日記/画像/空.png')
    expect(backend.assetUrl('日記/今日.md', '../../外.png')).toBeNull()
    expect(backend.assetUrl('日記/今日.md', '/etc/passwd')).toBeNull()
    expect(backend.assetUrl('日記/今日.md', 'file:///etc/passwd')).toBeNull()
    expect(backend.assetUrl('日記/今日.md', 'https://example.com/a.png')).toBe('https://example.com/a.png')
  })
})
