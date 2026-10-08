import { Command } from 'cmdk'
import { CornerDownLeft, FileText, Search, SquarePen } from 'lucide-react'
import { useEffect, useRef } from 'react'
import type { SearchHit } from '@/backend'
import { cn } from '@/lib/cn'
import { isComposing } from '@/lib/keyboard'
import { usePresence } from '@/lib/presence'
import { useApp } from '@/store/app'
import type { PageRow, PaletteCommand } from './palette-search'
import { closePalette, search, setPaletteInput, usePalette } from './palette-store'

/** 最近開いたページのうち、⌘1〜⌘5 で開ける件数 */
const RECENT_SHORTCUTS = 5

/**
 * コマンドパレット(デザイン 4)。ページ・本文の検索・コマンドをひとつの入力欄から。
 * - 入力が空なら最近開いたページ(⌘1〜⌘5 で開ける)
 * - 日本語の変換中は検索せず、確定したときに検索する。変換確定の Enter では開かない
 * - 結果は最後に確定した検索語のものだけ。届くまでは直前の結果を残す(一覧を空にしない、スピナーを出さない)
 */
export function CommandPalette() {
  const open = usePalette((s) => s.open)
  const query = usePalette((s) => s.query)
  const results = usePalette((s) => s.results)
  const selected = usePalette((s) => s.selected)
  const { mounted, ref } = usePresence<HTMLDivElement>(open, 'palette')
  const composing = useRef(false)

  // パレットの外を押したら閉じる
  useEffect(() => {
    if (!open) return
    const onDown = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) closePalette()
    }
    window.addEventListener('pointerdown', onDown, true)
    return () => window.removeEventListener('pointerdown', onDown, true)
  }, [open, ref])

  if (!mounted) return null

  const openPage = (path: string) => {
    closePalette()
    void useApp.getState().openPage(path)
  }
  const runCommand = (c: PaletteCommand) => {
    closePalette()
    const app = useApp.getState()
    if (c.id === 'new-page') void app.newPage('sibling')
    else if (c.id === 'new-child') void app.newPage('child')
    else if (c.id === 'toggle-sidebar') app.toggleSidebar()
  }

  const empty = results.kind === 'search' && results.pages.length + results.hits.length + results.commands.length === 0

  return (
    <div className={cn('fixed inset-0 z-50 flex justify-center px-4', !open && 'pointer-events-none')}>
      <div ref={ref} className="palette editor-popover mt-[12vh] h-fit w-full max-w-[560px] overflow-hidden" style={{ transformOrigin: 'top center' }}>
        <Command
          label="コマンドパレット"
          shouldFilter={false}
          loop
          value={selected}
          onValueChange={(v) => usePalette.setState({ selected: v })}
          onKeyDown={(e) => {
            if (isComposing(e.nativeEvent)) return
            if (e.key === 'Escape') {
              e.preventDefault()
              closePalette()
              return
            }
            // ⌘1〜⌘5: 最近開いたページを開く
            const n = Number(e.key)
            if (e.metaKey && !e.shiftKey && !e.altKey && n >= 1 && n <= RECENT_SHORTCUTS && results.kind === 'recent') {
              e.preventDefault()
              const page = results.recent[n - 1]
              if (page) openPage(page.path)
            }
          }}
        >
          <div className="flex items-center gap-2 border-b border-border px-3">
            <Search size={16} className="shrink-0 text-muted" />
            <Command.Input
              ref={setPaletteInput}
              value={query}
              onValueChange={(v) => {
                usePalette.setState({ query: v })
                if (!composing.current) search.run(v)
              }}
              onCompositionStart={() => {
                composing.current = true
              }}
              onCompositionEnd={(e) => {
                composing.current = false
                search.run(e.currentTarget.value)
              }}
              placeholder="ページ・本文・コマンドを検索"
              className="h-11 flex-1 bg-transparent text-base outline-none placeholder:text-muted"
            />
          </div>

          <Command.List className="palette-list max-h-[min(420px,60vh)] overflow-y-auto p-1">
            {results.kind === 'recent' ? (
              <Command.Group heading="最近開いたページ">
                {results.recent.map((p, i) => (
                  <PageItem key={p.path} value={`recent:${p.path}`} page={p} keys={i < RECENT_SHORTCUTS ? `⌘${i + 1}` : undefined} onSelect={openPage} selected={selected} />
                ))}
              </Command.Group>
            ) : empty ? (
              <NoResults query={results.query} />
            ) : (
              <>
                {results.pages.length > 0 && (
                  <Command.Group heading="ページ">
                    {results.pages.map((p) => (
                      <PageItem key={p.path} value={`page:${p.path}`} page={p} withWhere onSelect={openPage} selected={selected} />
                    ))}
                  </Command.Group>
                )}
                {results.hits.length > 0 && (
                  <Command.Group heading="本文の検索結果">
                    {results.hits.map((h) => (
                      <HitItem key={h.path} hit={h} onSelect={openPage} selected={selected} />
                    ))}
                  </Command.Group>
                )}
                {results.commands.length > 0 && (
                  <Command.Group heading="コマンド">
                    {results.commands.map((c) => (
                      <Command.Item key={c.id} value={`cmd:${c.id}`} onSelect={() => runCommand(c)} className="palette-item">
                        <SquarePen size={16} className="shrink-0 text-muted" />
                        <span className="flex-1 truncate">{c.label}</span>
                        {c.keys && <kbd className="palette-keys">{c.keys}</kbd>}
                      </Command.Item>
                    ))}
                  </Command.Group>
                )}
              </>
            )}
          </Command.List>
        </Command>
      </div>
    </div>
  )
}

const Enter = () => <CornerDownLeft size={14} className="shrink-0 text-muted" aria-hidden />

function PageItem({
  value,
  page,
  keys,
  withWhere,
  selected,
  onSelect,
}: {
  value: string
  page: PageRow
  keys?: string
  withWhere?: boolean
  selected: string
  onSelect: (path: string) => void
}) {
  return (
    <Command.Item value={value} onSelect={() => onSelect(page.path)} className="palette-item">
      <FileText size={16} className="shrink-0 text-muted" />
      <span className="min-w-0 flex-1">
        <span className="block truncate">{page.title}</span>
        {withWhere && page.where && <span className="block truncate text-sm text-muted">{page.where}</span>}
      </span>
      {keys ? <kbd className="palette-keys">{keys}</kbd> : selected === value && <Enter />}
    </Command.Item>
  )
}

/** 本文の検索結果。前後の文字はそのままつなぎ、当たった部分だけを強調する */
function HitItem({ hit, selected, onSelect }: { hit: SearchHit; selected: string; onSelect: (path: string) => void }) {
  const value = `hit:${hit.path}`
  return (
    <Command.Item value={value} onSelect={() => onSelect(hit.path)} className="palette-item">
      <FileText size={16} className="shrink-0 text-muted" />
      <span className="min-w-0 flex-1">
        <span className="block truncate">{hit.title}</span>
        <span className="block truncate text-sm text-muted">
          {hit.snippet.before}
          <mark className="palette-hit">{hit.snippet.hit}</mark>
          {hit.snippet.after}
        </span>
      </span>
      {selected === value && <Enter />}
    </Command.Item>
  )
}

function NoResults({ query }: { query: string }) {
  return (
    <div className="flex flex-col items-center px-6 py-8 text-center">
      <Search size={32} strokeWidth={1.5} className="mb-3 text-muted" />
      <p className="text-base">「{query.trim()}」に関するページ・検索結果・コマンドはありません</p>
      <ul className="mt-3 list-disc pl-5 text-left text-sm leading-relaxed text-muted">
        <li>別のキーワードで試す</li>
        <li>表記を変更してみる</li>
        <li>新しいページを作成する(⌘N)</li>
      </ul>
    </div>
  )
}
