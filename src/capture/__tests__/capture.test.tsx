// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { INBOX_PATH, setBackend } from '@/backend'
import { MockBackend } from '@/backend/mock'
import { useApp } from '@/store/app'
import { CaptureWindow } from '../CaptureWindow'

let mock: MockBackend
beforeEach(() => {
  mock = new MockBackend()
  setBackend(mock)
})
afterEach(cleanup)

const box = () => screen.getByLabelText('クイックキャプチャ') as HTMLTextAreaElement
const type = (value: string) => fireEvent.change(box(), { target: { value } })

describe('クイックキャプチャのウィンドウ', () => {
  it('出たときから入力欄に入力位置がある', () => {
    render(<CaptureWindow />)
    expect(document.activeElement).toBe(box())
  })

  it('Enter で Inbox に取り込み、入力欄を空にする', async () => {
    const capture = vi.spyOn(mock, 'captureToInbox')
    render(<CaptureWindow />)
    type('思いついたこと')
    fireEvent.keyDown(box(), { key: 'Enter' })
    expect(capture).toHaveBeenCalledWith('思いついたこと')
    expect(box().value).toBe('')
    await waitFor(async () => expect((await mock.readPage(INBOX_PATH)).content).toBe('思いついたこと\n'))
  })

  it('日本語の変換確定の Enter では取り込まない', () => {
    const capture = vi.spyOn(mock, 'captureToInbox')
    render(<CaptureWindow />)
    type('へんかん')
    fireEvent.keyDown(box(), { key: 'Enter', isComposing: true })
    fireEvent.keyDown(box(), { key: 'Enter', keyCode: 229 })
    expect(capture).not.toHaveBeenCalled()
    expect(box().value).toBe('へんかん')
  })

  it('Shift+Enter は改行(取り込まない)', () => {
    const capture = vi.spyOn(mock, 'captureToInbox')
    render(<CaptureWindow />)
    type('一行目')
    const notPrevented = fireEvent.keyDown(box(), { key: 'Enter', shiftKey: true })
    expect(notPrevented).toBe(true)
    expect(capture).not.toHaveBeenCalled()
  })

  it('空のまま Enter なら何もせず閉じる。Esc は取り込まずに閉じる', () => {
    const capture = vi.spyOn(mock, 'captureToInbox')
    render(<CaptureWindow />)
    type('  ')
    fireEvent.keyDown(box(), { key: 'Enter' })
    type('書きかけ')
    fireEvent.keyDown(box(), { key: 'Escape' })
    expect(capture).not.toHaveBeenCalled()
    expect(box().value).toBe('')
  })

  it('取り込めなかったら、文字を戻して知らせる', async () => {
    vi.spyOn(mock, 'captureToInbox').mockRejectedValue(new Error('ディスクがいっぱいです'))
    render(<CaptureWindow />)
    type('なくしたくないメモ')
    fireEvent.keyDown(box(), { key: 'Enter' })
    await waitFor(() => expect(box().value).toBe('なくしたくないメモ'))
    expect(screen.getByText(/取り込めませんでした/)).toBeTruthy()
  })
})

describe('Mock の captureToInbox(本物と同じ追記の規則)', () => {
  it('空行1つで区切って末尾に足し、空の取り込みでは何もしない', async () => {
    await mock.captureToInbox('一つ目')
    await mock.captureToInbox('')
    await mock.captureToInbox('二つ目')
    expect((await mock.readPage(INBOX_PATH)).content).toBe('一つ目\n\n二つ目\n')
  })

  it('取り込むと、外部の変更として知らせる', async () => {
    const changed = vi.fn()
    mock.onExternalChange(changed)
    await mock.captureToInbox('メモ')
    expect(changed).toHaveBeenCalledTimes(1)
  })
})

describe('サイドバーの「クイックキャプチャ」', () => {
  const initialState = useApp.getState()
  beforeEach(async () => {
    useApp.setState(initialState, true)
    await useApp.getState().init()
  })

  it('Inbox があれば開く', async () => {
    await mock.captureToInbox('メモ')
    await useApp.getState().refreshTree()
    await useApp.getState().openInbox()
    expect(useApp.getState().page?.path).toBe(INBOX_PATH)
  })

  it('Inbox がまだなければ、作られ方を知らせる', async () => {
    await useApp.getState().openInbox()
    expect(useApp.getState().notice?.title).toBe('Inbox はまだありません')
    expect(useApp.getState().notice?.body).toContain('⌃⌥Space')
  })
})
