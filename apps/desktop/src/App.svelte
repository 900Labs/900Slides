<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'
  import SlideThumbnail from './SlideThumbnail.svelte'
  import SlideCanvas from './SlideCanvas.svelte'
  import Presenter from './Presenter.svelte'
  import RecoveryPrompt from './RecoveryPrompt.svelte'
  import type {
    DeckSnapshot,
    HeadingLevelDto,
    ParagraphDto,
    RecoverySnapshot,
    RunDto,
    SlideSnapshot,
    TextBoxSnapshot,
    WarningDto,
  } from './lib/types'

  /** True if this window is the presenter view. */
  const isPresenter = window.location.hash === '#/presenter'

  let deck = $state<DeckSnapshot | null>(null)
  let activeIndex = $state(0)
  let warnings = $state<WarningDto[]>([])
  let showWarnings = $state(true)
  let recoverySnapshots = $state<RecoverySnapshot[]>([])
  let showRecovery = $state(false)
  /** Hidden file input used to pick an image to insert. */
  let imageInput = $state<HTMLInputElement | null>(null)

  const activeSlide = $derived<SlideSnapshot | null>(deck?.slides[activeIndex] ?? null)
  const notes = $derived(activeSlide?.notes ?? '')

  $effect(() => {
    if (!isPresenter) {
      loadInitial()
    }
  })

  /** On startup, list recovery snapshots or create a new blank deck. */
  async function loadInitial(): Promise<void> {
    try {
      const snapshots = await invoke<RecoverySnapshot[]>('list_recovery_snapshots')
      if (snapshots.length > 0) {
        recoverySnapshots = snapshots
        showRecovery = true
        return
      }
      await newDeck()
    } catch (err) {
      console.error('Failed to load initial state:', err)
    }
  }

  /** Creates a new blank deck from the Rust model. */
  async function newDeck(): Promise<void> {
    deck = await invoke<DeckSnapshot>('new_deck')
    activeIndex = 0
    warnings = deck?.warnings ?? []
    showWarnings = true
    showRecovery = false
  }

  /** Opens an existing .pptx file via the system dialog. */
  async function onOpen(): Promise<void> {
    const path = await open({
      multiple: false,
      filters: [{ name: 'Presentation', extensions: ['pptx'] }],
    })
    if (typeof path !== 'string') return
    deck = await invoke<DeckSnapshot>('open_deck', { path })
    activeIndex = 0
    warnings = deck?.warnings ?? []
    showWarnings = true
  }

  /** Saves the current deck to a .pptx file via the system dialog. */
  async function onSave(): Promise<void> {
    const path = await save({
      filters: [{ name: 'Presentation', extensions: ['pptx'] }],
    })
    if (typeof path !== 'string') return
    await invoke('save_deck', { path })
  }

  /** Undoes the last edit and refreshes from the returned snapshot. */
  async function onUndo(): Promise<void> {
    deck = await invoke<DeckSnapshot>('undo')
    activeIndex = Math.min(activeIndex, (deck?.slides.length ?? 1) - 1)
  }

  /** Opens the presenter window. */
  async function onStartPresenter(): Promise<void> {
    await invoke('start_presenter')
  }

  /** Sends a text-box edit command to Rust and re-renders from the snapshot. */
  async function handleTextEdit(detail: {
    slideId: string
    shapeIndex: number
    paragraphs: ParagraphDto[]
  }): Promise<void> {
    deck = await invoke<DeckSnapshot>('edit_text_box', {
      slide_id: detail.slideId,
      shape_index: detail.shapeIndex,
      paragraphs: detail.paragraphs,
    })
  }

  /** Finds the run index in a paragraph that contains the given character position. */
  function runIndexAtPosition(paragraph: ParagraphDto, position: number): number {
    if (paragraph.runs.length === 0) return 0
    let offset = 0
    for (let i = 0; i < paragraph.runs.length; i++) {
      const run = paragraph.runs[i]
      if (position <= offset + run.text.length) {
        return i
      }
      offset += run.text.length
    }
    return paragraph.runs.length - 1
  }

  /** Identifies the active text box, paragraph, and run from the focused textarea. */
  function getTextTarget():
    | {
        slideId: string
        shapeIndex: number
        paragraphIndex: number
        runIndex: number
        run: RunDto
        paragraph: ParagraphDto
      }
    | null {
    const active = document.activeElement
    if (!(active instanceof HTMLTextAreaElement)) return null
    const slideId = active.dataset.slideId
    const shapeIndexRaw = active.dataset.shapeIndex
    if (!slideId || shapeIndexRaw === undefined) return null
    const shapeIndex = Number(shapeIndexRaw)
    if (!activeSlide) return null
    const shape = activeSlide.shapes[shapeIndex]
    if (!shape || shape.kind !== 'text_box') return null
    const textBox = shape.value as TextBoxSnapshot

    const selection = active.selectionStart ?? 0
    const text = active.value
    const linesBefore = text.slice(0, selection).split('\n')
    const paragraphIndex = Math.max(0, linesBefore.length - 1)
    const paragraphText = linesBefore[linesBefore.length - 1] ?? ''

    const paragraph = textBox.paragraphs[paragraphIndex]
    if (!paragraph) return null
    const paragraphRunsText = paragraph.runs.map((r) => r.text).join('')
    const position = paragraphText.length
    const runIndex =
      paragraphText === paragraphRunsText
        ? runIndexAtPosition(paragraph, position)
        : paragraph.runs.length > 0
          ? 0
          : 0
    const run = paragraph.runs[runIndex] ?? {
      text: '',
      bold: false,
      italic: false,
      underline: false,
      strikethrough: false,
      verticalAlign: 'baseline' as const,
      code: false,
    }
    return { slideId, shapeIndex, paragraphIndex, runIndex, run, paragraph }
  }

  /** Toggles a run-level boolean flag by wrapping SetRunStyle. */
  async function toggleRunFlag(flag: 'bold' | 'italic' | 'underline' | 'strikethrough' | 'code'): Promise<void> {
    const target = getTextTarget()
    if (!target) return
    const value = !target.run[flag]
    deck = await invoke<DeckSnapshot>('set_run_style', {
      slide_id: target.slideId,
      shape_index: target.shapeIndex,
      paragraph_index: target.paragraphIndex,
      run_index: target.runIndex,
      [flag]: value,
    })
  }

  /** Toggles superscript on the active run. */
  async function toggleSuperscript(): Promise<void> {
    const target = getTextTarget()
    if (!target) return
    const value = target.run.verticalAlign === 'superscript' ? 'baseline' : 'superscript'
    deck = await invoke<DeckSnapshot>('set_run_style', {
      slide_id: target.slideId,
      shape_index: target.shapeIndex,
      paragraph_index: target.paragraphIndex,
      run_index: target.runIndex,
      vertical_align: value,
    })
  }

  /** Toggles subscript on the active run. */
  async function toggleSubscript(): Promise<void> {
    const target = getTextTarget()
    if (!target) return
    const value = target.run.verticalAlign === 'subscript' ? 'baseline' : 'subscript'
    deck = await invoke<DeckSnapshot>('set_run_style', {
      slide_id: target.slideId,
      shape_index: target.shapeIndex,
      paragraph_index: target.paragraphIndex,
      run_index: target.runIndex,
      vertical_align: value,
    })
  }

  /** Applies a heading level to the active paragraph. */
  async function setHeading(level: HeadingLevelDto | null): Promise<void> {
    const target = getTextTarget()
    if (!target) return
    const style = {
      ...target.paragraph.style,
      heading: level ?? undefined,
    }
    deck = await invoke<DeckSnapshot>('set_paragraph_style', {
      slide_id: target.slideId,
      shape_index: target.shapeIndex,
      paragraph_index: target.paragraphIndex,
      style,
    })
  }

  /** Toggles a paragraph-level boolean flag. */
  async function toggleParagraphFlag(flag: 'blockquote' | 'codeBlock'): Promise<void> {
    const target = getTextTarget()
    if (!target) return
    const style = {
      ...target.paragraph.style,
      [flag]: !target.paragraph.style[flag],
    }
    deck = await invoke<DeckSnapshot>('set_paragraph_style', {
      slide_id: target.slideId,
      shape_index: target.shapeIndex,
      paragraph_index: target.paragraphIndex,
      style,
    })
  }

  /** Opens the file picker to choose an image to insert onto the active slide. */
  function onInsertImage(): void {
    imageInput?.click()
  }

  /** Reads the chosen image file and inserts it onto the active slide. */
  async function handleImageSelected(event: Event): Promise<void> {
    const input = event.target as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file || !activeSlide) return
    const buffer = await file.arrayBuffer()
    const bytes = Array.from(new Uint8Array(buffer))
    deck = await invoke<DeckSnapshot>('insert_image', {
      slide_id: activeSlide.id,
      bytes,
    })
  }

  /** Appends a geometric shape of the given kind to the active slide. */
  async function onAddShape(geometryKind: string): Promise<void> {
    if (!activeSlide) return
    deck = await invoke<DeckSnapshot>('add_shape', {
      slide_id: activeSlide.id,
      geometry_kind: geometryKind,
    })
  }

  /** Selects a different slide in the thumbnail panel. */
  function selectSlide(index: number): void {
    activeIndex = index
  }

  /** Restores a recovery snapshot as the current deck. */
  async function handleRestore(id: string): Promise<void> {
    deck = await invoke<DeckSnapshot>('restore_recovery', { id })
    activeIndex = 0
    warnings = deck?.warnings ?? []
    showWarnings = true
    showRecovery = false
  }

  /** Discards a recovery snapshot and falls back to a new deck if none remain. */
  async function handleDiscard(id: string): Promise<void> {
    await invoke('discard_recovery', { id })
    recoverySnapshots = recoverySnapshots.filter((s) => s.id !== id)
    if (recoverySnapshots.length === 0) {
      showRecovery = false
      await newDeck()
    }
  }

  /** Skips recovery and creates a new deck, keeping the snapshots on disk. */
  async function handleSkip(): Promise<void> {
    showRecovery = false
    await newDeck()
  }
</script>

{#if isPresenter}
  <Presenter />
{:else}
  <div class="app">
    <header class="toolbar">
      <button onclick={newDeck} type="button">New</button>
      <button onclick={onOpen} type="button">Open</button>
      <button onclick={onSave} type="button">Save</button>
      <button onclick={onUndo} type="button">Undo</button>
      <button onclick={onStartPresenter} type="button">Present</button>
      <span class="toolbar-divider"></span>
      <button onclick={onInsertImage} type="button">Insert Image</button>
      <span class="shape-group">
        <span class="shape-label">Shape:</span>
        <button onclick={() => onAddShape('rectangle')} type="button">Rectangle</button>
        <button onclick={() => onAddShape('ellipse')} type="button">Ellipse</button>
        <button onclick={() => onAddShape('triangle')} type="button">Triangle</button>
      </span>
      <span class="toolbar-divider"></span>
      <span class="text-group">
        <span class="shape-label">Text:</span>
        <button onclick={() => toggleRunFlag('bold')} type="button" title="Bold">B</button>
        <button onclick={() => toggleRunFlag('italic')} type="button" title="Italic">I</button>
        <button onclick={() => toggleRunFlag('underline')} type="button" title="Underline">U</button>
        <button onclick={() => toggleRunFlag('strikethrough')} type="button" title="Strikethrough">S</button>
        <button onclick={toggleSuperscript} type="button" title="Superscript">x²</button>
        <button onclick={toggleSubscript} type="button" title="Subscript">x₂</button>
        <button onclick={() => toggleRunFlag('code')} type="button" title="Inline code">&lt;/&gt;</button>
        <select
          onchange={(event) => {
            const value = (event.target as HTMLSelectElement).value
            setHeading(value === 'paragraph' ? null : (value as HeadingLevelDto))
          }}
          title="Heading"
        >
          <option value="paragraph">Paragraph</option>
          <option value="h1">Heading 1</option>
          <option value="h2">Heading 2</option>
          <option value="h3">Heading 3</option>
          <option value="h4">Heading 4</option>
          <option value="h5">Heading 5</option>
          <option value="h6">Heading 6</option>
        </select>
        <button onclick={() => toggleParagraphFlag('blockquote')} type="button" title="Blockquote">Quote</button>
        <button onclick={() => toggleParagraphFlag('codeBlock')} type="button" title="Code block">Block</button>
      </span>
      <input
        bind:this={imageInput}
        class="hidden-input"
        type="file"
        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml"
        onchange={handleImageSelected}
      />
    </header>

    {#if showWarnings && warnings.length > 0}
      <div class="banner" role="alert">
        <strong>Warnings:</strong>
        <ul>
          {#each warnings as warning}
            <li>{warning.slideId}: {warning.message}</li>
          {/each}
        </ul>
        <button onclick={() => (showWarnings = false)} type="button">Dismiss</button>
      </div>
    {/if}

    <div class="workspace">
      <aside class="sidebar" aria-label="Slide thumbnails">
        {#if deck}
          {#each deck.slides as slide, index}
            <SlideThumbnail
              {slide}
              selected={index === activeIndex}
              onClick={() => selectSlide(index)}
            />
          {/each}
        {/if}
      </aside>

      <main class="canvas-area" aria-label="Editor canvas">
        {#if activeSlide && deck}
          <SlideCanvas
            slide={activeSlide}
            background={deck.theme.background}
            media={deck.media}
            onEditTextBox={handleTextEdit}
          />
        {:else}
          <div class="empty-canvas">Open or create a deck to start editing.</div>
        {/if}
      </main>

      <aside class="notes" aria-label="Speaker notes">
        <h2>Notes</h2>
        {#if notes}
          <p>{notes}</p>
        {:else}
          <p class="placeholder">No notes for this slide.</p>
        {/if}
      </aside>
    </div>

    {#if showRecovery}
      <RecoveryPrompt
        snapshots={recoverySnapshots}
        onRestore={handleRestore}
        onDiscard={handleDiscard}
        onSkip={handleSkip}
      />
    {/if}
  </div>
{/if}

<style>
  :global(body) {
    margin: 0;
    font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  }
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .toolbar {
    display: flex;
    gap: 0.5rem;
    padding: 0.5rem;
    border-bottom: 1px solid #ccc;
    background: #f4f4f4;
  }
  .toolbar button {
    padding: 0.4rem 0.8rem;
  }
  .toolbar-divider {
    width: 1px;
    align-self: stretch;
    background: #ccc;
    margin: 0 0.25rem;
  }
  .shape-group {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .text-group {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .text-group select {
    padding: 0.3rem 0.4rem;
  }
  .shape-label {
    font-size: 0.85rem;
    color: #555;
  }
  .hidden-input {
    display: none;
  }
  .banner {
    padding: 0.5rem 1rem;
    background: #fff3cd;
    border-bottom: 1px solid #e0c36e;
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .banner ul {
    margin: 0;
    padding-left: 1rem;
    flex: 1;
  }
  .workspace {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
  .sidebar {
    width: 180px;
    overflow-y: auto;
    border-right: 1px solid #ccc;
    background: #fafafa;
    padding: 0.5rem;
  }
  .canvas-area {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #e0e0e0;
    overflow: auto;
  }
  .empty-canvas {
    color: #666;
  }
  .notes {
    width: 220px;
    border-left: 1px solid #ccc;
    padding: 0.5rem;
    background: #fafafa;
    overflow-y: auto;
  }
  .notes h2 {
    font-size: 1rem;
    margin-top: 0;
  }
  .placeholder {
    color: #888;
  }
</style>
