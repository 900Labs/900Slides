<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'
  import SlideThumbnail from './SlideThumbnail.svelte'
  import SlideCanvas from './SlideCanvas.svelte'
  import Presenter from './Presenter.svelte'
  import RecoveryPrompt from './RecoveryPrompt.svelte'
  import ChartEditor from './ChartEditor.svelte'
  import type {
    ChartDataDto,
    ChartShapeSnapshot,
    ChartTypeDto,
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
  /** Currently focused table cell, used by the table row/column toolbar. */
  let activeCell = $state<{ shapeIndex: number; row: number; col: number } | null>(null)
  /** Whether the table size picker popover is open. */
  let showTablePicker = $state(false)
  /** Hovered dimensions in the table size picker (1-based). */
  let pickerRows = $state(1)
  let pickerCols = $state(1)
  /** Whether the chart type dropdown is open. */
  let showChartDropdown = $state(false)
  /** Currently edited chart, if any. */
  let activeChart = $state<{ shapeIndex: number } | null>(null)

  /** Maximum grid dimension offered by the table size picker. */
  const PICKER_MAX = 6
  /** Chart types offered by the chart toolbar dropdown. */
  const CHART_TYPES: ChartTypeDto[] = ['bar', 'column', 'line', 'area', 'pie', 'scatter']
  /** Row/column indices for the picker grid. */
  const pickerIndices = [...Array(PICKER_MAX).keys()]

  const hasActiveCell = $derived(activeCell !== null)

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

  /** Appends a new `rows` x `cols` table to the active slide. */
  async function onAddTable(rows: number, cols: number): Promise<void> {
    if (!activeSlide) return
    showTablePicker = false
    deck = await invoke<DeckSnapshot>('add_table', {
      slide_id: activeSlide.id,
      rows,
      cols,
    })
  }

  /** Commits a table cell text edit and re-renders from the snapshot. */
  async function handleSetCellText(detail: {
    slideId: string
    shapeIndex: number
    row: number
    col: number
    text: string
  }): Promise<void> {
    deck = await invoke<DeckSnapshot>('set_cell_text', {
      slide_id: detail.slideId,
      shape_index: detail.shapeIndex,
      row: detail.row,
      col: detail.col,
      text: detail.text,
    })
  }

  /** Records the focused cell so the row/column toolbar can target it. */
  function handleCellFocus(detail: { shapeIndex: number; row: number; col: number }): void {
    activeCell = detail
  }

  /** Inserts a row below the focused cell. */
  async function onInsertRow(): Promise<void> {
    if (!activeSlide || !activeCell) return
    deck = await invoke<DeckSnapshot>('insert_row', {
      slide_id: activeSlide.id,
      shape_index: activeCell.shapeIndex,
      index: activeCell.row + 1,
    })
  }

  /** Inserts a column to the right of the focused cell. */
  async function onInsertColumn(): Promise<void> {
    if (!activeSlide || !activeCell) return
    deck = await invoke<DeckSnapshot>('insert_column', {
      slide_id: activeSlide.id,
      shape_index: activeCell.shapeIndex,
      index: activeCell.col + 1,
    })
  }

  /** Deletes the focused cell's row. */
  async function onDeleteRow(): Promise<void> {
    if (!activeSlide || !activeCell) return
    deck = await invoke<DeckSnapshot>('delete_row', {
      slide_id: activeSlide.id,
      shape_index: activeCell.shapeIndex,
      index: activeCell.row,
    })
  }

  /** Deletes the focused cell's column. */
  async function onDeleteColumn(): Promise<void> {
    if (!activeSlide || !activeCell) return
    deck = await invoke<DeckSnapshot>('delete_column', {
      slide_id: activeSlide.id,
      shape_index: activeCell.shapeIndex,
      index: activeCell.col,
    })
  }

  /** Appends a chart of the given type to the active slide. */
  async function onAddChart(chartType: ChartTypeDto): Promise<void> {
    if (!activeSlide) return
    showChartDropdown = false
    deck = await invoke<DeckSnapshot>('add_chart', {
      slide_id: activeSlide.id,
      chart_type: chartType,
    })
  }

  /** Opens the chart data-table editor for the given shape. */
  function handleEditChart(detail: { slideId: string; shapeIndex: number }): void {
    activeChart = { shapeIndex: detail.shapeIndex }
  }

  /** Applies changes from the chart editor and re-renders from the snapshot. */
  async function handleChartApply(detail: {
    slideId: string
    shapeIndex: number
    chartType: ChartTypeDto
    data: ChartDataDto
    title: string
  }): Promise<void> {
    if (!deck) return

    try {
      // Always send the (possibly converted) data first. If the data kind no
      // longer matches the chart's current type, the backend command also
      // switches the type to a sensible default, after which we can refine it.
      deck = await invoke<DeckSnapshot>('set_chart_data', {
        slide_id: detail.slideId,
        shape_index: detail.shapeIndex,
        data: detail.data,
      })

      const updated = activeSlide?.shapes[detail.shapeIndex]
      const updatedType =
        updated && updated.kind === 'chart' ? (updated.value as ChartShapeSnapshot).chartType : null
      if (updatedType && updatedType !== detail.chartType) {
        deck = await invoke<DeckSnapshot>('set_chart_type', {
          slide_id: detail.slideId,
          shape_index: detail.shapeIndex,
          chart_type: detail.chartType,
        })
      }

      deck = await invoke<DeckSnapshot>('set_chart_title', {
        slide_id: detail.slideId,
        shape_index: detail.shapeIndex,
        title: detail.title,
      })
    } catch (err) {
      console.error('Failed to apply chart changes:', err)
      window.alert(`Chart update failed: ${err}`)
    }
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
      <span class="table-group">
        <div class="table-picker-wrap">
          <button
            onclick={() => {
              showTablePicker = !showTablePicker
              pickerRows = 1
              pickerCols = 1
            }}
            type="button"
          >
            Table
          </button>
          {#if showTablePicker}
            <button
              class="picker-backdrop"
              onclick={() => (showTablePicker = false)}
              type="button"
              aria-label="Close table size picker"
            ></button>
            <div class="table-picker" role="dialog" aria-label="Choose table size">
              <div class="table-picker-grid">
                {#each pickerIndices as r}
                  {#each pickerIndices as c}
                    <button
                      class="picker-cell"
                      class:active={pickerRows > r && pickerCols > c}
                      onmouseenter={() => {
                        pickerRows = r + 1
                        pickerCols = c + 1
                      }}
                      onclick={() => onAddTable(r + 1, c + 1)}
                      type="button"
                      aria-label={`Insert ${r + 1} by ${c + 1} table`}
                    ></button>
                  {/each}
                {/each}
              </div>
              <div class="table-picker-label">{pickerRows} × {pickerCols}</div>
            </div>
          {/if}
        </div>
        <button onclick={onInsertRow} type="button" disabled={!hasActiveCell} title="Insert row below">+ Row</button>
        <button onclick={onInsertColumn} type="button" disabled={!hasActiveCell} title="Insert column right">+ Col</button>
        <button onclick={onDeleteRow} type="button" disabled={!hasActiveCell} title="Delete row">− Row</button>
        <button onclick={onDeleteColumn} type="button" disabled={!hasActiveCell} title="Delete column">− Col</button>
      </span>
      <span class="toolbar-divider"></span>
      <span class="chart-group">
        <div class="chart-picker-wrap">
          <button
            onclick={() => (showChartDropdown = !showChartDropdown)}
            type="button"
          >
            Chart
          </button>
          {#if showChartDropdown}
            <button
              class="picker-backdrop"
              onclick={() => (showChartDropdown = false)}
              type="button"
              aria-label="Close chart type picker"
            ></button>
            <div class="chart-dropdown" role="menu" aria-label="Choose chart type">
              {#each CHART_TYPES as type}
                <button
                  onclick={() => onAddChart(type)}
                  type="button"
                  role="menuitem"
                >
                  {type.charAt(0).toUpperCase() + type.slice(1)}
                </button>
              {/each}
            </div>
          {/if}
        </div>
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
            onSetCellText={handleSetCellText}
            onCellFocus={handleCellFocus}
            onEditChart={handleEditChart}
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

    {#if activeChart && activeSlide}
      {@const chartShape = activeSlide.shapes[activeChart.shapeIndex]}
      {#if chartShape && chartShape.kind === 'chart'}
        <ChartEditor
          chart={chartShape.value as ChartShapeSnapshot}
          slideId={activeSlide.id}
          shapeIndex={activeChart.shapeIndex}
          onApply={handleChartApply}
          onClose={() => (activeChart = null)}
        />
      {/if}
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
  .table-group {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .table-group button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .table-picker-wrap {
    position: relative;
    display: inline-flex;
  }
  .picker-backdrop {
    position: fixed;
    inset: 0;
    z-index: 5;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .table-picker {
    position: absolute;
    top: 100%;
    left: 0;
    z-index: 10;
    margin-top: 0.25rem;
    padding: 0.4rem;
    background: #fff;
    border: 1px solid #ccc;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  }
  .table-picker-grid {
    display: grid;
    grid-template-columns: repeat(6, 1rem);
    grid-template-rows: repeat(6, 1rem);
    gap: 2px;
  }
  .picker-cell {
    padding: 0;
    border: 1px solid #ddd;
    background: #fafafa;
    cursor: pointer;
  }
  .picker-cell.active {
    background: #0070c0;
    border-color: #0070c0;
  }
  .table-picker-label {
    margin-top: 0.3rem;
    font-size: 0.75rem;
    color: #555;
    text-align: center;
  }
  .chart-group {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .chart-picker-wrap {
    position: relative;
    display: inline-flex;
  }
  .chart-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    z-index: 10;
    margin-top: 0.25rem;
    display: flex;
    flex-direction: column;
    min-width: 120px;
    background: #fff;
    border: 1px solid #ccc;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  }
  .chart-dropdown button {
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid #eee;
    padding: 0.4rem 0.8rem;
    cursor: pointer;
  }
  .chart-dropdown button:last-child {
    border-bottom: none;
  }
  .chart-dropdown button:hover {
    background: #f0f0f0;
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
