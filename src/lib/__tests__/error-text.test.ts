// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { BackendError, type BackendErrorKind } from '@/backend'
import { errorText, textFor, type ErrorScene } from '../error-text'

const KINDS: BackendErrorKind[] = ['notFound', 'invalidPath', 'notAPage', 'notUtf8', 'readOnly', 'nameOccupied', 'indexOverlapsVault', 'io']
const SCENES: ErrorScene[] = ['loadVault', 'openVault', 'refreshTree', 'openPage', 'createPage', 'renamePage', 'deletePage', 'savePage', 'recreatePage', 'capture']

afterEach(() => vi.restoreAllMocks())

describe('エラーの文', () => {
  it('すべての場面と種類の組み合わせに、日本語の文がある', () => {
    for (const scene of SCENES) for (const kind of KINDS) expect(textFor(scene, kind)).toMatch(/[ぁ-んァ-ヶ一-龠]/)
  })

  it('バックエンドの文は通知に出さず、開発用の記録にだけ残す', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const text = errorText('createPage', new BackendError('io', 'Permission denied (os error 13)'))
    expect(text).not.toContain('Permission denied')
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('Permission denied (os error 13)'))
  })

  it('知らない種類は io の文にし、記録には元の種類を残す', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    expect(errorText('savePage', { kind: 'somethingNew', message: 'x' })).toBe(textFor('savePage', 'io'))
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('somethingNew'))
  })

  it('場面によって言い方を変える', () => {
    expect(textFor('createPage', 'nameOccupied')).toBe('同じ名前のファイルがあるため、作れません。')
    expect(textFor('renamePage', 'invalidPath')).toBe('ファイルの場所が正しくありません。')
  })
})
