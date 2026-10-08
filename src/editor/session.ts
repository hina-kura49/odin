import { editorViewCtx, parserCtx, type Editor } from '@milkdown/kit/core'
import type { Node as PMNode } from '@milkdown/kit/prose/model'
import { backend, toBackendError, type BackendError } from '@/backend'
import { dirOf } from '@/lib/paths'
import { useApp } from '@/store/app'
import { loadMarkdown, serializeMarkdown, splitBom, splitFrontmatter, type SourceSnapshot } from './markdown'
import type { ActiveSession } from './registry'

/** 最後の入力から保存までの待ち時間 */
export const SAVE_DELAY_MS = 500

type Loaded = { path: string; version: string; bom: string; body: string }
type Job = { path: string; content: string; version: string; seq: number }

/**
 * 1つのエディタの保存を受け持つ。
 * - 最後の入力から 500ms 後、ページ切り替え時、ウィンドウを閉じる時に writePage を呼ぶ
 * - 打鍵の経路にはバックエンドを挟まない(保存は入力と無関係に裏で行う)
 * - IME の変換中は保存も読み直しもしない。変換が終わってから行う
 * - 保存の結果(衝突・見つからない・読み取り専用)は通知で尋ねる
 */
export class EditorSession implements ActiveSession {
  private loaded: Loaded | null = null
  private snapshot: SourceSnapshot | null = null
  /** ディスクにあると分かっている内容(BOM を含む)。同じなら書かない */
  private onDisk = ''
  private editSeq = 0
  private savedSeq = 0
  private timer: ReturnType<typeof setTimeout> | undefined
  private queue: Promise<void> = Promise.resolve()
  /** 読み取り専用の通知を、同じページで何度も出さない */
  private readOnlyNotified = false
  private afterComposition: (() => void)[] = []

  private readonly editor: Editor
  private readonly delayMs: number

  constructor(editor: Editor, delayMs = SAVE_DELAY_MS) {
    this.editor = editor
    this.delayMs = delayMs
  }

  // ---- 読み込み ----

  /** ページの内容を解析して、エディタに渡す文書を返す。BOM とフロントマターは外して控える */
  load(path: string, content: string, version: string): PMNode {
    clearTimeout(this.timer)
    const { bom, rest } = splitBom(content)
    this.loaded = { path, version, bom, body: rest }
    this.snapshot = null
    this.onDisk = content
    this.editSeq = this.savedSeq = 0
    this.readOnlyNotified = false
    return this.editor.action((ctx) => ctx.get(parserCtx)(splitFrontmatter(rest).body))
  }

  unload(): void {
    clearTimeout(this.timer)
    this.loaded = null
  }

  current() {
    return this.loaded ? { path: this.loaded.path, version: this.loaded.version } : null
  }

  isDirty(): boolean {
    return this.editSeq !== this.savedSeq
  }

  // ---- 入力 ----

  /** 文書が編集された(打鍵・貼り付け・並べ替えなど) */
  markEdited(): void {
    if (!this.loaded) return
    this.editSeq++
    clearTimeout(this.timer)
    this.timer = setTimeout(() => void this.flush(), this.delayMs)
  }

  private composing(): boolean {
    return this.editor.action((ctx) => ctx.get(editorViewCtx).composing)
  }

  /** IME の変換が終わったら呼ぶ。変換中に保留した保存や読み直しを行う */
  compositionEnded(): void {
    const pending = this.afterComposition.splice(0)
    pending.forEach((fn) => fn())
  }

  /** 変換中なら変換が終わってから、そうでなければすぐに fn を行う */
  whenNotComposing(fn: () => void): void {
    if (this.composing()) this.afterComposition.push(fn)
    else fn()
  }

  // ---- 保存 ----

  /** いまの文書を Markdown にする(変わっていないブロックは原文のまま、BOM とフロントマターを戻す) */
  private serialize(): string | null {
    const loaded = this.loaded
    if (!loaded) return null
    return this.editor.action((ctx) => {
      this.snapshot ??= loadMarkdown(ctx, loaded.body).snapshot
      const doc = ctx.get(editorViewCtx).state.doc
      return loaded.bom + serializeMarkdown(ctx, doc, this.snapshot)
    })
  }

  flush(): Promise<void> {
    clearTimeout(this.timer)
    // 変換中は確定前の文字を書き出さない。変換が終わってから保存する
    if (this.loaded && this.isDirty() && this.composing()) {
      this.afterComposition.push(() => void this.flush())
      return this.queue
    }
    const job = this.prepare()
    if (job) this.queue = this.queue.then(() => this.write(job))
    return this.queue
  }

  /** 書き出す内容をこの時点で決める(ページを切り替えた後に、新しいページの文書を書いてしまわないように) */
  private prepare(): Job | null {
    if (!this.loaded || !this.isDirty()) return null
    const content = this.serialize()
    if (content === null) return null
    return { path: this.loaded.path, content, version: this.loaded.version, seq: this.editSeq }
  }

  private async write(job: Job): Promise<void> {
    const isOpen = () => this.loaded?.path === job.path
    // 前の保存で version が進んでいたら、それを基準にする
    const version = isOpen() ? this.loaded!.version : job.version
    if (job.content === this.onDisk && isOpen()) {
      this.savedSeq = Math.max(this.savedSeq, job.seq)
      return
    }
    try {
      const result = await backend().writePage(job.path, job.content, version)
      if (result.ok) {
        if (isOpen()) {
          this.loaded!.version = result.version
          this.onDisk = job.content
          this.savedSeq = Math.max(this.savedSeq, job.seq)
        }
        useApp.getState().markSaved(job.path, job.content, result.version)
        return
      }
      this.onConflict(job)
    } catch (e) {
      this.onError(job, toBackendError(e))
    }
  }

  /** 自分の変更で上書きする */
  async overwriteWith(version: string): Promise<void> {
    if (!this.loaded) return
    this.loaded.version = version
    this.onDisk = ''
    this.editSeq++
    await this.flush()
  }

  // ---- 保存できなかったとき ----

  private onConflict(job: Job): void {
    const app = useApp.getState()
    const reload = async () => {
      const fresh = await backend().readPage(job.path)
      this.whenNotComposing(() => useApp.getState().replaceOpenPage(job.path, fresh))
    }
    const keepMine = async () => {
      const fresh = await backend().readPage(job.path)
      if (this.loaded?.path === job.path) await this.overwriteWith(fresh.version)
      else await this.writeDetached(job.path, job.content, fresh.version)
    }
    app.showNotice({
      tone: 'warning',
      title: 'このファイルは、他のアプリで変更されました。',
      body: this.loaded?.path === job.path ? '最新の内容を読み込みますか？' : `「${this.titleOf(job.path)}」の変更はまだ保存されていません。`,
      actions: [
        { label: '再読み込み', run: reload },
        { label: '自分の変更を保持', run: keepMine },
      ],
    })
  }

  private onError(job: Job, err: BackendError): void {
    const app = useApp.getState()
    const title = this.titleOf(job.path)
    if (err.kind === 'notFound') {
      app.showNotice({
        tone: 'warning',
        title: `「${title}」が見つかりません`,
        body: '他のアプリで移動または削除されました。書いた内容はまだ残っています。',
        actions: [
          { label: '作り直す', run: () => this.recreate(job, title) },
          {
            label: '破棄する',
            run: () => {
              if (this.loaded?.path === job.path) {
                this.unload()
                useApp.getState().closePage()
              }
              void useApp.getState().refreshTree()
            },
          },
        ],
      })
      return
    }
    if (err.kind === 'readOnly') {
      if (this.readOnlyNotified && this.loaded?.path === job.path) return
      this.readOnlyNotified = true
      app.showNotice({
        tone: 'warning',
        title: `「${title}」は読み取り専用のため、保存できません`,
        body: '書いた内容は、このページを開いている間は残っています。',
        actions: [],
      })
      return
    }
    app.showNotice({
      tone: 'warning',
      title: `「${title}」を保存できませんでした`,
      body: err.message,
      actions: [{ label: 'もう一度保存', run: () => this.writeDetached(job.path, job.content, job.version) }],
    })
  }

  /** 見つからなくなったページを、同じ場所・同じタイトルで作り直して内容を書く */
  private async recreate(job: Job, title: string): Promise<void> {
    const app = useApp.getState()
    try {
      const meta = await backend().createPage(dirOf(job.path) || null, title)
      const { version } = await backend().readPage(meta.path)
      const result = await backend().writePage(meta.path, job.content, version)
      if (!result.ok) throw new Error('作り直したページに書き込めませんでした')
      await app.refreshTree()
      if (this.loaded?.path === job.path) {
        this.unload()
        useApp.getState().replaceOpenPage(meta.path, { content: job.content, version: result.version })
        await useApp.getState().openPage(meta.path)
      }
    } catch (e) {
      app.showNotice({ tone: 'warning', title: '作り直せませんでした', body: toBackendError(e).message, actions: [] })
    }
  }

  /** 開いていないページへの書き込み(通知からの操作) */
  private async writeDetached(path: string, content: string, version: string): Promise<void> {
    await this.write({ path, content, version, seq: 0 })
  }

  private titleOf(path: string): string {
    return (path.split('/').pop() ?? path).replace(/\.md$/, '')
  }
}
