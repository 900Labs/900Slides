<script lang="ts">
  import type { DeckSnapshot, ParagraphDto, RunDto, TextBoxSnapshot } from './lib/types'

  /** A group of matches within a single text box. */
  interface MatchGroup {
    slideId: string
    slideNumber: number
    shapeIndex: number
    count: number
    snippet: string
  }

  interface Props {
    /** Current deck snapshot. */
    deck: DeckSnapshot
    /** Focus the replace field on open when true. */
    startInReplace: boolean
    /** Closes the dialog. */
    onClose: () => void
    /** Applies a text-box edit. The parent refreshes the deck prop. */
    onApplyEdit: (detail: {
      slideId: string
      shapeIndex: number
      paragraphs: ParagraphDto[]
    }) => Promise<void>
  }

  let { deck, startInReplace, onClose, onApplyEdit }: Props = $props()

  /** Live search query. */
  let query = $state('')
  /** Replacement text. */
  let replacement = $state('')
  /** Whether matching is case-sensitive. */
  let matchCase = $state(false)
  /** Replace input element, focused when opened via Cmd/Ctrl+H. */
  let replaceInput = $state<HTMLInputElement | null>(null)

  /** Joins a text box's paragraph/run text into a single searchable string. */
  function textBoxText(textBox: TextBoxSnapshot): string {
    return textBox.paragraphs.map((p) => p.runs.map((r) => r.text).join('')).join('\n')
  }

  /** Escapes regex metacharacters in a literal search string. */
  function escapeRegExp(s: string): string {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  }

  /** Builds a RegExp for the current query, or null for an empty query. */
  function queryRegExp(): RegExp | null {
    if (!query) return null
    const flags = matchCase ? 'g' : 'gi'
    return new RegExp(escapeRegExp(query), flags)
  }

  /** All match groups for the current query across the deck. */
  const matches = $derived<MatchGroup[]>(computeMatches(deck, queryRegExp()))

  /** Total number of individual matches across the deck. */
  const totalMatches = $derived(matches.reduce((sum, m) => sum + m.count, 0))

  function computeMatches(currentDeck: DeckSnapshot, reg: RegExp | null): MatchGroup[] {
    if (!reg) return []
    const groups: MatchGroup[] = []
    currentDeck.slides.forEach((slide, index) => {
      slide.shapes.forEach((shape, shapeIndex) => {
        if (shape.kind !== 'text_box') return
        const textBox = shape.value as TextBoxSnapshot
        const text = textBoxText(textBox)
        const re = new RegExp(reg.source, reg.flags)
        const found: string[] = []
        let m: RegExpExecArray | null
        while ((m = re.exec(text)) !== null) {
          found.push(m[0])
          if (m.index === re.lastIndex) re.lastIndex += 1
        }
        if (found.length > 0) {
          const first = text.indexOf(found[0])
          const start = Math.max(0, first - 12)
          const end = Math.min(text.length, first + found[0].length + 12)
          const snippet =
            (start > 0 ? '…' : '') +
            text.slice(start, end).replace(/\n/g, ' ') +
            (end < text.length ? '…' : '')
          groups.push({
            slideId: slide.id,
            slideNumber: index + 1,
            shapeIndex,
            count: found.length,
            snippet,
          })
        }
      })
    })
    return groups
  }

  /** Returns new paragraphs for a text box with every query occurrence replaced. */
  function replaceInTextBox(textBox: TextBoxSnapshot, reg: RegExp): ParagraphDto[] {
    return textBox.paragraphs.map((paragraph) => {
      const joined = paragraph.runs.map((r) => r.text).join('')
      const re = new RegExp(reg.source, reg.flags)
      const next = joined.replace(re, replacement)
      const baseRun: RunDto =
        paragraph.runs[0] ?? {
          text: '',
          bold: false,
          italic: false,
          underline: false,
          strikethrough: false,
          verticalAlign: 'baseline',
          code: false,
        }
      const newRun: RunDto = { ...baseRun, text: next }
      return {
        runs: next === '' ? [] : [newRun],
        listStyle: paragraph.listStyle,
        style: paragraph.style,
      }
    })
  }

  /** Replaces every match within a single text box (one edit command). */
  async function replaceGroup(group: MatchGroup): Promise<void> {
    const reg = queryRegExp()
    if (!reg) return
    const slide = deck.slides[group.slideNumber - 1]
    const shape = slide?.shapes[group.shapeIndex]
    if (!shape || shape.kind !== 'text_box') return
    const textBox = shape.value as TextBoxSnapshot
    const paragraphs = replaceInTextBox(textBox, reg)
    await onApplyEdit({
      slideId: group.slideId,
      shapeIndex: group.shapeIndex,
      paragraphs,
    })
  }

  /** Replaces every match across the whole deck, one edit per text box. */
  async function replaceAll(): Promise<void> {
    const reg = queryRegExp()
    if (!reg) return
    for (const group of [...matches]) {
      const slide = deck.slides[group.slideNumber - 1]
      const shape = slide?.shapes[group.shapeIndex]
      if (!shape || shape.kind !== 'text_box') continue
      const textBox = shape.value as TextBoxSnapshot
      const paragraphs = replaceInTextBox(textBox, reg)
      await onApplyEdit({
        slideId: group.slideId,
        shapeIndex: group.shapeIndex,
        paragraphs,
      })
    }
  }

  function onKey(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
    } else if (event.key === 'Enter') {
      event.preventDefault()
      if (matches.length > 0) void replaceAll()
    }
  }

  $effect(() => {
    if (startInReplace && replaceInput) {
      replaceInput.focus()
    }
  })
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation">
  <button type="button" class="backdrop-close" aria-label="Close find and replace" onclick={onClose}></button>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Find and replace">
    <div class="row">
      <input
        class="text-input"
        type="text"
        placeholder="Find"
        bind:value={query}
        aria-label="Find"
      />
      <label class="check">
        <input type="checkbox" bind:checked={matchCase} />
        Match case
      </label>
      <button type="button" class="close-btn" onclick={onClose} aria-label="Close">✕</button>
    </div>
    <div class="row">
      <input
        bind:this={replaceInput}
        class="text-input"
        type="text"
        placeholder="Replace with"
        bind:value={replacement}
        aria-label="Replace with"
      />
      <button type="button" class="action" disabled={totalMatches === 0} onclick={replaceAll}>
        Replace all
      </button>
    </div>
    <div class="results">
      {#if query && totalMatches === 0}
        <p class="empty">No matches.</p>
      {:else if totalMatches === 0}
        <p class="empty">Type to search across all slides.</p>
      {:else}
        <p class="summary">{totalMatches} match{totalMatches === 1 ? '' : 'es'}</p>
        {#each matches as group (group.slideId + ':' + group.shapeIndex)}
          <div class="match">
            <div class="match-info">
              <span class="match-loc">Slide {group.slideNumber} · text box {group.shapeIndex}</span>
              <span class="match-count">{group.count}</span>
            </div>
            <div class="match-snippet">{group.snippet}</div>
            <button type="button" class="replace-here" onclick={() => replaceGroup(group)}>
              Replace here
            </button>
          </div>
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
  }
  .backdrop-close {
    position: fixed;
    inset: 0;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .dialog {
    position: relative;
    z-index: 1;
    width: min(520px, 92vw);
    background: #fff;
    border-radius: 8px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .text-input {
    flex: 1;
    padding: 0.35rem 0.5rem;
    border: 1px solid #ccc;
    border-radius: 4px;
    font-size: 0.9rem;
  }
  .check {
    font-size: 0.8rem;
    color: #555;
    white-space: nowrap;
  }
  .close-btn {
    background: none;
    border: none;
    font-size: 1rem;
    cursor: pointer;
    color: #555;
  }
  .action {
    padding: 0.35rem 0.75rem;
    border: 1px solid #0070c0;
    background: #0070c0;
    color: #fff;
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
  }
  .action:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .results {
    max-height: 40vh;
    overflow-y: auto;
    border-top: 1px solid #eee;
    padding-top: 0.4rem;
  }
  .empty,
  .summary {
    margin: 0.25rem 0;
    font-size: 0.85rem;
    color: #666;
  }
  .match {
    padding: 0.4rem 0;
    border-bottom: 1px solid #f0f0f0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .match-info {
    display: flex;
    justify-content: space-between;
    font-size: 0.8rem;
    color: #333;
  }
  .match-count {
    background: #0070c0;
    color: #fff;
    border-radius: 999px;
    padding: 0 0.5rem;
    font-size: 0.75rem;
  }
  .match-snippet {
    font-size: 0.85rem;
    color: #444;
    word-break: break-word;
  }
  .replace-here {
    align-self: flex-start;
    padding: 0.2rem 0.5rem;
    font-size: 0.8rem;
    border: 1px solid #ccc;
    background: #f7f7f7;
    border-radius: 4px;
    cursor: pointer;
  }
</style>
