// @vitest-environment jsdom
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { useApp } from '@/store/app'
import { NoticeBar } from '../NoticeBar'

afterEach(() => {
  cleanup()
  useApp.setState({ notice: null })
})

const confirm = (onYes: () => void) =>
  useApp.getState().showNotice({
    tone: 'warning',
    title: 'このページと子ページ 2 件をゴミ箱に移動します',
    actions: [
      { label: 'ゴミ箱に移動', run: onYes },
      { label: 'キャンセル', run: () => {} },
    ],
    focus: true,
  })

describe('確認の通知(キーボード)', () => {
  it('出たら最初のボタンに入力位置が移り、Esc で取り消すと元の場所へ戻る', () => {
    const before = document.createElement('button')
    document.body.append(before)
    before.focus()
    const { getByText } = render(<NoticeBar />)
    let yes = 0
    act(() => confirm(() => yes++))
    expect(document.activeElement).toBe(getByText('ゴミ箱に移動'))
    fireEvent.keyDown(document.activeElement!, { key: 'Escape' })
    expect(useApp.getState().notice).toBeNull()
    expect(yes).toBe(0)
    expect(document.activeElement).toBe(before)
    before.remove()
  })

  it('Enter(ボタンの決定)で実行する', () => {
    const { getByText } = render(<NoticeBar />)
    let yes = 0
    act(() => confirm(() => yes++))
    act(() => getByText('ゴミ箱に移動').click())
    expect(yes).toBe(1)
  })
})
