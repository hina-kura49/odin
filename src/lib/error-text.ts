import { toBackendError, type BackendError, type BackendErrorKind } from '@/backend'

// 通知に出すエラーの文。バックエンド(core)の文はそのまま出さず、エラーの種類ごとにここで日本語の文を決める。
// - 通知の見出しは場面ごとに、呼ぶ側で決める(「ページを作れませんでした」など)。ここで決めるのは本文
// - 本文は種類ごとの文を使い、場面によって言い方を変えたいものだけ SCENE_TEXT で上書きする
// - バックエンドの文は、開発用の記録(console)にだけ残す

/** エラーが起きた場面 */
export type ErrorScene =
  | 'loadVault' // 起動時に前回の保管庫を開く・ツリーを読む
  | 'openVault' // 保管庫を選び直す
  | 'refreshTree' // ページの一覧を読み直す
  | 'openPage' // ページを開く
  | 'createPage' // ページを作る
  | 'renamePage' // 名前を変える
  | 'deletePage' // ゴミ箱に移動する
  | 'savePage' // 保存する
  | 'recreatePage' // 見つからなくなったページを作り直す
  | 'capture' // クイックキャプチャで Inbox に書き足す

const KIND_TEXT: Record<BackendErrorKind, string> = {
  notFound: 'ファイルが見つかりません。他のアプリで移動または削除された可能性があります。',
  invalidPath: 'ファイルの場所が正しくありません。',
  notAPage: 'Markdown のページではないため、扱えません。',
  notUtf8: '文字コードが UTF-8 ではないため、扱えません。',
  readOnly: '読み取り専用のため、変更できません。',
  nameOccupied: '同じ名前のファイルやフォルダがあります。',
  indexOverlapsVault: 'このフォルダは保管庫にできません。別のフォルダを選んでください。',
  io: 'ファイルの読み書きに失敗しました。もう一度お試しください。',
}

const SCENE_TEXT: Partial<Record<ErrorScene, Partial<Record<BackendErrorKind, string>>>> = {
  loadVault: {
    notFound: '前回の保管庫のフォルダが見つかりません。移動または削除された可能性があります。',
  },
  openVault: {
    notFound: '選んだフォルダが見つかりません。',
  },
  createPage: {
    notFound: '作る場所のページやフォルダが見つかりません。',
    nameOccupied: '同じ名前のファイルがあるため、作れません。',
    readOnly: '作る場所のフォルダが読み取り専用のため、作れません。',
  },
  renamePage: {
    notFound: 'ページが見つかりません。他のアプリで移動または削除された可能性があります。',
    readOnly: '読み取り専用のため、名前を変えられません。',
  },
  deletePage: {
    notFound: 'ページが見つかりません。すでに移動または削除された可能性があります。',
  },
  savePage: {
    notAPage: 'Markdown のページではないため、保存できません。',
    nameOccupied: '同じ名前のファイルやフォルダがあるため、保存できません。',
  },
  capture: {
    notFound: '保管庫のフォルダが見つかりません。',
    readOnly: 'Inbox が読み取り専用のため、書き足せません。',
    notUtf8: 'Inbox の文字コードが UTF-8 ではないため、書き足せません。',
  },
}

/** 通知の本文にするエラーの文。バックエンドの文は開発用の記録にだけ残す */
export function errorText(scene: ErrorScene, e: unknown): string {
  const err = toBackendError(e)
  logError(scene, err)
  return textFor(scene, err.kind)
}

/** 種類と場面に対する文(記録は残さない。一覧の確認やテスト用) */
export const textFor = (scene: ErrorScene, kind: BackendErrorKind): string => SCENE_TEXT[scene]?.[kind] ?? KIND_TEXT[kind]

/** 開発用の記録。通知の文を決める側で、別に文を決めたとき(同じ名前のページがあります、など)も残す */
export function logError(scene: ErrorScene, err: BackendError): void {
  console.warn(`[backend] ${scene}: ${err.rawKind}: ${err.message}`)
}
