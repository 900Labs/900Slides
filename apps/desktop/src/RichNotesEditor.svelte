<script lang="ts">
  import type { ParagraphDto, ParagraphStyleDto, RunDto } from './lib/types'

  /** Paragraph list style kind. */
  type ListStyle = 'none' | 'ordered' | 'unordered'

  /** A run carrying its paragraph's style, or a paragraph break. */
  type Token =
    | { kind: 'run'; run: RunDto; style: ParagraphStyleDto; listStyle: ListStyle }
    | { kind: 'break' }

  /** Props for the rich-text notes editor. */
  interface Props {
    /** Slide whose notes are edited. */
    slideId: string
    /** Current rich notes (always defined when this editor is shown). */
    richNotes: ParagraphDto[]
    /** Commits the (possibly null) rich notes to the backend. */
    onSetRichNotes: (paragraphs: ParagraphDto[] | null) => void
  }

  let { slideId, richNotes, onSetRichNotes }: Props = $props()

  /** Default plain run formatting. */
  function defaultRun(text = ''): RunDto {
    return {
      text,
      bold: false,
      italic: false,
      underline: false,
      strikethrough: false,
      verticalAlign: 'baseline',
      code: false,
    }
  }

  /** Default paragraph style. */
  const defaultStyle: ParagraphStyleDto = {
    blockquote: false,
    codeBlock: false,
    indentLevel: 0,
  }

  /** Converts rich-notes paragraphs to the editable token model. */
  function paragraphsToTokens(paragraphs: ParagraphDto[]): Token[] {
    const tokens: Token[] = []
    paragraphs.forEach((paragraph, index) => {
      const style = paragraph.style ?? defaultStyle
      const listStyle = (paragraph.listStyle ?? 'none') as ListStyle
      const runs = paragraph.runs.length > 0 ? paragraph.runs : [defaultRun('')]
      for (const run of runs) {
        tokens.push({ kind: 'run', run: { ...run }, style, listStyle })
      }
      if (index < paragraphs.length - 1) {
        tokens.push({ kind: 'break' })
      }
    })
    return tokens
  }

  /** Converts the token model back to rich-notes paragraphs. */
  function tokensToParagraphs(tokens: Token[]): ParagraphDto[] {
    const paragraphs: ParagraphDto[] = []
    let current: Token[] = []
    const flush = (): void => {
      if (current.length === 0) {
        current = [{ kind: 'run', run: defaultRun(''), style: defaultStyle, listStyle: 'none' }]
      }
      const first = current[0]
      paragraphs.push({
        runs: current.map((t) => (t.kind === 'run' ? t.run : defaultRun(''))),
        listStyle: first.kind === 'run' ? first.listStyle : 'none',
        style: first.kind === 'run' ? first.style : defaultStyle,
      })
      current = []
    }
    for (const token of tokens) {
      if (token.kind === 'break') {
        flush()
      } else {
        current.push(token)
      }
    }
    flush()
    return paragraphs
  }

  /** Joins tokens into the editable textarea text. */
  function joinTokens(tokens: Token[]): string {
    let text = ''
    for (const token of tokens) {
      text += token.kind === 'break' ? '\n' : token.run.text
    }
    return text
  }

  /** Splits a token list at a global character offset. */
  function splitAtOffset(tokens: Token[], offset: number): { left: Token[]; right: Token[] } {
    const left: Token[] = []
    const right: Token[] = []
    let acc = 0
    let split = false
    for (const token of tokens) {
      if (split) {
        right.push(token)
        continue
      }
      const len = token.kind === 'break' ? 1 : token.run.text.length
      if (offset >= acc + len) {
        left.push(token)
        acc += len
      } else if (offset <= acc) {
        right.push(token)
        split = true
      } else {
        const local = offset - acc
        const runToken = token as Extract<Token, { kind: 'run' }>
        left.push({
          kind: 'run',
          run: { ...runToken.run, text: runToken.run.text.slice(0, local) },
          style: runToken.style,
          listStyle: runToken.listStyle,
        })
        right.push({
          kind: 'run',
          run: { ...runToken.run, text: runToken.run.text.slice(local) },
          style: runToken.style,
          listStyle: runToken.listStyle,
        })
        split = true
      }
    }
    return { left, right }
  }

  /** Merges adjacent run tokens with identical formatting. */
  function mergeAdjacentRuns(tokens: Token[]): Token[] {
    const out: Token[] = []
    for (const token of tokens) {
      const last = out[out.length - 1]
      if (
        token.kind === 'run' &&
        last &&
        last.kind === 'run' &&
        runsEqual(last.run, token.run) &&
        last.listStyle === token.listStyle
      ) {
        last.run = { ...last.run, text: last.run.text + token.run.text }
      } else {
        out.push(token)
      }
    }
    return out
  }

  /** Whether two runs have identical formatting (text ignored). */
  function runsEqual(a: RunDto, b: RunDto): boolean {
    return (
      a.bold === b.bold &&
      a.italic === b.italic &&
      a.underline === b.underline &&
      a.strikethrough === b.strikethrough &&
      a.verticalAlign === b.verticalAlign &&
      a.code === b.code &&
      a.fontFamily === b.fontFamily
    )
  }

  /** The last run token before a position, used to inherit formatting on insert. */
  function inheritRun(left: Token[]): RunDto {
    for (let i = left.length - 1; i >= 0; i -= 1) {
      const token = left[i]
      if (token.kind === 'run') return { ...token.run }
    }
    return defaultRun()
  }

  /** Editable token model (source of truth for run styling). */
  let tokens = $state<Token[]>([])
  /** Textarea value, derived from tokens but independently editable. */
  let text = $state('')
  /** Previous text, for minimal-diff reconciliation on input. */
  let prevText = ''
  /** The textarea element, for selection access. */
  let textareaEl = $state<HTMLTextAreaElement | null>(null)
  /** Whether unsaved changes are pending a commit. */
  let dirty = $state(false)

  /** Resyncs the editor from the backend whenever the slide or its notes change. */
  $effect.pre(() => {
    void slideId
    const next = paragraphsToTokens(richNotes)
    tokens = next
    text = joinTokens(next)
    prevText = text
    dirty = false
  })

  /** Patches the token model for a typed change via a minimal prefix/suffix diff. */
  function handleInput(): void {
    const a = prevText
    const b = text
    const minLen = Math.min(a.length, b.length)
    let prefix = 0
    while (prefix < minLen && a[prefix] === b[prefix]) prefix += 1
    let suffix = 0
    while (
      suffix < minLen - prefix &&
      a[a.length - 1 - suffix] === b[b.length - 1 - suffix]
    ) {
      suffix += 1
    }
    const delStart = prefix
    const delEnd = a.length - suffix
    const inserted = b.slice(prefix, b.length - suffix)
    if (delStart === delEnd && inserted === '') {
      prevText = b
      return
    }

    const { left, right: midRight } = splitAtOffset(tokens, delStart)
    const { left: mid, right } = splitAtOffset(midRight, delEnd - delStart)
    void mid

    const formatting = inheritRun(left)
    const insertedTokens: Token[] = []
    if (inserted !== '') {
      const parts = inserted.split('\n')
      parts.forEach((part, index) => {
        insertedTokens.push({
          kind: 'run',
          run: { ...formatting, text: part },
          style: { ...defaultStyle },
          listStyle: 'none',
        })
        if (index < parts.length - 1) {
          insertedTokens.push({ kind: 'break' })
        }
      })
    }

    tokens = mergeAdjacentRuns([...left, ...insertedTokens, ...right])
    prevText = b
    dirty = true
  }

  /** Toggles a boolean run flag across the current textarea selection. */
  function toggleFlag(flag: 'bold' | 'italic' | 'underline' | 'strikethrough'): void {
    if (!textareaEl) return
    const selStart = textareaEl.selectionStart ?? 0
    const selEnd = textareaEl.selectionEnd ?? 0
    if (selStart === selEnd) return
    const { left, right: midRight } = splitAtOffset(tokens, selStart)
    const { left: mid, right } = splitAtOffset(midRight, selEnd - selStart)
    const styledMid = mid.map((token) =>
      token.kind === 'run'
        ? { ...token, run: { ...token.run, [flag]: !token.run[flag] } }
        : token,
    )
    tokens = mergeAdjacentRuns([...left, ...styledMid, ...right])
    text = joinTokens(tokens)
    prevText = text
    dirty = true
    queueMicrotask(() => {
      textareaEl?.focus()
      textareaEl?.setSelectionRange(selStart, selEnd)
    })
  }

  /** Commits the current rich notes to the backend. */
  function commit(): void {
    if (!dirty) return
    const paragraphs = tokensToParagraphs(tokens)
    const changed = !richNotesEqual(paragraphs, richNotes)
    if (changed) {
      onSetRichNotes(paragraphs)
    }
    dirty = false
  }

  /** Shallow structural equality check for two rich-notes paragraph lists. */
  function richNotesEqual(a: ParagraphDto[], b: ParagraphDto[]): boolean {
    if (a.length !== b.length) return false
    return a.every((pa, i) => {
      const pb = b[i]
      return (
        pa.listStyle === pb.listStyle &&
        JSON.stringify(pa.style) === JSON.stringify(pb.style) &&
        pa.runs.length === pb.runs.length &&
        pa.runs.every((ra, j) => JSON.stringify(ra) === JSON.stringify(pb.runs[j]))
      )
    })
  }
</script>

<div class="rich-notes">
  <div class="toolbar" role="toolbar" aria-label="Rich notes formatting">
    <button type="button" title="Bold" onclick={() => toggleFlag('bold')}><strong>B</strong></button>
    <button type="button" title="Italic" onclick={() => toggleFlag('italic')}><em>I</em></button>
    <button type="button" title="Underline" onclick={() => toggleFlag('underline')}><u>U</u></button>
    <button type="button" title="Strikethrough" onclick={() => toggleFlag('strikethrough')}>
      <s>S</s>
    </button>
  </div>
  <textarea
    bind:this={textareaEl}
    class="rich-notes-input"
    bind:value={text}
    oninput={handleInput}
    onblur={commit}
    aria-label="Rich-text speaker notes"
  ></textarea>
</div>

<style>
  .rich-notes {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .toolbar {
    display: flex;
    gap: 0.2rem;
  }
  .toolbar button {
    min-width: 1.6rem;
    padding: 0.15rem 0.3rem;
    border: 1px solid #ccc;
    background: #fff;
    border-radius: 3px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .rich-notes-input {
    width: 100%;
    min-height: 120px;
    resize: vertical;
    box-sizing: border-box;
    padding: 0.35rem;
    border: 1px solid #ccc;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.85rem;
    line-height: 1.3;
  }
  .rich-notes-input:focus {
    outline: 2px solid #0070c0;
  }
</style>
