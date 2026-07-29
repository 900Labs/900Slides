<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'
  import SlideThumbnail from './SlideThumbnail.svelte'
  import SlideCanvas from './SlideCanvas.svelte'
  import Presenter from './Presenter.svelte'
  import AudienceWindow from './AudienceWindow.svelte'
  import RecoveryPrompt from './RecoveryPrompt.svelte'
  import ChartEditor from './ChartEditor.svelte'
  import RichNotesEditor from './RichNotesEditor.svelte'
  import FindReplace from './FindReplace.svelte'
  import ShortcutsDialog from './ShortcutsDialog.svelte'
  import TemplatePicker from './TemplatePicker.svelte'
  import type {
    AnimationDto,
    BuildEffectDto,
    BuildStepDto,
    ChartDataDto,
    ChartShapeSnapshot,
    ChartTypeDto,
    DeckSnapshot,
    HeadingLevelDto,
    ParagraphDto,
    ParagraphStyleDto,
    RecoverySnapshot,
    RunDto,
    SlideSectionDto,
    SlideSizeDto,
    SlideSnapshot,
    TemplateInfoDto,
    TextBoxSnapshot,
    TransitionKindDto,
    WarningDto,
  } from './lib/types'

  /** True if this window is the presenter control view. */
  const isPresenter = window.location.hash === '#/presenter'
  /** True if this window is the fullscreen audience view. */
  const isAudience = window.location.hash === '#/audience'

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
  /** Active right-panel tab: 'notes' or 'animation'. */
  let rightPanelTab = $state<'notes' | 'animation'>('notes')
  /** Selected shape index for adding a build step. */
  let selectedShapeIndex = $state<number>(0)
  /** Selected build effect for adding a build step. */
  let selectedBuildEffect = $state<BuildEffectDto>('fade')
  /** Duration (ms) for a new build step. */
  let selectedBuildDuration = $state(500)
  /** Duration (ms) for the slide transition. */
  let transitionDuration = $state(500)
  /** Whether the find/replace dialog is open. */
  let showFindReplace = $state(false)
  /** Whether the find dialog opens focused on the replace field. */
  let findReplaceMode = $state<'find' | 'replace'>('find')
  /** Whether the shortcuts dialog is open. */
  let showShortcuts = $state(false)
  /** Slide-section start ids whose headers are collapsed in the sidebar. */
  let collapsedSections = $state<Set<string>>(new Set())
  /** Name being typed for a new slide section. */
  let newSectionName = $state('')
  /** Slide id at which a new section will start. */
  let newSectionSlideId = $state<string>('')
  /** Whether the template picker dialog is open. */
  let showTemplatePicker = $state(false)
  /** Built-in templates loaded for the picker. */
  let templates = $state<TemplateInfoDto[]>([])
  /** Whether the export dropdown menu is open. */
  let showExportMenu = $state(false)
  /** Which export is currently running (`''` when idle). */
  let exporting = $state<'' | 'svg' | 'png' | 'pdf'>('')
  /** Human-readable error from the last export, shown until dismissed. */
  let exportError = $state('')
  /** Active text-box paragraph the cursor is in, so the toolbar can show and
   *  edit paragraph-level options (e.g. code-step ranges) for it. */
  let activeTextTarget = $state<{
    shapeIndex: number
    paragraphIndex: number
    style: ParagraphStyleDto
  } | null>(null)
  /** Bound step-ranges input, so committing the value does not hide it. */
  let codeStepsInput = $state<HTMLInputElement | null>(null)

  /** Maximum grid dimension offered by the table size picker. */
  const PICKER_MAX = 6
  /** Chart types offered by the chart toolbar dropdown. */
  const CHART_TYPES: ChartTypeDto[] = ['bar', 'column', 'line', 'area', 'pie', 'scatter']
  /** Slide transition kinds offered by the transition picker. */
  const TRANSITION_KINDS: TransitionKindDto[] = ['none', 'fade', 'slide', 'push', 'wipe', 'morph']
  /** Build-in effects offered by the build step picker. */
  const BUILD_EFFECTS: BuildEffectDto[] = [
    'fade',
    'slide_in_left',
    'slide_in_right',
    'slide_in_top',
    'slide_in_bottom',
    'appear',
    'disappear',
  ]
  /** Row/column indices for the picker grid. */
  const pickerIndices = [...Array(PICKER_MAX).keys()]

  const hasActiveCell = $derived(activeCell !== null)

  const activeSlide = $derived<SlideSnapshot | null>(deck?.slides[activeIndex] ?? null)
  const notes = $derived(activeSlide?.notes ?? '')
  const activeTransitionKind = $derived<TransitionKindDto>(activeSlide?.transition?.kind ?? 'none')
  const activeTransitionDurationMs = $derived<number>(activeSlide?.transition?.durationMs ?? 500)
  const activeAnimation = $derived<AnimationDto | undefined>(activeSlide?.animation)

  /** Deck slide size (aspect ratio), when set. */
  const slideSize = $derived<SlideSizeDto | undefined>(deck?.slideSize)
  /** Whether the deck theme is in high-contrast mode. */
  const highContrast = $derived<boolean>(deck?.theme.highContrast ?? false)
  /** Rich-text notes for the active slide, when present. */
  const activeRichNotes = $derived<ParagraphDto[] | undefined>(activeSlide?.richNotes)

  /** Layouts available on the current deck (from its template). */
  const deckLayouts = $derived(deck?.layouts ?? [])
  /** Name of the layout the active slide uses, or '' when none. */
  const activeLayoutRef = $derived<string>(activeSlide?.layoutRef ?? '')

  /** Preset aspect-ratio slide sizes, in EMU, matching the Rust constructors. */
  const ASPECT_PRESETS: Record<'16:9' | '4:3' | '16:10', SlideSizeDto> = {
    '16:9': { widthEmu: 12_192_000, heightEmu: 6_858_000 },
    '4:3': { widthEmu: 9_144_000, heightEmu: 6_858_000 },
    '16:10': { widthEmu: 12_149_333, heightEmu: 7_593_333 },
  }

  /** The current aspect-ratio preset, or 'default' when the size is custom/unset. */
  const currentAspectRatio = $derived.by<'16:9' | '4:3' | '16:10' | 'default'>(() => {
    const size = slideSize
    if (!size) return '16:9'
    for (const key of ['16:9', '4:3', '16:10'] as const) {
      const preset = ASPECT_PRESETS[key]
      if (preset.widthEmu === size.widthEmu && preset.heightEmu === size.heightEmu) {
        return key
      }
    }
    return 'default'
  })

  /** Ordered section groups for the sidebar: each carries its section (or null
   *  for slides before the first section) and the slide indices it spans. */
  const sectionGroups = $derived.by<
    { section: SlideSectionDto | null; indices: number[] }[]
  >(() => {
    const slides = deck?.slides ?? []
    if (slides.length === 0) return []
    const sections = deck?.sections ?? []
    const starts = sections
      .map((section) => ({
        section,
        idx: slides.findIndex((slide) => slide.id === section.startSlideId),
      }))
      .filter((entry) => entry.idx >= 0)
      .sort((a, b) => a.idx - b.idx)

    const groups: { section: SlideSectionDto | null; indices: number[] }[] = [
      { section: null, indices: [] },
    ]
    let cursor = 0
    for (let i = 0; i < slides.length; i += 1) {
      if (cursor < starts.length && starts[cursor].idx === i) {
        groups.push({ section: starts[cursor].section, indices: [] })
        cursor += 1
      }
      groups[groups.length - 1].indices.push(i)
    }
    return groups.filter((group) => group.indices.length > 0)
  })

  $effect(() => {
    if (!isPresenter && !isAudience) {
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

  /** Creates a new blank deck from the Rust model, optionally applying a template. */
  async function newDeck(templateName?: string | null): Promise<void> {
    const payload = templateName ? { template_name: templateName } : {}
    deck = await invoke<DeckSnapshot>('new_deck', payload)
    activeIndex = 0
    warnings = deck?.warnings ?? []
    showWarnings = true
    showRecovery = false
  }

  /** Loads built-in templates and opens the template picker. */
  async function openTemplatePicker(): Promise<void> {
    if (templates.length === 0) {
      try {
        templates = await invoke<TemplateInfoDto[]>('list_templates')
      } catch (err) {
        console.error('Failed to list templates:', err)
      }
    }
    showTemplatePicker = true
  }

  /** Creates a new deck from the picker's selected template (or blank). */
  async function onSelectTemplate(templateName: string | null): Promise<void> {
    showTemplatePicker = false
    await newDeck(templateName)
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

  /** Filename stem used as the default for export save dialogs. */
  function exportStem(): string {
    return deck?.id || 'deck'
  }

  /** Guards an export: disables the menu, surfaces errors, clears the busy flag. */
  async function runExport(kind: 'svg' | 'png' | 'pdf', task: () => Promise<void>): Promise<void> {
    exporting = kind
    exportError = ''
    try {
      await task()
    } catch (error) {
      exportError = error instanceof Error ? error.message : String(error)
    } finally {
      exporting = ''
    }
  }

  /** Exports the current slide as a standalone SVG via the system save dialog. */
  async function onExportSvg(): Promise<void> {
    showExportMenu = false
    const slide = deck?.slides[activeIndex]
    if (!slide) return
    const path = await save({
      defaultPath: `${exportStem()}-slide-${activeIndex + 1}.svg`,
      filters: [{ name: 'SVG', extensions: ['svg'] }],
    })
    if (typeof path !== 'string') return
    await runExport('svg', () => invoke('export_svg', { slideId: slide.id, filePath: path }))
  }

  /** Exports the current slide as a 2x PNG via the system save dialog. */
  async function onExportPng(): Promise<void> {
    showExportMenu = false
    const slide = deck?.slides[activeIndex]
    if (!slide) return
    const path = await save({
      defaultPath: `${exportStem()}-slide-${activeIndex + 1}.png`,
      filters: [{ name: 'PNG', extensions: ['png'] }],
    })
    if (typeof path !== 'string') return
    await runExport('png', () =>
      invoke('export_png', { slideId: slide.id, scale: 2, filePath: path }),
    )
  }

  /** Exports the entire deck as a multi-page PDF via the system save dialog. */
  async function onExportPdf(): Promise<void> {
    showExportMenu = false
    if (!deck) return
    const path = await save({
      defaultPath: `${exportStem()}.pdf`,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    })
    if (typeof path !== 'string') return
    await runExport('pdf', () => invoke('export_pdf', { filePath: path }))
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

  /** Captures the paragraph the cursor is currently in, so the toolbar can show
   *  and edit its paragraph-level style (e.g. code-step ranges). It updates the
   *  target whenever a slide text area is focused and keeps it while the
   *  step-ranges input itself is focused (so committing the value does not hide
   *  the field). When focus is elsewhere the previous capture is retained until
   *  a slide change clears it. */
  function refreshActiveTextTarget(): void {
    const active = document.activeElement
    if (codeStepsInput && active === codeStepsInput) return
    if (!(active instanceof HTMLTextAreaElement)) return
    const slideId = active.dataset.slideId
    const shapeIndexRaw = active.dataset.shapeIndex
    if (!slideId || shapeIndexRaw === undefined || !activeSlide) return
    const shapeIndex = Number(shapeIndexRaw)
    const shape = activeSlide.shapes[shapeIndex]
    if (!shape || shape.kind !== 'text_box') return
    const textBox = shape.value as TextBoxSnapshot
    const selection = active.selectionStart ?? 0
    const linesBefore = active.value.slice(0, selection).split('\n')
    const paragraphIndex = Math.max(0, linesBefore.length - 1)
    const paragraph = textBox.paragraphs[paragraphIndex]
    if (!paragraph) return
    activeTextTarget = { shapeIndex, paragraphIndex, style: paragraph.style }
  }

  /** Id of the slide the capture was taken from, so a slide change clears it. */
  let lastCapturedSlideId: string | undefined

  // Track the focused paragraph as the cursor moves between text boxes and
  // lines, so the code-step field reflects the active code block.
  $effect(() => {
    const handler = (): void => refreshActiveTextTarget()
    document.addEventListener('selectionchange', handler)
    document.addEventListener('focusin', handler)
    return () => {
      document.removeEventListener('selectionchange', handler)
      document.removeEventListener('focusin', handler)
    }
  })

  // Clear stale captures on slide changes, and re-capture after any edit so the
  // field stays in sync with the now-updated paragraph.
  $effect(() => {
    void deck
    const slideId = activeSlide?.id
    if (slideId !== lastCapturedSlideId) {
      activeTextTarget = null
      lastCapturedSlideId = slideId
    }
    refreshActiveTextTarget()
  })

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

  /** Commits the typed stepped-code ranges (e.g. "1-3|4|5,7") to the active
   *  code-block paragraph. An empty value clears the ranges. */
  async function onCodeStepRangesChange(event: Event): Promise<void> {
    if (!activeTextTarget || !activeSlide) return
    const value = (event.target as HTMLInputElement).value
    const style: ParagraphStyleDto = {
      ...activeTextTarget.style,
      codeStepRanges: value.trim() === '' ? undefined : value,
    }
    deck = await invoke<DeckSnapshot>('set_paragraph_style', {
      slide_id: activeSlide.id,
      shape_index: activeTextTarget.shapeIndex,
      paragraph_index: activeTextTarget.paragraphIndex,
      style,
    })
    activeTextTarget = { ...activeTextTarget, style }
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

  /** Applies or clears the active slide's transition. */
  async function onSetTransition(kind: TransitionKindDto, durationMs: number): Promise<void> {
    if (!activeSlide) return
    deck = await invoke<DeckSnapshot>('set_transition', {
      slide_id: activeSlide.id,
      kind: kind === 'none' ? null : kind,
      duration_ms: durationMs,
    })
  }

  /** Sets or clears the active slide's layout reference. Pass '' to clear. */
  async function onSetSlideLayout(layoutName: string): Promise<void> {
    if (!activeSlide) return
    deck = await invoke<DeckSnapshot>('set_slide_layout', {
      slide_id: activeSlide.id,
      layout_name: layoutName === '' ? null : layoutName,
    })
  }

  /** Replaces the full animation sequence for the active slide. */
  async function onSetSlideAnimation(steps: BuildStepDto[]): Promise<void> {
    if (!activeSlide) return
    deck = await invoke<DeckSnapshot>('set_slide_animation', {
      slide_id: activeSlide.id,
      steps,
    })
  }

  /** Appends a build step to the active slide's animation sequence. */
  async function onAddBuildStep(): Promise<void> {
    if (!activeSlide) return
    deck = await invoke<DeckSnapshot>('add_build_step', {
      slide_id: activeSlide.id,
      shape_index: selectedShapeIndex,
      effect: selectedBuildEffect,
      duration_ms: selectedBuildDuration,
    })
  }

  /** Removes a build step by index. */
  async function onRemoveBuildStep(stepIndex: number): Promise<void> {
    if (!activeSlide) return
    deck = await invoke<DeckSnapshot>('remove_build_step', {
      slide_id: activeSlide.id,
      step_index: stepIndex,
    })
  }

  /** Moves a build step from one position to another. */
  async function onMoveBuildStep(from: number, to: number): Promise<void> {
    if (!activeSlide) return
    deck = await invoke<DeckSnapshot>('move_build_step', {
      slide_id: activeSlide.id,
      from,
      to,
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

  /** Sets the deck's aspect ratio to one of the presets, or clears it. */
  async function onSetAspectRatio(ratio: '16:9' | '4:3' | '16:10' | 'default'): Promise<void> {
    const slideSizePayload =
      ratio === 'default' ? null : { ...ASPECT_PRESETS[ratio] }
    deck = await invoke<DeckSnapshot>('set_slide_size', { slide_size: slideSizePayload })
  }

  /** Toggles the deck's high-contrast accessibility theme. */
  async function toggleHighContrast(): Promise<void> {
    deck = await invoke<DeckSnapshot>('set_high_contrast', {
      high_contrast: !highContrast,
    })
  }

  /** Commits rich-text notes (or clears them with null) for the active slide. */
  async function handleSetRichNotes(paragraphs: ParagraphDto[] | null): Promise<void> {
    if (!activeSlide) return
    deck = await invoke<DeckSnapshot>('set_rich_notes', {
      slide_id: activeSlide.id,
      rich_notes: paragraphs,
    })
  }

  /** Enables rich-text notes, seeded from the slide's plain notes. */
  async function handleEnableRichNotes(): Promise<void> {
    if (!activeSlide) return
    const seed: ParagraphDto[] =
      activeSlide.notes && activeSlide.notes.length > 0
        ? [
            {
              runs: [
                {
                  text: activeSlide.notes,
                  bold: false,
                  italic: false,
                  underline: false,
                  strikethrough: false,
                  verticalAlign: 'baseline',
                  code: false,
                },
              ],
              listStyle: 'none',
              style: { blockquote: false, codeBlock: false, indentLevel: 0 },
            },
          ]
        : [
            {
              runs: [],
              listStyle: 'none',
              style: { blockquote: false, codeBlock: false, indentLevel: 0 },
            },
          ]
    await handleSetRichNotes(seed)
  }

  /** Adds a new section starting at the chosen slide. */
  async function addSection(): Promise<void> {
    if (!deck) return
    const name = newSectionName.trim()
    const startSlideId = newSectionSlideId || deck.slides[0]?.id
    if (!name || !startSlideId) return
    const sections = [...deck.sections, { name, startSlideId }]
    deck = await invoke<DeckSnapshot>('set_sections', { sections })
    newSectionName = ''
  }

  /** Removes the section that starts at the given slide. */
  async function removeSection(startSlideId: string): Promise<void> {
    if (!deck) return
    const sections = deck.sections.filter((s) => s.startSlideId !== startSlideId)
    deck = await invoke<DeckSnapshot>('set_sections', { sections })
    const next = new Set(collapsedSections)
    next.delete(startSlideId)
    collapsedSections = next
  }

  /** Collapses or expands a section's thumbnails. */
  function toggleSectionCollapse(startSlideId: string): void {
    const next = new Set(collapsedSections)
    if (next.has(startSlideId)) {
      next.delete(startSlideId)
    } else {
      next.add(startSlideId)
    }
    collapsedSections = next
  }

  /** Applies a text-box edit from the find/replace dialog. */
  async function handleFindReplaceApply(detail: {
    slideId: string
    shapeIndex: number
    paragraphs: ParagraphDto[]
  }): Promise<void> {
    await handleTextEdit(detail)
  }

  /** Global keyboard shortcuts for the editor window. */
  function handleGlobalKey(event: KeyboardEvent): void {
    const mod = event.metaKey || event.ctrlKey
    const target = event.target as HTMLElement | null
    const typing =
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      (target?.isContentEditable ?? false)

    if (mod && event.key.toLowerCase() === 'f') {
      event.preventDefault()
      findReplaceMode = 'find'
      showFindReplace = true
    } else if (mod && event.key.toLowerCase() === 'h') {
      event.preventDefault()
      findReplaceMode = 'replace'
      showFindReplace = true
    } else if (!typing && event.key === '?') {
      event.preventDefault()
      showShortcuts = true
    }
  }
</script>

{#if isPresenter}
  <Presenter />
{:else if isAudience}
  <AudienceWindow />
{:else}
  <div class="app">
    <header class="toolbar">
      <button onclick={openTemplatePicker} type="button">New</button>
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
        {#if activeTextTarget?.style.codeBlock}
          <input
            bind:this={codeStepsInput}
            class="code-steps-input"
            type="text"
            value={activeTextTarget.style.codeStepRanges ?? ''}
            placeholder="Steps: 1-3|4|5,7"
            title="Stepped code ranges (pipe = next step, comma = same step, e.g. 1-3|4|5,7)"
            onchange={onCodeStepRangesChange}
          />
        {/if}
      </span>
      <span class="toolbar-divider"></span>
      <span class="deck-group">
        <span class="shape-label">Ratio:</span>
        <select
          value={currentAspectRatio}
          onchange={(event) =>
            onSetAspectRatio(
              (event.target as HTMLSelectElement).value as '16:9' | '4:3' | '16:10' | 'default',
            )
          }
          title="Slide aspect ratio"
        >
          <option value="16:9">16:9</option>
          <option value="4:3">4:3</option>
          <option value="16:10">16:10</option>
          <option value="default">Reset</option>
        </select>
        <button
          onclick={toggleHighContrast}
          type="button"
          class:active-toggle={highContrast}
          title="Toggle high-contrast theme"
        >
          Contrast
        </button>
        <button
          onclick={() => {
            findReplaceMode = 'find'
            showFindReplace = true
          }}
          type="button"
          title="Find (Ctrl/Cmd+F)"
        >
          Find
        </button>
        <span class="export-picker-wrap">
          <button
            onclick={() => (showExportMenu = !showExportMenu)}
            type="button"
            disabled={!deck || exporting !== ''}
            title="Export the current slide or the whole deck"
          >
            Export
          </button>
          {#if showExportMenu}
            <button
              class="picker-backdrop"
              onclick={() => (showExportMenu = false)}
              type="button"
              aria-label="Close export menu"
            ></button>
            <div class="export-dropdown" role="menu" aria-label="Export format">
              <button onclick={onExportSvg} type="button" role="menuitem" title="Current slide as SVG">
                SVG (current slide)
              </button>
              <button onclick={onExportPng} type="button" role="menuitem" title="Current slide as 2x PNG">
                PNG (current slide, 2x)
              </button>
              <button onclick={onExportPdf} type="button" role="menuitem" title="Entire deck as PDF">
                PDF (entire deck)
              </button>
            </div>
          {/if}
        </span>
        <button onclick={() => (showShortcuts = true)} type="button" title="Keyboard shortcuts (?)">
          ?
        </button>
      </span>
      <input
        bind:this={imageInput}
        class="hidden-input"
        type="file"
        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml"
        onchange={handleImageSelected}
      />
    </header>

    {#if exporting !== ''}
      <div class="banner export-progress" role="status" aria-live="polite">
        Exporting {exporting.toUpperCase()}…
        {#if exporting === 'pdf'}Rendering every slide as a PDF page can take a moment.{/if}
      </div>
    {/if}
    {#if exportError}
      <div class="banner" role="alert">
        <strong>Export failed:</strong> {exportError}
        <button onclick={() => (exportError = '')} type="button">Dismiss</button>
      </div>
    {/if}

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
          {#each sectionGroups as group (group.section?.startSlideId ?? '__nostart__')}
            {#if group.section}
              <div class="section-header">
                <button
                  type="button"
                  class="section-toggle"
                  onclick={() => toggleSectionCollapse(group.section!.startSlideId)}
                  aria-expanded={!collapsedSections.has(group.section!.startSlideId)}
                  title={collapsedSections.has(group.section!.startSlideId) ? 'Expand' : 'Collapse'}
                >
                  {collapsedSections.has(group.section!.startSlideId) ? '▶' : '▼'}
                </button>
                <span class="section-name" title={group.section.name}>{group.section.name}</span>
                <button
                  type="button"
                  class="section-remove"
                  onclick={() => removeSection(group.section!.startSlideId)}
                  title="Remove section"
                >
                  ✕
                </button>
              </div>
            {/if}
            {#if !group.section || !collapsedSections.has(group.section.startSlideId)}
              {#each group.indices as index (index)}
                {@const slide = deck.slides[index]}
                <SlideThumbnail
                  {slide}
                  selected={index === activeIndex}
                  onClick={() => selectSlide(index)}
                />
              {/each}
            {/if}
          {/each}

          <div class="section-add">
            <input
              class="section-name-input"
              type="text"
              placeholder="New section name"
              bind:value={newSectionName}
              aria-label="New section name"
            />
            <select
              class="section-slide-select"
              value={newSectionSlideId || activeSlide?.id || deck.slides[0]?.id || ''}
              onchange={(event) => (newSectionSlideId = (event.target as HTMLSelectElement).value)}
              aria-label="Section start slide"
            >
              {#each deck.slides as slide, index}
                <option value={slide.id}>Slide {index + 1}</option>
              {/each}
            </select>
            <button type="button" onclick={addSection} disabled={!newSectionName.trim()}>
              Add Section
            </button>
          </div>
        {/if}
      </aside>

      <main class="canvas-area" aria-label="Editor canvas">
        {#if activeSlide && deck}
          <SlideCanvas
            slide={activeSlide}
            background={deck.theme.background}
            media={deck.media}
            slideSize={slideSize}
            highContrast={highContrast}
            onEditTextBox={handleTextEdit}
            onSetCellText={handleSetCellText}
            onCellFocus={handleCellFocus}
            onEditChart={handleEditChart}
          />
        {:else}
          <div class="empty-canvas">Open or create a deck to start editing.</div>
        {/if}
      </main>

      <aside class="right-panel" aria-label="Notes and animation">
        <div class="tab-bar" role="tablist">
          <button
            class="tab"
            class:active={rightPanelTab === 'notes'}
            onclick={() => (rightPanelTab = 'notes')}
            type="button"
            role="tab"
            aria-selected={rightPanelTab === 'notes'}
          >
            Notes
          </button>
          <button
            class="tab"
            class:active={rightPanelTab === 'animation'}
            onclick={() => (rightPanelTab = 'animation')}
            type="button"
            role="tab"
            aria-selected={rightPanelTab === 'animation'}
          >
            Animation
          </button>
        </div>

        {#if rightPanelTab === 'notes'}
          <div class="panel-content" role="tabpanel">
            {#if activeRichNotes}
              <RichNotesEditor
                slideId={activeSlide!.id}
                richNotes={activeRichNotes}
                onSetRichNotes={handleSetRichNotes}
              />
              <button class="notes-toggle" type="button" onclick={() => handleSetRichNotes(null)}>
                Use plain notes
              </button>
            {:else}
              <div class="plain-notes">
                {#if notes}
                  <p>{notes}</p>
                {:else}
                  <p class="placeholder">No notes for this slide.</p>
                {/if}
                <button class="notes-toggle" type="button" onclick={handleEnableRichNotes}>
                  Use rich-text notes
                </button>
              </div>
            {/if}
          </div>
        {:else}
          <div class="panel-content" role="tabpanel">
            <div class="section">
              <h3>Layout</h3>
              {#if deckLayouts.length > 0}
                <select
                  value={activeLayoutRef}
                  onchange={(event) => onSetSlideLayout((event.target as HTMLSelectElement).value)}
                  title="Slide layout"
                >
                  <option value="">None</option>
                  {#each deckLayouts as layout}
                    <option value={layout.name}>{layout.name}</option>
                  {/each}
                </select>
              {:else}
                <p class="placeholder">No layouts. Apply a template to add layouts.</p>
              {/if}
            </div>

            <div class="section">
              <h3>Transition</h3>
              <label class="field">
                Kind
                <select
                  value={activeTransitionKind}
                  onchange={(event) =>
                    onSetTransition(
                      (event.target as HTMLSelectElement).value as TransitionKindDto,
                      activeTransitionDurationMs,
                    )}
                >
                  {#each TRANSITION_KINDS as kind}
                    <option value={kind}>{kind.charAt(0).toUpperCase() + kind.slice(1)}</option>
                  {/each}
                </select>
              </label>
              <label class="field">
                Duration: {activeTransitionDurationMs}ms
                <input
                  type="range"
                  min={0}
                  max={5000}
                  step={100}
                  value={activeTransitionDurationMs}
                  onchange={(event) =>
                    onSetTransition(
                      activeTransitionKind,
                      Number.parseInt((event.target as HTMLInputElement).value, 10),
                    )}
                />
              </label>
            </div>

            <div class="section">
              <h3>Build Steps</h3>
              {#if activeAnimation && activeAnimation.steps.length > 0}
                <ol class="build-list">
                  {#each activeAnimation.steps as step, stepIndex}
                    <li class="build-item">
                      <span class="build-info">
                        Shape {step.shapeIndex}: {step.effect.replace(/_/g, ' ')} ({step.durationMs}ms)
                      </span>
                      <span class="build-actions">
                        <button
                          onclick={() => onMoveBuildStep(stepIndex, stepIndex - 1)}
                          disabled={stepIndex === 0}
                          type="button"
                          title="Move up"
                        >
                          ↑
                        </button>
                        <button
                          onclick={() => onMoveBuildStep(stepIndex, stepIndex + 1)}
                          disabled={stepIndex === activeAnimation.steps.length - 1}
                          type="button"
                          title="Move down"
                        >
                          ↓
                        </button>
                        <button
                          onclick={() => onRemoveBuildStep(stepIndex)}
                          type="button"
                          title="Remove"
                        >
                          −
                        </button>
                      </span>
                    </li>
                  {/each}
                </ol>
              {:else}
                <p class="placeholder">No build steps yet.</p>
              {/if}
            </div>

            <div class="section">
              <h3>Add Build Step</h3>
              <label class="field">
                Shape
                <select bind:value={selectedShapeIndex}>
                  {#each activeSlide?.shapes ?? [] as shape, index}
                    <option value={index}>{index}: {shape.kind}</option>
                  {/each}
                </select>
              </label>
              <label class="field">
                Effect
                <select bind:value={selectedBuildEffect}>
                  {#each BUILD_EFFECTS as effect}
                    <option value={effect}>{effect.replace(/_/g, ' ')}</option>
                  {/each}
                </select>
              </label>
              <label class="field">
                Duration: {selectedBuildDuration}ms
                <input type="range" min={0} max={3000} step={100} bind:value={selectedBuildDuration} />
              </label>
              <button onclick={onAddBuildStep} type="button">Add Step</button>
            </div>
          </div>
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

    {#if showFindReplace && deck}
      <FindReplace
        {deck}
        startInReplace={findReplaceMode === 'replace'}
        onClose={() => (showFindReplace = false)}
        onApplyEdit={handleFindReplaceApply}
      />
    {/if}

    {#if showShortcuts}
      <ShortcutsDialog onClose={() => (showShortcuts = false)} />
    {/if}

    {#if showTemplatePicker}
      <TemplatePicker
        {templates}
        onSelect={onSelectTemplate}
        onCancel={() => (showTemplatePicker = false)}
      />
    {/if}
  </div>
{/if}

<svelte:window onkeydown={isPresenter || isAudience ? undefined : handleGlobalKey} />

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
  .code-steps-input {
    width: 9rem;
    padding: 0.3rem 0.4rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.8rem;
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
  .export-picker-wrap {
    position: relative;
    display: inline-flex;
  }
  .export-picker-wrap button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .export-dropdown {
    position: absolute;
    top: 100%;
    right: 0;
    z-index: 10;
    margin-top: 0.25rem;
    display: flex;
    flex-direction: column;
    min-width: 200px;
    background: #fff;
    border: 1px solid #ccc;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  }
  .export-dropdown button {
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid #eee;
    padding: 0.4rem 0.8rem;
    cursor: pointer;
  }
  .export-dropdown button:last-child {
    border-bottom: none;
  }
  .export-dropdown button:hover {
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
  .export-progress {
    background: #d1ecf1;
    border-bottom-color: #9ecfe0;
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
  .right-panel {
    width: 240px;
    border-left: 1px solid #ccc;
    background: #fafafa;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .tab-bar {
    display: flex;
    border-bottom: 1px solid #ccc;
  }
  .tab {
    flex: 1;
    padding: 0.5rem;
    background: #f0f0f0;
    border: none;
    border-right: 1px solid #ccc;
    cursor: pointer;
  }
  .tab:last-child {
    border-right: none;
  }
  .tab.active {
    background: #fafafa;
    font-weight: bold;
  }
  .panel-content {
    flex: 1;
    padding: 0.75rem;
    overflow-y: auto;
  }
  .panel-content h3 {
    font-size: 0.9rem;
    margin: 0 0 0.5rem;
  }
  .section {
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid #ddd;
  }
  .section:last-child {
    border-bottom: none;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 0.5rem;
    font-size: 0.8rem;
    color: #555;
  }
  .field input,
  .field select {
    padding: 0.25rem;
    font-size: 0.85rem;
  }
  .build-list {
    margin: 0;
    padding-left: 1.25rem;
    font-size: 0.85rem;
  }
  .build-item {
    margin-bottom: 0.25rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.25rem;
  }
  .build-info {
    flex: 1;
    text-transform: capitalize;
  }
  .build-actions {
    display: flex;
    gap: 0.1rem;
  }
  .build-actions button {
    padding: 0.1rem 0.3rem;
    font-size: 0.8rem;
  }
  .placeholder {
    color: #888;
  }
  .deck-group {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }
  .deck-group select {
    padding: 0.3rem 0.4rem;
  }
  .active-toggle {
    background: #0070c0 !important;
    color: #fff !important;
  }
  .section-header {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    margin: 0.25rem 0 0.15rem;
    padding: 0.2rem 0.25rem;
    background: #e8eef7;
    border: 1px solid #cdd9ea;
    border-radius: 3px;
  }
  .section-toggle {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0 0.2rem;
    font-size: 0.7rem;
    color: #335;
  }
  .section-name {
    flex: 1;
    font-size: 0.78rem;
    font-weight: 600;
    color: #234;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .section-remove {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 0.7rem;
    color: #a33;
    padding: 0 0.2rem;
  }
  .section-add {
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px dashed #ccc;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .section-name-input,
  .section-slide-select {
    padding: 0.2rem;
    font-size: 0.78rem;
  }
  .section-add button {
    padding: 0.25rem;
    font-size: 0.78rem;
  }
  .section-add button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .plain-notes {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .notes-toggle {
    margin-top: 0.5rem;
    padding: 0.25rem 0.4rem;
    font-size: 0.78rem;
  }
</style>
