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

/**
 * リンクの行き先のエンコード。空白 ( ) < > # % ? と制御文字だけをパーセントエンコードする(16進数は大文字)。
 * 日本語などほかの文字はそのまま。バックエンドの改名処理と一致させる必要がある。
 */
export function encodeLinkDestination(path: string): string {
  // 制御文字はわざと対象にしている
  // oxlint-disable-next-line no-control-regex
  return path.replace(/[\s()<>#%?\u0000-\u001f\u007f]/g, (ch) =>
    Array.from(new TextEncoder().encode(ch), (b) => `%${b.toString(16).toUpperCase().padStart(2, '0')}`).join(''),
  )
}
