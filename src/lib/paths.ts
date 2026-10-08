// 保管庫の中のパス(「/」区切り、ルートからの相対)の計算。path は識別子なので、ここ以外では加工しない。

/** ページのあるフォルダ("" はルート) */
export const dirOf = (path: string): string => path.split('/').slice(0, -1).join('/')

/** URL のスキーム(https: や data: など)で始まるか。Windows のドライブ名のような1文字は除く */
export const hasScheme = (src: string): boolean => /^[a-z][a-z0-9+.-]+:/i.test(src)

/**
 * フォルダ dir から見た相対パス rel を、保管庫のルートからのパスにする。
 * 保管庫の外に出る(.. がルートを越える)、または絶対パスなら null。
 */
export function resolveInVault(dir: string, rel: string): string | null {
  if (rel.startsWith('/')) return null
  const parts = dir ? dir.split('/') : []
  for (const seg of rel.split('/')) {
    if (seg === '' || seg === '.') continue
    if (seg === '..') {
      if (parts.length === 0) return null
      parts.pop()
    } else parts.push(seg)
  }
  return parts.join('/')
}

/** フォルダ fromDir から、保管庫のパス to への相対パス */
export function relativePath(fromDir: string, to: string): string {
  const from = fromDir ? fromDir.split('/') : []
  const target = to.split('/')
  let common = 0
  while (common < from.length && common < target.length - 1 && from[common] === target[common]) common++
  return [...Array<string>(from.length - common).fill('..'), ...target.slice(common)].join('/')
}

const UNRESERVED_ASCII = /[A-Za-z0-9\-_./]/
const UNICODE_SPACE_OR_CONTROL = /[\s\p{Cc}]/u

const percentEncode = (ch: string) =>
  Array.from(new TextEncoder().encode(ch), (b) => `%${b.toString(16).toUpperCase().padStart(2, '0')}`).join('')

/**
 * リンクの行き先のエンコード(バックエンドと同じ規則。フォルダ名の部分にも同じ規則を当てる)。
 * - ASCII の文字は、英字・数字・- _ . と区切りの / だけをそのまま書き、ほかはすべてパーセントエンコードする(16進数は大文字)
 * - ASCII 以外の文字は、Unicode の空白と制御文字だけをエンコードし、ほかはそのまま書く
 * - <...> で囲まない
 */
export function encodeLinkDestination(path: string): string {
  let out = ''
  for (const ch of path) {
    const ascii = ch.codePointAt(0)! < 0x80
    const keep = ascii ? UNRESERVED_ASCII.test(ch) : !UNICODE_SPACE_OR_CONTROL.test(ch)
    out += keep ? ch : percentEncode(ch)
  }
  return out
}
