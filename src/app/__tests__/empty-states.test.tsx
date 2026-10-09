// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { BackendError, setBackend } from '@/backend'
import { MockBackend } from '@/backend/mock'
import { useApp } from '@/store/app'
import { App } from '../App'

const initialState = useApp.getState()
beforeEach(() => useApp.setState(initialState, true))
afterEach(cleanup)

describe('初回起動', () => {
  it('保管庫がまだないとき、フォルダの選択を出し、選ぶとノートの画面になる', async () => {
    const mock = new MockBackend({ vault: null })
    setBackend(mock)
    render(<App />)
    expect(await screen.findByText('ノートの保存先を選択')).toBeTruthy()
    const button = screen.getByRole('button', { name: 'フォルダを選択…' })
    expect(document.activeElement).toBe(button)
    fireEvent.click(button)
    await waitFor(() => expect(useApp.getState().status).toBe('ready'))
  })

  it('保管庫を読めなかったとき、通知で知らせて、もう一度試せる', async () => {
    const mock = new MockBackend()
    const listTree = vi.spyOn(mock, 'listTree').mockRejectedValueOnce(new BackendError('io', '読み込めません'))
    setBackend(mock)
    render(<App />)
    expect(await screen.findByText('保管庫を開けませんでした')).toBeTruthy()
    expect(screen.getByText('ノートの保存先を選択')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'もう一度試す' }))
    await waitFor(() => expect(useApp.getState().status).toBe('ready'))
    expect(listTree).toHaveBeenCalledTimes(2)
  })

  it('フォルダを開けなかったとき、通知で知らせる', async () => {
    const mock = new MockBackend({ vault: null })
    vi.spyOn(mock, 'openVault').mockRejectedValueOnce(new BackendError('io', '権限がありません'))
    setBackend(mock)
    render(<App />)
    fireEvent.click(await screen.findByRole('button', { name: 'フォルダを選択…' }))
    expect(await screen.findByText('フォルダを開けませんでした')).toBeTruthy()
    expect(useApp.getState().status).toBe('no-vault')
  })
})

describe('ページが1つもない', () => {
  it('案内を出し、新規ページを作成できる', async () => {
    setBackend(new MockBackend({ empty: true }))
    render(<App />)
    expect(await screen.findByText('まだページがありません')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '新規ページを作成' }))
    await waitFor(() => expect(useApp.getState().page?.path).toBeTruthy())
    expect(screen.queryByText('まだページがありません')).toBeNull()
  })

  it('最後のページを削除したら、案内に戻る', async () => {
    const mock = new MockBackend({ empty: true })
    setBackend(mock)
    render(<App />)
    await screen.findByText('まだページがありません')
    const meta = await mock.createPage(null, 'ひとつだけ')
    await useApp.getState().refreshTree()
    await useApp.getState().openPage(meta.path)
    await waitFor(() => expect(screen.queryByText('まだページがありません')).toBeNull())
    await useApp.getState().deletePage(meta.path)
    expect(await screen.findByText('まだページがありません')).toBeTruthy()
  })
})
