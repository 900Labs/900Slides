<script lang="ts">
  import { invokeApp as invoke } from './lib/invokeDeckSnapshot'
  import { listen } from '@tauri-apps/api/event'
  import { tick } from 'svelte'
  import { open, save } from '@tauri-apps/plugin-dialog'
  import SlideThumbnail from './SlideThumbnail.svelte'
  import SlideCanvas from './SlideCanvas.svelte'
  import ShapePicker from './ShapePicker.svelte'
  import Presenter from './Presenter.svelte'
  import AudienceWindow from './AudienceWindow.svelte'
  import RecoveryPrompt from './RecoveryPrompt.svelte'
  import ChartEditor from './ChartEditor.svelte'
  import RichNotesEditor from './RichNotesEditor.svelte'
  import FindReplace from './FindReplace.svelte'
  import ShortcutsDialog from './ShortcutsDialog.svelte'
  import VersionHistory from './VersionHistory.svelte'
  import TemplatePicker from './TemplatePicker.svelte'
  import SaveChangesDialog from './SaveChangesDialog.svelte'
  import { confirmDocumentTransition, documentFileName } from './lib/documentLifecycle.js'
  import Comments from './Comments.svelte'
  import AccessibilityPanel from './AccessibilityPanel.svelte'
  import AnimationPane from './AnimationPane.svelte'
  import { fitCanvasScale } from './lib/canvasScale.js'
  import { resetThumbnailCache, thumbnailDeckRenderKey } from './lib/thumbnailCache.js'
  import type {
    AccessibilityReportDto,
    ChartDataDto,
    ChartShapeSnapshot,
    ChartTypeDto,
    ColorDto,
    CommentAnchorDto,
    DeckSnapshot,
    GeometricShapeSnapshot,
    HeadingLevelDto,
    ImageShapeSnapshot,
    ParagraphDto,
    ParagraphStyleDto,
    PassthroughSnapshot,
    PlaceholderDefDto,
    RecoverySnapshot,
    RectDto,
    RunDto,
    ShapeSnapshot,
    SlideSectionDto,
    SlideSizeDto,
    SlideSnapshot,
    StyleDto,
    TableShapeSnapshot,
    TemplateInfoDto,
    TextBoxSnapshot,
    TransformDto,
    TransitionKindDto,
    VerticalAlignDto,
    WarningDto,
  } from './lib/types'

  /** True if this window is the presenter control view. */
  const isPresenter = window.location.hash === '#/presenter'
  /** True if this window is the fullscreen audience view. */
  const isAudience = window.location.hash === '#/audience'

  let deck = $state<DeckSnapshot | null>(null)
  let documentStatus = $state<{ path: string | null; isDirty: boolean }>({ path: null, isDirty: false })
  let documentBusy = $state(false)
  let canvasEditor = $state<{ flushEdits: () => Promise<void> } | undefined>()
  let notesEditor = $state<{ flushEdits: () => Promise<void> } | undefined>()
  let canvasDraftDirty = $state(false)
  let notesDraftDirty = $state(false)
  let savePrompt = $state<{ action: string; resolve: (choice: 'save' | 'discard' | 'cancel') => void } | null>(null)
  let showInspector = $state(true)
  let statusRequest = 0
  let pendingDraftCommit: Promise<void> = Promise.resolve()
  const documentName = $derived(documentFileName(documentStatus.path))
  const hasUnsavedChanges = $derived(documentStatus.isDirty || canvasDraftDirty || notesDraftDirty)
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
  /** Active right-panel tab. The inspector stays contextual to the selection. */
  let rightPanelTab = $state<'style' | 'text' | 'arrange' | 'notes' | 'animation'>('style')
  /** Duration (ms) for the slide transition. */
  let transitionDuration = $state(500)
  /** Whether the find/replace dialog is open. */
  let showFindReplace = $state(false)
  /** Whether the find dialog opens focused on the replace field. */
  let findReplaceMode = $state<'find' | 'replace'>('find')
  /** Whether the shortcuts dialog is open. */
  let showShortcuts = $state(false)
  /** Whether the version-history panel is open. */
  let showVersionHistory = $state(false)
  /** Whether the comments sidebar is open. */
  let showComments = $state(false)
  /** Whether the accessibility checker panel is open. */
  let showAccessibility = $state(false)
  /** Latest document accessibility report, or null while loading/none. */
  let accessibility = $state<AccessibilityReportDto | null>(null)
  /** True while an accessibility check is running. */
  let checkingAccessibility = $state(false)
  /** Index of a shape to highlight on the active slide (driven by the
   *  accessibility panel); null draws no selection ring. */
  let a11ySelectedShapeIndex = $state<number | null>(null)
  /** Index of the shape the user has selected on the canvas, or null. */
  let selectedShapeIndex = $state<number | null>(null)
  /** Index of a text box currently in text-edit mode, or null. */
  let editingShapeIndex = $state<number | null>(null)
  /** Pending comment anchor invoked from a context menu, or null. */
  let commentDraft = $state<CommentAnchorDto | null>(null)
  /** Open "Add comment" shape context menu, or null. */
  let shapeMenu = $state<{ shapeId: string; x: number; y: number } | null>(null)
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
  /** Human-readable failure for editor actions. Never leave a command failure in
   *  the developer console as the only feedback path. */
  let actionError = $state('')
  /** Active text-box paragraph the cursor is in, so the toolbar can show and
   *  edit paragraph-level options (e.g. code-step ranges) for it. */
  let activeTextTarget = $state<{
    shapeIndex: number
    paragraphIndex: number
    style: ParagraphStyleDto
  } | null>(null)
  /** Bound step-ranges input, so committing the value does not hide it. */
  let codeStepsInput = $state<HTMLInputElement | null>(null)
  /** Whether the shape picker flyout is open. */
  let showShapeFlyout = $state(false)
  /** Whether "Text Box" creation mode is armed (next canvas click places one). */
  let creatingTextBox = $state(false)
  /** Selected geometric primitive awaiting a click or drag on the canvas. */
  let creatingShapeKind = $state<string | null>(null)
  /** The editor canvas-area element, used to map clicks to slide coordinates. */
  let canvasAreaEl = $state<HTMLElement | null>(null)
  let canvasViewport = $state({ width: 0, height: 0 })

  /** Maximum grid dimension offered by the table size picker. */
  const PICKER_MAX = 6
  /** Chart types offered by the chart toolbar dropdown. */
  const CHART_TYPES: ChartTypeDto[] = ['bar', 'column', 'line', 'area', 'pie', 'scatter']
  /** Slide transition kinds offered by the transition picker. */
  const TRANSITION_KINDS: TransitionKindDto[] = ['none', 'fade', 'slide', 'push', 'wipe', 'morph']
  /** Row/column indices for the picker grid. */
  const pickerIndices = [...Array(PICKER_MAX).keys()]

  const hasActiveCell = $derived(activeCell !== null)

  const activeSlide = $derived<SlideSnapshot | null>(deck?.slides[activeIndex] ?? null)
  const notes = $derived(activeSlide?.notes ?? '')
  const activeTransitionKind = $derived<TransitionKindDto>(activeSlide?.transition?.kind ?? 'none')
  const activeTransitionDurationMs = $derived<number>(activeSlide?.transition?.durationMs ?? 500)

  /** Deck slide size (aspect ratio), when set. */
  const slideSize = $derived<SlideSizeDto | undefined>(deck?.slideSize)
  const thumbnailDeckKey = $derived.by(() =>
    deck
      ? thumbnailDeckRenderKey(
          deck.theme,
          deck.template,
          deck.layouts,
          deck.master,
          deck.slideSize,
          deck.mediaRevision,
          deck.id,
        )
      : '',
  )
  const thumbnailAspectRatio = $derived(
    `${deck?.slideSize?.widthEmu ?? 12_192_000} / ${deck?.slideSize?.heightEmu ?? 6_858_000}`,
  )
  /** Default zoom fits the whole slide between the fixed side panels. */
  const canvasScale = $derived(
    fitCanvasScale(
      canvasViewport.width,
      canvasViewport.height,
      slideSize?.widthEmu ?? 12_192_000,
      slideSize?.heightEmu ?? 6_858_000,
    ),
  )
  /** Whether the deck theme is in high-contrast mode. */
  const highContrast = $derived<boolean>(deck?.theme.highContrast ?? false)
  /** Shape index to render with the selection ring: the user's canvas
   *  selection, falling back to the accessibility panel's highlight. */
  const canvasSelectedShapeIndex = $derived<number | null>(
    selectedShapeIndex ?? a11ySelectedShapeIndex,
  )
  /** Rich-text notes for the active slide, when present. */
  const activeRichNotes = $derived<ParagraphDto[] | undefined>(activeSlide?.richNotes)
  /** Selected geometric shape, if the current selection supports visual styling. */
  const selectedGeometric = $derived.by<GeometricShapeSnapshot | null>(() => {
    if (selectedShapeIndex === null || !activeSlide) return null
    const shape = activeSlide.shapes[selectedShapeIndex]
    return shape?.kind === 'geometric' ? (shape.value as GeometricShapeSnapshot) : null
  })
  /** Frame of any selected, placeable shape for the Arrange inspector. */
  const selectedShapeFrame = $derived.by<RectDto | null>(() => {
    if (selectedShapeIndex === null || !activeSlide) return null
    const shape = activeSlide.shapes[selectedShapeIndex]
    return shape ? shapeFrameOf(shape) ?? null : null
  })

  /** Layouts available on the current deck (from its template). */
  const deckLayouts = $derived(deck?.layouts ?? [])
  /** Name of the layout the active slide uses, or '' when none. */
  const activeLayoutRef = $derived<string>(activeSlide?.layoutRef ?? '')

  /** Effective placeholder frames for the active slide's layout: the deck
   *  master's placeholders overridden by the selected layout's overrides
   *  (matched by name). Empty when no layout is selected, so no guides render. */
  const activeLayoutPlaceholders = $derived.by<PlaceholderDefDto[]>(() => {
    const layoutName = activeSlide?.layoutRef
    if (!layoutName) return []
    const masterPlaceholders = deck?.master.placeholders ?? []
    const layout = (deck?.layouts ?? []).find((l) => l.name === layoutName)
    if (!layout) return masterPlaceholders
    const byName = new Map<string, PlaceholderDefDto>()
    for (const p of masterPlaceholders) byName.set(p.name, p)
    for (const p of layout.placeholders) byName.set(p.name, p)
    return Array.from(byName.values())
  })

  // Recompute the default fit scale when the available center pane changes
  // (window resize, sidebar changes, or the first render).
  $effect(() => {
    const element = canvasAreaEl
    if (!element) return
    const update = () => {
      canvasViewport = { width: element.clientWidth, height: element.clientHeight }
    }
    update()
    const observer = new ResizeObserver(update)
    observer.observe(element)
    return () => observer.disconnect()
  })

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
      reportActionError('Could not load the presentation', err)
    }
  }

  async function refreshDocumentStatus(): Promise<void> {
    const request = ++statusRequest
    const status = await invoke<typeof documentStatus>('get_document_status')
    if (request === statusRequest) documentStatus = status
  }

  $effect(() => {
    void deck
    if (!isPresenter && !isAudience) {
      void refreshDocumentStatus().catch((error) => reportActionError('Could not check save status', error))
    }
  })

  async function flushEditorDrafts(): Promise<void> {
    await canvasEditor?.flushEdits()
    await notesEditor?.flushEdits()
  }

  function commitEditorDraft(task: () => Promise<void>): Promise<void> {
    const next = pendingDraftCommit.then(task)
    pendingDraftCommit = next.catch(() => {})
    return next
  }

  async function selectInspectorTab(tab: typeof rightPanelTab): Promise<void> {
    try {
      await notesEditor?.flushEdits()
      rightPanelTab = tab
    } catch (error) { reportActionError('Could not save the notes edit', error) }
  }

  async function openVersionHistory(): Promise<void> {
    try {
      await flushEditorDrafts()
      showVersionHistory = true
    } catch (error) { reportActionError('Could not save the current edits', error) }
  }

  async function usePlainNotes(): Promise<void> {
    try {
      await notesEditor?.flushEdits()
      await handleSetRichNotes(null)
    } catch (error) { reportActionError('Could not change the notes format', error) }
  }

  async function addStarterTitle(): Promise<void> {
    if (!activeSlide) return
    try {
      const width = slideSize?.widthEmu ?? 12_192_000
      const height = slideSize?.heightEmu ?? 6_858_000
      deck = await invoke<DeckSnapshot>('add_text_box', {
        slideId: activeSlide.id, asTitle: true, x: width * .1, y: height * .18,
        width: width * .8, height: height * .25,
      })
      selectedShapeIndex = (deck.slides[activeIndex]?.shapes.length ?? 1) - 1
      editingShapeIndex = selectedShapeIndex
      await tick()
      canvasAreaEl?.querySelector<HTMLTextAreaElement>('textarea.text-box')?.focus()
    } catch (error) { reportActionError('Could not add a title', error) }
  }

  function chooseSaveAction(choice: 'save' | 'discard' | 'cancel'): void {
    const prompt = savePrompt
    savePrompt = null
    prompt?.resolve(choice)
  }

  async function confirmTransition(action: string): Promise<'continue' | 'discard' | 'cancel'> {
    return confirmDocumentTransition({
      flush: flushEditorDrafts,
      isDirty: async () => { await refreshDocumentStatus(); return documentStatus.isDirty },
      recover: () => invoke<void>('flush_recovery'),
      decide: () => new Promise((resolve) => { savePrompt = { action, resolve } }),
      save: () => saveDocument(false),
    })
  }

  function resetDocumentView(snapshot: DeckSnapshot): void {
    if (deck?.id !== snapshot.id) resetThumbnailCache()
    deck = snapshot
    activeIndex = 0
    warnings = snapshot.warnings
    showWarnings = true
    showRecovery = false
    selectedShapeIndex = null
    editingShapeIndex = null
    activeCell = null
    activeChart = null
    canvasDraftDirty = false
    notesDraftDirty = false
  }

  /** Creates a new blank deck from the Rust model, optionally applying a template. */
  async function newDeck(templateName?: string | null): Promise<void> {
    if (documentBusy) return
    documentBusy = true
    try {
      const decision = await confirmTransition('creating another presentation')
      if (decision === 'cancel') return
      resetDocumentView(await invoke<DeckSnapshot>('new_deck', { templateName, discardChanges: decision === 'discard' }))
      await refreshDocumentStatus()
    } catch (error) {
      reportActionError('Could not create a new presentation', error)
    } finally {
      documentBusy = false
    }
  }

  /** Loads built-in templates and opens the template picker. */
  async function openTemplatePicker(): Promise<void> {
    if (documentBusy) return
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
    if (documentBusy) return
    documentBusy = true
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: 'Presentation', extensions: ['pptx'] }],
      })
      if (typeof path !== 'string') return
      const decision = await confirmTransition('opening another presentation')
      if (decision === 'cancel') return
      resetDocumentView(await invoke<DeckSnapshot>('open_deck', { path, discardChanges: decision === 'discard' }))
      await refreshDocumentStatus()
    } catch (error) {
      reportActionError('Could not open the presentation', error)
    } finally {
      documentBusy = false
    }
  }

  /** Save reuses the current file; Save As always chooses a new destination. */
  async function saveDocument(saveAs: boolean): Promise<boolean> {
    await flushEditorDrafts()
    await refreshDocumentStatus()
    let path = documentStatus.path
    if (saveAs || !path) {
      path = await save({
        defaultPath: path ?? 'Untitled presentation.pptx',
        filters: [{ name: 'PowerPoint presentation', extensions: ['pptx'] }],
      })
      if (!path) return false
    }
    const status = await invoke<typeof documentStatus>('save_deck', { path })
    statusRequest += 1
    documentStatus = status
    actionError = ''
    return true
  }

  async function onSave(saveAs = false): Promise<void> {
    if (documentBusy || !deck) return
    documentBusy = true
    try {
      await saveDocument(saveAs)
    } catch (error) {
      reportActionError('Could not save the presentation', error)
    } finally {
      documentBusy = false
    }
  }

  async function requestClose(): Promise<void> {
    if (documentBusy) return
    documentBusy = true
    try {
      const decision = await confirmTransition('closing this presentation')
      if (decision !== 'cancel') await invoke('complete_close', { discardChanges: decision === 'discard' })
    } catch (error) {
      reportActionError('Could not close the presentation safely', error)
    } finally {
      documentBusy = false
    }
  }

  /** Filename stem used as the default for export save dialogs. */
  function exportStem(): string {
    return documentStatus.path ? documentName.replace(/\.pptx$/i, '') : 'presentation'
  }

  /** Guards an export: disables the menu, surfaces errors, clears the busy flag. */
  async function runExport(kind: 'svg' | 'png' | 'pdf', task: () => Promise<void>): Promise<void> {
    exporting = kind
    exportError = ''
    try {
      await flushEditorDrafts()
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
    const selectedSlideId = activeSlide?.id
    await flushEditorDrafts()
    deck = await invoke<DeckSnapshot>('undo')
    retainSlideSelection(selectedSlideId)
  }

  /** Adopts a deck restored from version history (reversible via Undo). */
  function onRestoreVersion(snapshot: DeckSnapshot): void {
    deck = snapshot
    activeIndex = Math.min(activeIndex, Math.max(snapshot.slides.length - 1, 0))
  }

  /** Adopts the deck snapshot returned by any comment command. */
  function handleCommentApplied(snapshot: DeckSnapshot): void {
    deck = snapshot
  }

  /** Opens the "Add comment" context menu for a right-clicked shape. */
  function handleShapeContextMenu(detail: {
    shapeId: string
    shapeIndex: number
    x: number
    y: number
  }): void {
    shapeMenu = { shapeId: detail.shapeId, x: detail.x, y: detail.y }
  }

  /** Creates a shape-anchored comment draft from the open context menu. */
  function addShapeComment(): void {
    if (!shapeMenu || !activeSlide) return
    commentDraft = { kind: 'shape', slideId: activeSlide.id, shapeId: shapeMenu.shapeId }
    shapeMenu = null
    showComments = true
  }

  /** Creates a text-range-anchored comment draft from a text selection. */
  function handleCommentOnSelection(detail: {
    shapeId: string
    shapeIndex: number
    start: number
    end: number
    x: number
    y: number
  }): void {
    if (!activeSlide) return
    commentDraft = {
      kind: 'text_range',
      slideId: activeSlide.id,
      shapeId: detail.shapeId,
      start: detail.start,
      end: detail.end,
    }
    showComments = true
  }

  /** Closes the comments sidebar and clears any pending draft. */
  function closeComments(): void {
    showComments = false
    commentDraft = null
  }

  /** Runs the document accessibility checks on the current deck. */
  async function runAccessibility(): Promise<void> {
    checkingAccessibility = true
    try {
      accessibility = await invoke<AccessibilityReportDto>('check_accessibility')
    } catch (err) {
      console.error('Accessibility check failed:', err)
      accessibility = null
    } finally {
      checkingAccessibility = false
    }
  }

  /** Opens the accessibility panel and runs the first check. */
  function openAccessibility(): void {
    showAccessibility = true
    void runAccessibility()
  }

  /** Navigates to an issue's slide and highlights its shape, when applicable. */
  function navigateToIssue(slideId: string | null, shapeIndex: number | null): void {
    if (slideId && deck) {
      const idx = deck.slides.findIndex((s) => s.id === slideId)
      if (idx >= 0) activeIndex = idx
    }
    a11ySelectedShapeIndex = shapeIndex
    selectedShapeIndex = shapeIndex
    editingShapeIndex = null
  }

  // Keep the accessibility report fresh while the panel is open: re-run the
  // offline check shortly after any deck edit (coalesces rapid keystrokes).
  $effect(() => {
    void deck
    if (!showAccessibility) return
    const timer = setTimeout(() => void runAccessibility(), 300)
    return () => clearTimeout(timer)
  })

  /** Opens the presenter window. */
  async function onStartPresenter(): Promise<void> {
    await flushEditorDrafts()
    await invoke('start_presenter')
  }

  /** Sends a text-box edit command to Rust and re-renders from the snapshot. */
  async function handleTextEdit(detail: {
    slideId: string
    shapeIndex: number
    paragraphs: ParagraphDto[]
  }): Promise<void> {
    try {
      await commitEditorDraft(async () => {
        deck = await invoke<DeckSnapshot>('edit_text_box', {
          slideId: detail.slideId,
          shapeIndex: detail.shapeIndex,
          paragraphs: detail.paragraphs,
        })
      })
    } catch (error) {
      reportActionError('Could not save the text edit', error)
      throw error
    }
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

  /** Applies a run-level boolean flag. When a text box is being edited, it
   *  toggles the active run; otherwise it applies to every run in the selected
   *  text box. */
  async function toggleRunFlag(flag: 'bold' | 'italic' | 'underline' | 'strikethrough' | 'code'): Promise<void> {
    await flushEditorDrafts()
    const target = getTextTarget()
    if (target) {
      const value = !target.run[flag]
      deck = await invoke<DeckSnapshot>('set_run_style', {
        slideId: target.slideId,
        shapeIndex: target.shapeIndex,
        paragraphIndex: target.paragraphIndex,
        runIndex: target.runIndex,
        [flag]: value,
      })
      return
    }
    await applyRunFlagToAll((run) => ({ ...run, [flag]: !allRunsHave(run, flag) }))
  }

  /** Returns true when every run in the selected text box already has `flag`
   *  set; used to decide whether applying to all should turn the flag on/off. */
  function allRunsHave(run: RunDto, flag: 'bold' | 'italic' | 'underline' | 'strikethrough' | 'code'): boolean {
    if (!activeSlide || selectedShapeIndex === null) return run[flag]
    const shape = activeSlide.shapes[selectedShapeIndex]
    if (!shape || shape.kind !== 'text_box') return run[flag]
    const textBox = shape.value as TextBoxSnapshot
    const runs = textBox.paragraphs.flatMap((p) => p.runs)
    return runs.length > 0 && runs.every((r) => r[flag])
  }

  /** Applies a per-run transform to every run of the selected text box and
   *  commits it as a single text-box edit. */
  async function applyRunFlagToAll(
    transform: (run: RunDto) => RunDto,
  ): Promise<void> {
    if (selectedShapeIndex === null || !activeSlide) return
    const shape = activeSlide.shapes[selectedShapeIndex]
    if (!shape || shape.kind !== 'text_box') return
    const textBox = shape.value as TextBoxSnapshot
    if (textBox.paragraphs.every((p) => p.runs.length === 0)) return
    const paragraphs = textBox.paragraphs.map((p) => ({
      ...p,
      runs: p.runs.map(transform),
    }))
    deck = await invoke<DeckSnapshot>('edit_text_box', {
      slideId: activeSlide.id,
      shapeIndex: selectedShapeIndex,
      paragraphs,
    })
  }

  /** Toggles superscript on the active run, or across the selected text box. */
  async function toggleSuperscript(): Promise<void> {
    await flushEditorDrafts()
    const target = getTextTarget()
    if (target) {
      const value: VerticalAlignDto =
        target.run.verticalAlign === 'superscript' ? 'baseline' : 'superscript'
      deck = await invoke<DeckSnapshot>('set_run_style', {
        slideId: target.slideId,
        shapeIndex: target.shapeIndex,
        paragraphIndex: target.paragraphIndex,
        runIndex: target.runIndex,
        verticalAlign: value,
      })
      return
    }
    await applyVerticalAlignToAll('superscript')
  }

  /** Toggles subscript on the active run, or across the selected text box. */
  async function toggleSubscript(): Promise<void> {
    await flushEditorDrafts()
    const target = getTextTarget()
    if (target) {
      const value: VerticalAlignDto =
        target.run.verticalAlign === 'subscript' ? 'baseline' : 'subscript'
      deck = await invoke<DeckSnapshot>('set_run_style', {
        slideId: target.slideId,
        shapeIndex: target.shapeIndex,
        paragraphIndex: target.paragraphIndex,
        runIndex: target.runIndex,
        verticalAlign: value,
      })
      return
    }
    await applyVerticalAlignToAll('subscript')
  }

  /** Applies a vertical-align toggle to every run of the selected text box. */
  async function applyVerticalAlignToAll(kind: 'superscript' | 'subscript'): Promise<void> {
    if (selectedShapeIndex === null || !activeSlide) return
    const shape = activeSlide.shapes[selectedShapeIndex]
    if (!shape || shape.kind !== 'text_box') return
    const textBox = shape.value as TextBoxSnapshot
    const runs = textBox.paragraphs.flatMap((p) => p.runs)
    if (runs.length === 0) return
    const value: VerticalAlignDto = runs.every((r) => r.verticalAlign === kind)
      ? 'baseline'
      : kind
    await applyRunFlagToAll((run) => ({ ...run, verticalAlign: value }))
  }

  /** Applies a heading level to the active paragraph. */
  function getParagraphTarget() {
    const focused = getTextTarget()
    if (focused) return focused
    if (!activeSlide || selectedShapeIndex === null) return null
    const shape = activeSlide.shapes[selectedShapeIndex]
    if (shape?.kind !== 'text_box') return null
    const paragraphIndex = activeTextTarget?.shapeIndex === selectedShapeIndex
      ? activeTextTarget.paragraphIndex : 0
    const paragraph = (shape.value as TextBoxSnapshot).paragraphs[paragraphIndex]
    return paragraph ? { slideId: activeSlide.id, shapeIndex: selectedShapeIndex, paragraphIndex, paragraph } : null
  }

  async function setHeading(level: HeadingLevelDto | null): Promise<void> {
    await flushEditorDrafts()
    const target = getParagraphTarget()
    if (!target) return
    const style = {
      ...target.paragraph.style,
      heading: level ?? undefined,
    }
    deck = await invoke<DeckSnapshot>('set_paragraph_style', {
      slideId: target.slideId,
      shapeIndex: target.shapeIndex,
      paragraphIndex: target.paragraphIndex,
      style,
    })
  }

  /** Toggles a paragraph-level boolean flag. */
  async function toggleParagraphFlag(flag: 'blockquote' | 'codeBlock'): Promise<void> {
    await flushEditorDrafts()
    const target = getParagraphTarget()
    if (!target) return
    const style = {
      ...target.paragraph.style,
      [flag]: !target.paragraph.style[flag],
    }
    deck = await invoke<DeckSnapshot>('set_paragraph_style', {
      slideId: target.slideId,
      shapeIndex: target.shapeIndex,
      paragraphIndex: target.paragraphIndex,
      style,
    })
  }

  /** Commits the typed stepped-code ranges (e.g. "1-3|4|5,7") to the active
   *  code-block paragraph. An empty value clears the ranges. */
  async function onCodeStepRangesChange(event: Event): Promise<void> {
    await flushEditorDrafts()
    if (!activeTextTarget || !activeSlide) return
    const value = (event.target as HTMLInputElement).value
    const style: ParagraphStyleDto = {
      ...activeTextTarget.style,
      codeStepRanges: value.trim() === '' ? undefined : value,
    }
    deck = await invoke<DeckSnapshot>('set_paragraph_style', {
      slideId: activeSlide.id,
      shapeIndex: activeTextTarget.shapeIndex,
      paragraphIndex: activeTextTarget.paragraphIndex,
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
      slideId: activeSlide.id,
      bytes,
    })
  }

  /** Arms geometric creation mode; the next canvas click or drag places it. */
  async function onAddShape(geometryKind: string): Promise<void> {
    if (!activeSlide) return
    showShapeFlyout = false
    creatingTextBox = false
    creatingShapeKind = geometryKind
  }

  /** Creates the armed geometric shape at the canvas frame, if one is armed. */
  async function onPlaceShape(placement: { frame: RectDto; rotation?: number } | null): Promise<void> {
    if (!activeSlide || !creatingShapeKind) return
    const geometryKind = creatingShapeKind
    creatingShapeKind = null
    try {
      deck = await invoke<DeckSnapshot>('add_shape', {
        slideId: activeSlide.id,
        geometryKind,
        ...(placement ? { frame: placement.frame, rotation: placement.rotation } : {}),
      })
      const newIndex = (deck?.slides[activeIndex]?.shapes.length ?? 1) - 1
      selectedShapeIndex = newIndex
    } catch (e) {
      reportActionError('Could not add the shape', e)
    }
  }

  /** Returns the bounding frame (EMU) for any shape kind, or undefined. */
  function shapeFrameOf(shape: ShapeSnapshot): RectDto | undefined {
    switch (shape.kind) {
      case 'text_box':
        return (shape.value as TextBoxSnapshot).frame
      case 'image':
        return (shape.value as ImageShapeSnapshot).transform.frame
      case 'geometric':
        return (shape.value as GeometricShapeSnapshot).transform.frame
      case 'table':
        return (shape.value as TableShapeSnapshot).transform.frame
      case 'chart':
        return (shape.value as ChartShapeSnapshot).transform.frame
      case 'passthrough':
        return (shape.value as PassthroughSnapshot).frame
      default:
        return undefined
    }
  }

  /** Returns a shape's rotation in degrees (0 for text boxes). */
  function shapeRotationOf(shape: ShapeSnapshot): number {
    switch (shape.kind) {
      case 'image':
        return (shape.value as ImageShapeSnapshot).transform.rotation
      case 'geometric':
        return (shape.value as GeometricShapeSnapshot).transform.rotation
      case 'table':
        return (shape.value as TableShapeSnapshot).transform.rotation
      case 'chart':
        return (shape.value as ChartShapeSnapshot).transform.rotation
      default:
        return 0
    }
  }

  /** Sets the canvas selection; clears text-edit mode unless re-selecting the
   *  shape currently being edited. */
  function handleSelectShape(detail: { shapeIndex: number | null }): void {
    selectedShapeIndex = detail.shapeIndex
    if (detail.shapeIndex === null || detail.shapeIndex !== editingShapeIndex) {
      editingShapeIndex = null
    }
  }

  /** Enters (`index`) or leaves (`null`) text-edit mode for a text box. */
  function handleEditShape(detail: { shapeIndex: number | null }): void {
    editingShapeIndex = detail.shapeIndex
    if (detail.shapeIndex !== null) selectedShapeIndex = detail.shapeIndex
  }

  /** Commits a shape's new transform (after a drag or resize). */
  async function handleUpdateShapeTransform(detail: {
    shapeIndex: number
    transform: TransformDto
  }): Promise<void> {
    if (!activeSlide) return
    try {
      deck = await invoke<DeckSnapshot>('update_shape_transform', {
        slideId: activeSlide.id,
        shapeIndex: detail.shapeIndex,
        transform: detail.transform,
      })
    } catch (error) {
      reportActionError('Could not update the selected object', error)
    }
  }

  /** Provides clear, local feedback for a desktop command failure. */
  function reportActionError(action: string, error: unknown): void {
    const detail = error instanceof Error ? error.message : String(error)
    actionError = `${action}. ${detail}`
  }

  /** Converts a native colour-input value to the DTO used at the Rust boundary. */
  function colorFromHex(value: string, alpha = 255): ColorDto {
    const hex = value.replace('#', '')
    const parsed = Number.parseInt(hex, 16)
    return {
      r: Number.isFinite(parsed) ? (parsed >> 16) & 0xff : 0,
      g: Number.isFinite(parsed) ? (parsed >> 8) & 0xff : 0,
      b: Number.isFinite(parsed) ? parsed & 0xff : 0,
      a: alpha,
    }
  }

  /** Formats a DTO colour for a native colour input. */
  function colorToHex(color: ColorDto): string {
    return `#${[color.r, color.g, color.b]
      .map((channel) => channel.toString(16).padStart(2, '0'))
      .join('')}`
  }

  /** Applies a style update only when a geometric shape is selected. */
  async function updateSelectedGeometryStyle(
    update: (style: StyleDto) => StyleDto,
  ): Promise<void> {
    if (selectedShapeIndex === null || !activeSlide || !selectedGeometric) return
    try {
      deck = await invoke<DeckSnapshot>('update_shape_style', {
        slideId: activeSlide.id,
        shapeIndex: selectedShapeIndex,
        style: update(selectedGeometric.style),
      })
    } catch (error) {
      reportActionError('Could not update the shape style', error)
    }
  }

  /** Updates one position or size field through the existing transform command. */
  function updateSelectedFrame(
    field: 'x' | 'y' | 'width' | 'height',
    value: number,
  ): void {
    if (!selectedShapeFrame || selectedShapeIndex === null || !activeSlide || !Number.isFinite(value)) {
      return
    }
    const frame = { ...selectedShapeFrame, [field]: Math.max(0, value) }
    if (field === 'width' || field === 'height') frame[field] = Math.max(76_200, value)
    const shape = activeSlide.shapes[selectedShapeIndex]
    if (!shape) return
    void handleUpdateShapeTransform({
      shapeIndex: selectedShapeIndex,
      transform: { frame, rotation: shapeRotationOf(shape) },
    })
  }

  /** Nudges the selected shape by the given EMU delta. */
  const EMU_PER_CSS_PIXEL = 9_525

  async function nudgeSelected(dxEmu: number, dyEmu: number): Promise<void> {
    if (selectedShapeIndex === null || !activeSlide) return
    const shape = activeSlide.shapes[selectedShapeIndex]
    if (!shape) return
    const frame = shapeFrameOf(shape)
    if (!frame) return
    await handleUpdateShapeTransform({
      shapeIndex: selectedShapeIndex,
      transform: {
        frame: { x: frame.x + dxEmu, y: frame.y + dyEmu, width: frame.width, height: frame.height },
        rotation: shapeRotationOf(shape),
      },
    })
  }

  /** Deletes the currently selected shape. */
  async function deleteSelectedShape(): Promise<void> {
    await flushEditorDrafts()
    if (selectedShapeIndex === null || !activeSlide) return
    deck = await invoke<DeckSnapshot>('delete_shape', {
      slideId: activeSlide.id,
      shapeIndex: selectedShapeIndex,
    })
    selectedShapeIndex = null
    editingShapeIndex = null
  }

  /** Re-applies the most recently undone edit. */
  async function onRedo(): Promise<void> {
    const selectedSlideId = activeSlide?.id
    await flushEditorDrafts()
    deck = await invoke<DeckSnapshot>('redo')
    retainSlideSelection(selectedSlideId)
  }

  /** Keep the same slide selected when undo or reordering changes its index. */
  function retainSlideSelection(slideId?: string): void {
    const index = deck?.slides.findIndex((slide) => slide.id === slideId) ?? -1
    activeIndex = index >= 0 ? index : Math.max(0, Math.min(activeIndex, (deck?.slides.length ?? 1) - 1))
    // Reordering across a collapsed section must not hide the active slide.
    const starts = (deck?.sections ?? [])
      .map((section) => ({ id: section.startSlideId, index: deck!.slides.findIndex((slide) => slide.id === section.startSlideId) }))
      .filter((section) => section.index >= 0 && section.index <= activeIndex)
      .sort((a, b) => b.index - a.index)
    const section = starts[0]
    if (section && collapsedSections.has(section.id)) {
      const expanded = new Set(collapsedSections)
      expanded.delete(section.id)
      collapsedSections = expanded
    }
    activeCell = null
    activeChart = null
    a11ySelectedShapeIndex = null
    if (!slideId || index < 0) {
      selectedShapeIndex = null
      editingShapeIndex = null
    }
  }

  async function moveActiveSlide(direction: -1 | 1): Promise<void> {
    if (documentBusy || !activeSlide || !deck) return
    const slideId = activeSlide.id
    const toIndex = activeIndex + direction
    if (toIndex < 0 || toIndex >= deck.slides.length) return
    documentBusy = true
    try {
      await flushEditorDrafts()
      deck = await invoke<DeckSnapshot>('move_slide', { slideId, toIndex })
      retainSlideSelection(slideId)
    } catch (error) {
      reportActionError('Could not move the slide', error)
    } finally {
      documentBusy = false
    }
  }

  async function deleteActiveSlide(): Promise<void> {
    if (documentBusy || !activeSlide || !deck || deck.slides.length <= 1) return
    const slideId = activeSlide.id
    documentBusy = true
    try {
      await flushEditorDrafts()
      deck = await invoke<DeckSnapshot>('delete_slide', { slideId })
      retainSlideSelection()
    } catch (error) {
      reportActionError('Could not delete the slide', error)
    } finally {
      documentBusy = false
    }
  }

  /** Inserts a new blank slide after the active one and selects it. */
  async function onNewSlide(): Promise<void> {
    if (!deck) return
    try {
      await flushEditorDrafts()
      deck = await invoke<DeckSnapshot>('new_slide', { afterIndex: activeIndex })
      activeIndex = Math.min(activeIndex + 1, (deck?.slides.length ?? 1) - 1)
      a11ySelectedShapeIndex = null
      selectedShapeIndex = null
      editingShapeIndex = null
    } catch (error) {
      reportActionError('Could not create a new slide', error)
    }
  }

  /** Arms text-box creation mode: the next click on the canvas places a box. */
  function enterCreateTextBox(): void {
    creatingShapeKind = null
    creatingTextBox = true
  }

  /** Cancels text-box creation mode without placing a box. */
  function cancelCreateTextBox(): void {
    creatingTextBox = false
  }

  /** Default text-box size (EMU) for click-to-create placement. */
  const TEXT_BOX_DEFAULT_W_EMU = 2_743_200
  const TEXT_BOX_DEFAULT_H_EMU = 1_371_600

  /** Maps a pointer event on the canvas-area to a top-left EMU frame for a new
   *  text box, centered on the click and clamped to the slide bounds. Returns
   *  null when the click landed outside the rendered slide. */
  function textBoxFrameFromClick(event: MouseEvent): { x: number; y: number; w: number; h: number } | null {
    const canvas = canvasAreaEl?.querySelector<HTMLElement>('.canvas')
    if (!canvas) return null
    const rect = canvas.getBoundingClientRect()
    const insideX = event.clientX >= rect.left && event.clientX <= rect.right
    const insideY = event.clientY >= rect.top && event.clientY <= rect.bottom
    if (!insideX || !insideY) return null
    const slideW = slideSize?.widthEmu ?? 12_192_000
    const slideH = slideSize?.heightEmu ?? 6_858_000
    const relX = (event.clientX - rect.left) / rect.width
    const relY = (event.clientY - rect.top) / rect.height
    const cx = relX * slideW
    const cy = relY * slideH
    const w = TEXT_BOX_DEFAULT_W_EMU
    const h = TEXT_BOX_DEFAULT_H_EMU
    const x = Math.min(Math.max(cx - w / 2, 0), Math.max(slideW - w, 0))
    const y = Math.min(Math.max(cy - h / 2, 0), Math.max(slideH - h, 0))
    return { x, y, w, h }
  }

  /** Places a new text box at the click position, selects it, and enters
   *  edit mode so the user can start typing immediately. */
  async function onCanvasClickCreateTextBox(event: MouseEvent): Promise<void> {
    const frame = textBoxFrameFromClick(event)
    creatingTextBox = false
    if (!frame || !activeSlide) return
    try {
      deck = await invoke<DeckSnapshot>('add_text_box', {
        slideId: activeSlide.id,
        x: frame.x,
        y: frame.y,
        width: frame.w,
        height: frame.h,
      })
      const newIndex = (deck?.slides[activeIndex]?.shapes.length ?? 0) - 1
      selectedShapeIndex = newIndex
      editingShapeIndex = newIndex
      await tick()
      const ta = document.querySelector<HTMLTextAreaElement>(
        `textarea.text-box[data-shape-index="${newIndex}"]`,
      )
      ta?.focus()
    } catch (e) {
      reportActionError('Could not create the text box', e)
    }
  }

  /** Dispatches a native menu-event id to the matching editor action. */
  function handleMenuEvent(id: string): void {
    if (documentBusy) return
    switch (id) {
      case 'menu_new':
        void openTemplatePicker()
        break
      case 'menu_open':
        void onOpen()
        break
      case 'menu_save':
        void onSave()
        break
      case 'menu_save_as':
        void onSave(true)
        break
      case 'menu_undo':
        void onUndo()
        break
      case 'menu_redo':
        void onRedo()
        break
      case 'menu_new_slide':
        void onNewSlide()
        break
      case 'menu_text_box':
        enterCreateTextBox()
        break
      case 'menu_image':
        onInsertImage()
        break
      case 'menu_shape':
        showShapeFlyout = true
        break
      case 'menu_table':
        showTablePicker = true
        break
      case 'menu_chart':
        showChartDropdown = true
        break
      case 'menu_comment':
        if (deck) showComments = !showComments
        break
      case 'menu_bold':
        void toggleRunFlag('bold')
        break
      case 'menu_italic':
        void toggleRunFlag('italic')
        break
      case 'menu_underline':
        void toggleRunFlag('underline')
        break
      case 'menu_present':
        void onStartPresenter()
        break
      case 'menu_export_svg':
        void onExportSvg()
        break
      case 'menu_export_png':
        void onExportPng()
        break
      case 'menu_export_pdf':
        void onExportPdf()
        break
      case 'menu_find':
        findReplaceMode = 'find'
        showFindReplace = true
        break
      case 'menu_find_replace':
        findReplaceMode = 'replace'
        showFindReplace = true
        break
      case 'menu_shortcuts':
        showShortcuts = true
        break
      default:
        break
    }
  }

  // Listen for native menu actions emitted by the Tauri backend.
  $effect(() => {
    if (isPresenter || isAudience) return
    let unlisten: (() => void) | undefined
    let cancelled = false
    void listen<string>('menu-event', (event) => handleMenuEvent(event.payload)).then((fn) => {
      if (cancelled) fn()
      else unlisten = fn
    })
    return () => {
      cancelled = true
      unlisten?.()
    }
  })

  $effect(() => {
    if (isPresenter || isAudience) return
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen('document-close-requested', () => void requestClose()).then((stop) => {
      if (disposed) stop()
      else unlisten = stop
    })
    return () => { disposed = true; unlisten?.() }
  })

  /** Appends a new `rows` x `cols` table to the active slide. */
  async function onAddTable(rows: number, cols: number): Promise<void> {
    if (!activeSlide) return
    showTablePicker = false
    deck = await invoke<DeckSnapshot>('add_table', {
      slideId: activeSlide.id,
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
    try {
      await commitEditorDraft(async () => {
        deck = await invoke<DeckSnapshot>('set_cell_text', detail)
      })
    } catch (error) {
      reportActionError('Could not save the table edit', error)
      throw error
    }
  }

  /** Records the focused cell so the row/column toolbar can target it. */
  function handleCellFocus(detail: { shapeIndex: number; row: number; col: number }): void {
    activeCell = detail
  }

  /** Inserts a row below the focused cell. */
  async function onInsertRow(): Promise<void> {
    await flushEditorDrafts()
    if (!activeSlide || !activeCell) return
    deck = await invoke<DeckSnapshot>('insert_row', {
      slideId: activeSlide.id,
      shapeIndex: activeCell.shapeIndex,
      index: activeCell.row + 1,
    })
  }

  /** Inserts a column to the right of the focused cell. */
  async function onInsertColumn(): Promise<void> {
    await flushEditorDrafts()
    if (!activeSlide || !activeCell) return
    deck = await invoke<DeckSnapshot>('insert_column', {
      slideId: activeSlide.id,
      shapeIndex: activeCell.shapeIndex,
      index: activeCell.col + 1,
    })
  }

  /** Deletes the focused cell's row. */
  async function onDeleteRow(): Promise<void> {
    await flushEditorDrafts()
    if (!activeSlide || !activeCell) return
    deck = await invoke<DeckSnapshot>('delete_row', {
      slideId: activeSlide.id,
      shapeIndex: activeCell.shapeIndex,
      index: activeCell.row,
    })
  }

  /** Deletes the focused cell's column. */
  async function onDeleteColumn(): Promise<void> {
    await flushEditorDrafts()
    if (!activeSlide || !activeCell) return
    deck = await invoke<DeckSnapshot>('delete_column', {
      slideId: activeSlide.id,
      shapeIndex: activeCell.shapeIndex,
      index: activeCell.col,
    })
  }

  /** Appends a chart of the given type to the active slide. */
  async function onAddChart(chartType: ChartTypeDto): Promise<void> {
    if (!activeSlide) return
    showChartDropdown = false
    deck = await invoke<DeckSnapshot>('add_chart', {
      slideId: activeSlide.id,
      chartType: chartType,
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
        slideId: detail.slideId,
        shapeIndex: detail.shapeIndex,
        data: detail.data,
      })

      const updated = activeSlide?.shapes[detail.shapeIndex]
      const updatedType =
        updated && updated.kind === 'chart' ? (updated.value as ChartShapeSnapshot).chartType : null
      if (updatedType && updatedType !== detail.chartType) {
        deck = await invoke<DeckSnapshot>('set_chart_type', {
          slideId: detail.slideId,
          shapeIndex: detail.shapeIndex,
          chartType: detail.chartType,
        })
      }

      deck = await invoke<DeckSnapshot>('set_chart_title', {
        slideId: detail.slideId,
        shapeIndex: detail.shapeIndex,
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
      slideId: activeSlide.id,
      kind: kind === 'none' ? null : kind,
      durationMs,
    })
  }

  /** Sets or clears the active slide's layout reference. Pass '' to clear. */
  async function onSetSlideLayout(layoutName: string): Promise<void> {
    if (!activeSlide) return
    deck = await invoke<DeckSnapshot>('set_slide_layout', {
      slideId: activeSlide.id,
      layoutName: layoutName === '' ? null : layoutName,
    })
  }

  /** Selects a different slide in the thumbnail panel. */
  async function selectSlide(index: number): Promise<void> {
    try { await flushEditorDrafts() } catch (error) {
      reportActionError('Could not save the current slide edits', error)
      return
    }
    activeIndex = index
    a11ySelectedShapeIndex = null
    selectedShapeIndex = null
    editingShapeIndex = null
  }

  // Drop a selection that no longer points at a valid shape (e.g. after an
  // undo/redo or an edit that changed the shape count).
  $effect(() => {
    const count = activeSlide?.shapes.length ?? 0
    if (selectedShapeIndex !== null && selectedShapeIndex >= count) {
      selectedShapeIndex = null
      editingShapeIndex = null
    }
  })

  /** Restores a recovery snapshot as the current deck. */
  async function handleRestore(id: string): Promise<void> {
    resetDocumentView(await invoke<DeckSnapshot>('restore_recovery', { id }))
    await refreshDocumentStatus()
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
    deck = await invoke<DeckSnapshot>('set_slide_size', { slideSize: slideSizePayload })
  }

  /** Toggles the deck's high-contrast accessibility theme. */
  async function toggleHighContrast(): Promise<void> {
    deck = await invoke<DeckSnapshot>('set_high_contrast', {
      highContrast: !highContrast,
    })
  }

  /** Commits rich-text notes (or clears them with null) for the active slide. */
  async function handleSetRichNotes(paragraphs: ParagraphDto[] | null, sourceSlideId?: string): Promise<void> {
    const slideId = sourceSlideId ?? activeSlide?.id
    if (!slideId) return
    try {
      await commitEditorDraft(async () => {
        deck = await invoke<DeckSnapshot>('set_rich_notes', { slideId, richNotes: paragraphs })
      })
    } catch (error) {
      reportActionError('Could not save the notes edit', error)
      throw error
    }
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
    if (documentBusy) return
    const mod = event.metaKey || event.ctrlKey
    const target = event.target as HTMLElement | null
    const typing =
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      (target?.isContentEditable ?? false)

    if ((creatingTextBox || creatingShapeKind) && event.key === 'Escape') {
      event.preventDefault()
      cancelCreateTextBox()
      creatingShapeKind = null
      return
    }

    if (mod && event.key.toLowerCase() === 'f') {
      event.preventDefault()
      findReplaceMode = event.altKey ? 'replace' : 'find'
      showFindReplace = true
    } else if (event.ctrlKey && !event.metaKey && event.key.toLowerCase() === 'h') {
      event.preventDefault()
      findReplaceMode = 'replace'
      showFindReplace = true
    } else if (!typing && event.key === '?') {
      event.preventDefault()
      showShortcuts = true
    } else if (!mod && !typing && (event.key === 'c' || event.key === 'C')) {
      event.preventDefault()
      showComments = !showComments
    } else if (!typing && selectedShapeIndex !== null) {
      // OOXML uses 9,525 EMU per CSS pixel. A 1/10 EMU nudge is invisible;
      // retain the familiar one-pixel and ten-pixel keyboard increments.
      const step = (event.shiftKey ? 10 : 1) * EMU_PER_CSS_PIXEL
      if (event.key === 'Delete' || event.key === 'Backspace') {
        event.preventDefault()
        void deleteSelectedShape()
      } else if (event.key === 'ArrowLeft') {
        event.preventDefault()
        void nudgeSelected(-step, 0)
      } else if (event.key === 'ArrowRight') {
        event.preventDefault()
        void nudgeSelected(step, 0)
      } else if (event.key === 'ArrowUp') {
        event.preventDefault()
        void nudgeSelected(0, -step)
      } else if (event.key === 'ArrowDown') {
        event.preventDefault()
        void nudgeSelected(0, step)
      }
    }
  }
</script>

{#if isPresenter}
  <Presenter />
{:else if isAudience}
  <AudienceWindow />
{:else}
  <div class="app">
    <header class="document-header">
      <span class="app-mark" aria-label="900Slides">900<span>Slides</span></span>
      <div class="document-heading">
        <strong title={documentStatus.path ?? documentName}>{documentName}</strong>
        <span class:unsaved={hasUnsavedChanges} role="status">{documentBusy ? 'Working…' : hasUnsavedChanges ? 'Unsaved changes' : documentStatus.path ? 'Saved to this computer' : 'New presentation · not saved yet'}</span>
      </div>
      <nav class="document-actions" aria-label="Document actions">
        <button type="button" onclick={openTemplatePicker} disabled={documentBusy}>New</button>
        <button type="button" onclick={onOpen} disabled={documentBusy}>Open</button>
        <button type="button" onclick={() => onSave(true)} disabled={documentBusy || !deck}>Save As</button>
        <button class="present-action" type="button" onclick={onStartPresenter} disabled={documentBusy || !deck}>Present</button>
      </nav>
    </header>
    <header class="toolbar" role="toolbar" aria-label="Editor toolbar" inert={documentBusy}>
      <!-- Left group: Undo | Redo | Save | separator | New Slide -->
      <span class="tb-group">
        <button class="tb-btn" onclick={onUndo} type="button" title="Undo (Cmd/Ctrl+Z)" aria-label="Undo">
          <svg viewBox="0 0 24 24"><path d="M9 14 4 9l5-5"></path><path d="M4 9h9a6 6 0 0 1 0 12H9"></path></svg>
        </button>
        <button class="tb-btn" onclick={onRedo} type="button" title="Redo (Cmd/Ctrl+Shift+Z)" aria-label="Redo">
          <svg viewBox="0 0 24 24"><path d="M15 14l5-5-5-5"></path><path d="M20 9h-9a6 6 0 0 0 0 12h4"></path></svg>
        </button>
        <button class="tb-btn" onclick={() => onSave()} type="button" disabled={!deck} title="Save (Cmd/Ctrl+S)" aria-label="Save">
          <svg viewBox="0 0 24 24"><path d="M6 3h9l4 4v14H6z"></path><path d="M9 3v5h6"></path><rect x="9" y="13" width="6" height="6"></rect></svg>
        </button>
        <span class="tb-sep"></span>
        <button
          class="tb-btn"
          onclick={onNewSlide}
          type="button"
          disabled={!deck}
          title="New slide (Cmd/Ctrl+Shift+N)"
          aria-label="New slide"
        >
          <svg viewBox="0 0 24 24"><rect x="3" y="4" width="13" height="16" rx="1"></rect><path d="M17 10h5v10h-8"></path><path d="M19.5 13v4M17.5 15h4"></path></svg>
        </button>
      </span>

      <span class="tb-sep"></span>

      <!-- Center group: Text Box | Shape (flyout) | Image | Table | Chart -->
      <span class="tb-group">
        <button
          class="tb-btn"
          class:active={creatingTextBox}
          onclick={enterCreateTextBox}
          type="button"
          disabled={!deck || !activeSlide}
          title="Text box — then click the slide to place it"
          aria-label="Text box"
          aria-pressed={creatingTextBox}
        >
          <svg viewBox="0 0 24 24"><rect x="3" y="5" width="18" height="14" rx="1"></rect><text x="12" y="17" text-anchor="middle" font-size="12" font-weight="bold" stroke="none" fill="currentColor">T</text></svg>
        </button>
        <span class="shape-picker-wrap">
          <button
            class="tb-btn"
            class:active={showShapeFlyout}
            onclick={() => (showShapeFlyout = !showShapeFlyout)}
            type="button"
            disabled={!deck || !activeSlide}
            title="Shapes"
            aria-label="Shapes"
            aria-expanded={showShapeFlyout}
          >
            <svg viewBox="0 0 24 24"><path d="M4 17l4-8 4 8z"></path><rect x="13" y="9" width="7" height="7" rx="1"></rect></svg>
          </button>
          {#if showShapeFlyout}
            <button
              class="picker-backdrop"
              onclick={() => (showShapeFlyout = false)}
              type="button"
              aria-label="Close shape picker"
            ></button>
            <div class="shape-picker-popover">
              <ShapePicker onPick={(kind) => onAddShape(kind)} />
            </div>
          {/if}
        </span>
        <button
          class="tb-btn"
          onclick={onInsertImage}
          type="button"
          disabled={!deck || !activeSlide}
          title="Insert image"
          aria-label="Insert image"
        >
          <svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="1"></rect><circle cx="8.5" cy="9.5" r="1.6"></circle><path d="M4 18l5-5 4 4 3-3 4 4"></path></svg>
        </button>
        <span class="table-picker-wrap">
          <button
            class="tb-btn"
            onclick={() => {
              showTablePicker = !showTablePicker
              pickerRows = 1
              pickerCols = 1
            }}
            type="button"
            disabled={!deck || !activeSlide}
            title="Table"
            aria-label="Table"
            aria-expanded={showTablePicker}
          >
            <svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="1"></rect><path d="M3 10h18M3 15h18M9 4v16M15 4v16"></path></svg>
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
        </span>
        <span class="chart-picker-wrap">
          <button
            class="tb-btn"
            onclick={() => (showChartDropdown = !showChartDropdown)}
            type="button"
            disabled={!deck || !activeSlide}
            title="Chart"
            aria-label="Chart"
            aria-expanded={showChartDropdown}
          >
            <svg viewBox="0 0 24 24"><rect x="4" y="11" width="4" height="9" fill="currentColor" stroke="none"></rect><rect x="10" y="6" width="4" height="14" fill="currentColor" stroke="none"></rect><rect x="16" y="14" width="4" height="6" fill="currentColor" stroke="none"></rect></svg>
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
        </span>
      </span>
      <span class="tb-spacer"></span>

      <!-- Right group: Bold | Italic | Underline | separator | Present | Export | Help -->
      <span class="tb-group">
        <button class="tb-btn tb-text" onclick={() => toggleRunFlag('bold')} type="button" title="Bold (Cmd/Ctrl+B)" aria-label="Bold"><strong>B</strong></button>
        <button class="tb-btn tb-text" onclick={() => toggleRunFlag('italic')} type="button" title="Italic (Cmd/Ctrl+I)" aria-label="Italic"><em>I</em></button>
        <button class="tb-btn tb-text" onclick={() => toggleRunFlag('underline')} type="button" title="Underline (Cmd/Ctrl+U)" aria-label="Underline"><u>U</u></button>
        <span class="tb-sep"></span>
        <button class="tb-btn" onclick={onStartPresenter} type="button" disabled={!deck} title="Present (Cmd/Ctrl+Enter)" aria-label="Present">
          <svg viewBox="0 0 24 24"><path d="M5 4l13 8-13 8z" fill="currentColor" stroke="none"></path></svg>
        </button>
        <span class="export-picker-wrap">
          <button
            class="tb-btn"
            onclick={() => (showExportMenu = !showExportMenu)}
            type="button"
            disabled={!deck || exporting !== ''}
            title="Export"
            aria-label="Export"
            aria-expanded={showExportMenu}
          >
            <svg viewBox="0 0 24 24"><path d="M12 3v11"></path><path d="M8 10l4 4 4-4"></path><path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"></path></svg>
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
        <button class="tb-btn tb-text" onclick={() => (showShortcuts = true)} type="button" title="Keyboard shortcuts (?)" aria-label="Help">?</button>
      </span>
      <input
        bind:this={imageInput}
        class="hidden-input"
        type="file"
        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml"
        onchange={handleImageSelected}
      />
    </header>

    <!-- Secondary format bar: advanced text formatting, table ops, deck tools. -->
    <div class="format-bar" role="toolbar" aria-label="Formatting" inert={documentBusy}>
      <span class="tb-group">
        <button class="tb-btn tb-text" onclick={() => toggleRunFlag('strikethrough')} type="button" title="Strikethrough" aria-label="Strikethrough"><s>S</s></button>
        <button class="tb-btn tb-text" onclick={toggleSuperscript} type="button" title="Superscript" aria-label="Superscript">x<sup>2</sup></button>
        <button class="tb-btn tb-text" onclick={toggleSubscript} type="button" title="Subscript" aria-label="Subscript">x<sub>2</sub></button>
        <button class="tb-btn tb-text" onclick={() => toggleRunFlag('code')} type="button" title="Inline code" aria-label="Inline code">&lt;/&gt;</button>
        <select
          class="tb-select"
          onchange={(event) => {
            const value = (event.target as HTMLSelectElement).value
            setHeading(value === 'paragraph' ? null : (value as HeadingLevelDto))
          }}
          title="Heading level"
          aria-label="Heading level"
        >
          <option value="paragraph">Paragraph</option>
          <option value="h1">Heading 1</option>
          <option value="h2">Heading 2</option>
          <option value="h3">Heading 3</option>
          <option value="h4">Heading 4</option>
          <option value="h5">Heading 5</option>
          <option value="h6">Heading 6</option>
        </select>
        <button class="tb-btn" onclick={() => toggleParagraphFlag('blockquote')} type="button" title="Blockquote" aria-label="Blockquote">
          <svg viewBox="0 0 24 24"><path d="M4 7v6h5l-2 4M13 7v6h5l-2 4" fill="none"></path></svg>
        </button>
        <button class="tb-btn" onclick={() => toggleParagraphFlag('codeBlock')} type="button" title="Code block" aria-label="Code block">
          <svg viewBox="0 0 24 24"><path d="M9 9l-4 3 4 3M15 9l4 3-4 3"></path></svg>
        </button>
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
      <span class="tb-sep"></span>
      <span class="tb-group">
        <button class="tb-btn tb-text sm" onclick={onInsertRow} type="button" disabled={!hasActiveCell} title="Insert row below">+ Row</button>
        <button class="tb-btn tb-text sm" onclick={onInsertColumn} type="button" disabled={!hasActiveCell} title="Insert column right">+ Col</button>
        <button class="tb-btn tb-text sm" onclick={onDeleteRow} type="button" disabled={!hasActiveCell} title="Delete row">− Row</button>
        <button class="tb-btn tb-text sm" onclick={onDeleteColumn} type="button" disabled={!hasActiveCell} title="Delete column">− Col</button>
      </span>
      <span class="tb-sep"></span>
      <span class="tb-group">
        <select
          class="tb-select"
          value={currentAspectRatio}
          onchange={(event) =>
            onSetAspectRatio(
              (event.target as HTMLSelectElement).value as '16:9' | '4:3' | '16:10' | 'default',
            )
          }
          title="Slide aspect ratio"
          aria-label="Slide aspect ratio"
        >
          <option value="16:9">16:9</option>
          <option value="4:3">4:3</option>
          <option value="16:10">16:10</option>
          <option value="default">Reset</option>
        </select>
        <button
          class="tb-btn tb-text"
          onclick={toggleHighContrast}
          type="button"
          class:active={highContrast}
          title="Toggle high-contrast theme"
          aria-pressed={highContrast}
        >
          Contrast
        </button>
        <button
          class="tb-btn tb-text"
          onclick={() => {
            findReplaceMode = 'find'
            showFindReplace = true
          }}
          type="button"
          title="Find (Cmd/Ctrl+F)"
        >
          Find
        </button>
        <button
          class="tb-btn tb-text"
          onclick={openVersionHistory}
          type="button"
          disabled={!deck}
          title="Local version history"
        >
          History
        </button>
        <button
          class="tb-btn tb-text"
          onclick={() => (showComments = !showComments)}
          type="button"
          class:active={showComments}
          disabled={!deck}
          title="Comments (C)"
          aria-pressed={showComments}
        >
          Comments
        </button>
        <button
          class="tb-btn tb-text"
          onclick={openAccessibility}
          type="button"
          class:active={showAccessibility}
          disabled={!deck}
          title="Document accessibility checks"
          aria-pressed={showAccessibility}
        >
          A11y
        </button>
      </span>
    </div>

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
    {#if actionError}
      <div class="banner action-error" role="alert">
        <strong>Editor action failed:</strong> {actionError}
        <button onclick={() => (actionError = '')} type="button">Dismiss</button>
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

    <div class="workspace" inert={documentBusy}>
      <aside class="sidebar" aria-label="Slide thumbnails">
        {#if deck}
          <div class="slide-navigation-heading">
            <strong>Slides</strong>
            <span>{deck.slides.length}</span>
          </div>
          <div class="slide-actions" role="group" aria-label="Organize selected slide">
            <button type="button" onclick={() => moveActiveSlide(-1)} disabled={activeIndex === 0 || documentBusy}
              aria-label="Move selected slide up" title="Move selected slide up">↑ Up</button>
            <button type="button" onclick={() => moveActiveSlide(1)} disabled={activeIndex >= deck.slides.length - 1 || documentBusy}
              aria-label="Move selected slide down" title="Move selected slide down">↓ Down</button>
            <button class="delete-slide" type="button" onclick={deleteActiveSlide} disabled={deck.slides.length <= 1 || documentBusy}
              aria-label="Delete selected slide" title={deck.slides.length <= 1 ? 'Keep at least one slide' : 'Delete selected slide (can be undone)'}>Delete</button>
          </div>
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
              {#each group.indices as index (deck.slides[index].id)}
                {@const slide = deck.slides[index]}
                <span class="slide-number" aria-hidden="true">{index + 1}</span>
                <SlideThumbnail
                  {slide}
                  deckRenderKey={thumbnailDeckKey}
                  aspectRatio={thumbnailAspectRatio}
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
          <button class="add-slide-btn" type="button" onclick={onNewSlide} title="New slide">
            + Slide
          </button>
        {/if}
      </aside>

      <main
        class="canvas-area"
        class:text-box-mode={creatingTextBox}
        class:shape-mode={creatingShapeKind !== null}
        bind:this={canvasAreaEl}
        aria-label="Editor canvas"
      >
        {#if activeSlide && deck}
          <SlideCanvas
            bind:this={canvasEditor}
            onDraftChange={(dirty) => (canvasDraftDirty = dirty)}
            slide={activeSlide}
            background={deck.theme.background}
            bodyFont={deck.theme.bodyFont}
            headingFont={deck.theme.headingFont}
            media={deck.media}
            slideSize={slideSize}
            scale={canvasScale}
            highContrast={highContrast}
            selectedShapeIndex={canvasSelectedShapeIndex}
            {editingShapeIndex}
            placeholderGuides={activeLayoutPlaceholders}
            {creatingTextBox}
            creatingShapeKind={creatingShapeKind}
            onPlaceTextBox={onCanvasClickCreateTextBox}
            {onPlaceShape}
            onEditTextBox={handleTextEdit}
            onSetCellText={handleSetCellText}
            onCellFocus={handleCellFocus}
            onEditChart={handleEditChart}
            onSelectShape={handleSelectShape}
            onEditShape={handleEditShape}
            onUpdateShapeTransform={handleUpdateShapeTransform}
            onShapeContextMenu={handleShapeContextMenu}
            onCommentOnSelection={handleCommentOnSelection}
          />
          {#if activeSlide.shapes.length === 0 && !creatingTextBox && !creatingShapeKind}
            <div class="slide-start">
              <span class="start-eyebrow">Your next idea starts here</span>
              <h2>Make this slide yours.</h2>
              <p>Add a title, bring in an image, or start with a built-in theme.</p>
              <div>
                <button class="primary" type="button" onclick={addStarterTitle}>Add a title</button>
                <button type="button" onclick={onInsertImage}>Insert image</button>
                <button type="button" onclick={openTemplatePicker}>Choose a template</button>
              </div>
            </div>
          {/if}
        {:else}
          <div class="empty-canvas">Open or create a deck to start editing.</div>
        {/if}
      </main>

      <aside class="right-panel" class:inspector-hidden={!showInspector} aria-label="Inspector, notes and animation">
        <div class="tab-bar" role="tablist">
          <button
            class="tab"
            class:active={rightPanelTab === 'style'}
            onclick={() => selectInspectorTab('style')}
            type="button"
            role="tab"
            aria-selected={rightPanelTab === 'style'}
          >
            Style
          </button>
          <button
            class="tab"
            class:active={rightPanelTab === 'text'}
            onclick={() => selectInspectorTab('text')}
            type="button"
            role="tab"
            aria-selected={rightPanelTab === 'text'}
          >
            Text
          </button>
          <button
            class="tab"
            class:active={rightPanelTab === 'arrange'}
            onclick={() => selectInspectorTab('arrange')}
            type="button"
            role="tab"
            aria-selected={rightPanelTab === 'arrange'}
          >
            Arrange
          </button>
          <button
            class="tab"
            class:active={rightPanelTab === 'notes'}
            onclick={() => selectInspectorTab('notes')}
            type="button"
            role="tab"
            aria-selected={rightPanelTab === 'notes'}
          >
            Notes
          </button>
          <button
            class="tab"
            class:active={rightPanelTab === 'animation'}
            onclick={() => selectInspectorTab('animation')}
            type="button"
            role="tab"
            aria-selected={rightPanelTab === 'animation'}
          >
            Animation
          </button>
        </div>

        {#if rightPanelTab === 'style'}
          <div class="panel-content" role="tabpanel">
            {#if selectedGeometric}
              <div class="section">
                <h3>Shape style</h3>
                <label class="field">
                  Fill
                  <input
                    type="color"
                    value={colorToHex(selectedGeometric.style.fill?.solid ?? deck!.theme.accentColor)}
                    onchange={(event) => {
                      const color = colorFromHex(
                        (event.target as HTMLInputElement).value,
                        selectedGeometric!.style.fill?.solid?.a ?? 255,
                      )
                      void updateSelectedGeometryStyle((style) => ({
                        ...style,
                        fill: { solid: color },
                      }))
                    }}
                  />
                </label>
                <label class="field">
                  Outline
                  <input
                    type="color"
                    value={colorToHex(selectedGeometric.style.outline?.color ?? { r: 0, g: 0, b: 0, a: 255 })}
                    onchange={(event) => {
                      const color = colorFromHex((event.target as HTMLInputElement).value)
                      void updateSelectedGeometryStyle((style) => ({
                        ...style,
                        outline: {
                          color,
                          widthEmu: style.outline?.widthEmu ?? 9_525,
                          dash: style.outline?.dash ?? 'solid',
                        },
                      }))
                    }}
                  />
                </label>
                <label class="field">
                  Outline width
                  <input
                    type="range"
                    min="0"
                    max="76200"
                    step="9525"
                    value={selectedGeometric.style.outline?.widthEmu ?? 0}
                    onchange={(event) => {
                      const widthEmu = Number((event.target as HTMLInputElement).value)
                      void updateSelectedGeometryStyle((style) => ({
                        ...style,
                        outline: widthEmu > 0
                          ? {
                              color: style.outline?.color ?? { r: 0, g: 0, b: 0, a: 255 },
                              widthEmu,
                              dash: style.outline?.dash ?? 'solid',
                            }
                          : undefined,
                      }))
                    }}
                  />
                </label>
                <label class="field">
                  Opacity: {Math.round((selectedGeometric.style.fill?.solid?.a ?? 255) / 2.55)}%
                  <input
                    type="range"
                    min="0"
                    max="255"
                    value={selectedGeometric.style.fill?.solid?.a ?? 255}
                    onchange={(event) => {
                      const alpha = Number((event.target as HTMLInputElement).value)
                      void updateSelectedGeometryStyle((style) => ({
                        ...style,
                        fill: {
                          solid: {
                            ...(style.fill?.solid ?? deck!.theme.accentColor),
                            a: alpha,
                          },
                        },
                      }))
                    }}
                  />
                </label>
                <label class="check-field">
                  <input
                    type="checkbox"
                    checked={selectedGeometric.style.shadow !== undefined}
                    onchange={(event) => {
                      const enabled = (event.target as HTMLInputElement).checked
                      void updateSelectedGeometryStyle((style) => ({
                        ...style,
                        shadow: enabled
                          ? (style.shadow ?? {
                              offsetX: 28_575,
                              offsetY: 28_575,
                              blur: 38_100,
                              color: { r: 0, g: 0, b: 0, a: 255 },
                              opacity: 0.25,
                            })
                          : undefined,
                      }))
                    }}
                  />
                  Shadow
                </label>
              </div>
            {:else}
              <p class="placeholder">Select a geometric shape to edit its fill, outline, opacity, and shadow.</p>
            {/if}
          </div>
        {:else if rightPanelTab === 'text'}
          <div class="panel-content" role="tabpanel">
            {#if selectedShapeIndex !== null && activeSlide?.shapes[selectedShapeIndex]?.kind === 'text_box'}
              <div class="section">
                <h3>Text formatting</h3>
                <div class="inspector-actions">
                  <button class="tb-btn tb-text" type="button" onclick={() => toggleRunFlag('bold')}>B</button>
                  <button class="tb-btn tb-text" type="button" onclick={() => toggleRunFlag('italic')}><em>I</em></button>
                  <button class="tb-btn tb-text" type="button" onclick={() => toggleRunFlag('underline')}><u>U</u></button>
                  <button class="tb-btn tb-text" type="button" onclick={() => toggleRunFlag('strikethrough')}><s>S</s></button>
                </div>
                <p class="helper-copy">Double-click the box to edit its text. These controls apply to the active run while editing, or to the selected text box otherwise.</p>
              </div>
            {:else}
              <p class="placeholder">Select a text box to format its text.</p>
            {/if}
          </div>
        {:else if rightPanelTab === 'arrange'}
          <div class="panel-content" role="tabpanel">
            {#if selectedShapeFrame}
              <div class="section">
                <h3>Position and size</h3>
                <div class="arrange-grid">
                  <label class="field">X<input type="number" value={Math.round(selectedShapeFrame.x)} onchange={(event) => updateSelectedFrame('x', Number((event.target as HTMLInputElement).value))} /></label>
                  <label class="field">Y<input type="number" value={Math.round(selectedShapeFrame.y)} onchange={(event) => updateSelectedFrame('y', Number((event.target as HTMLInputElement).value))} /></label>
                  <label class="field">Width<input type="number" min="76200" value={Math.round(selectedShapeFrame.width)} onchange={(event) => updateSelectedFrame('width', Number((event.target as HTMLInputElement).value))} /></label>
                  <label class="field">Height<input type="number" min="76200" value={Math.round(selectedShapeFrame.height)} onchange={(event) => updateSelectedFrame('height', Number((event.target as HTMLInputElement).value))} /></label>
                </div>
                <p class="helper-copy">Drag selected objects on the canvas or use the eight handles to resize them.</p>
              </div>
            {:else}
              <p class="placeholder">Select an object to move or resize it.</p>
            {/if}
          </div>
        {:else if rightPanelTab === 'notes'}
          <div class="panel-content" role="tabpanel">
            {#if activeRichNotes}
              <RichNotesEditor
                bind:this={notesEditor}
                onDraftChange={(dirty) => (notesDraftDirty = dirty)}
                slideId={activeSlide!.id}
                richNotes={activeRichNotes}
                onSetRichNotes={handleSetRichNotes}
              />
              <button class="notes-toggle" type="button" onclick={usePlainNotes}>
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

            {#if activeSlide}
              <AnimationPane slide={activeSlide} onApplied={(next) => (deck = next)} />
            {/if}
          </div>
        {/if}
      </aside>
    </div>

    <footer class="status-bar">
      <span>{deck ? `Slide ${activeIndex + 1} of ${deck.slides.length}` : 'No presentation open'}</span>
      <span class="offline-status">Offline · No account needed</span>
      <button type="button" aria-pressed={showInspector} onclick={() => (showInspector = !showInspector)} disabled={documentBusy}>
        {showInspector ? 'Hide inspector' : 'Show inspector'}
      </button>
    </footer>

    {#if savePrompt}
      <SaveChangesDialog name={documentName} action={savePrompt.action} onChoose={chooseSaveAction} />
    {/if}

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

    {#if showVersionHistory && deck}
      <VersionHistory
        onClose={() => (showVersionHistory = false)}
        onRestore={onRestoreVersion}
        onBeforeRestore={flushEditorDrafts}
      />
    {/if}

    {#if showComments && deck}
      <Comments
        deck={deck}
        activeSlideId={activeSlide?.id ?? ''}
        draft={commentDraft}
        onApplied={handleCommentApplied}
        onClearDraft={() => (commentDraft = null)}
        onClose={closeComments}
      />
    {/if}

    {#if showAccessibility && deck}
      <AccessibilityPanel
        report={accessibility}
        {deck}
        checking={checkingAccessibility}
        onRecheck={() => void runAccessibility()}
        onNavigate={navigateToIssue}
        onClose={() => (showAccessibility = false)}
      />
    {/if}

    {#if shapeMenu}
      <button
        type="button"
        class="picker-backdrop"
        aria-label="Close shape menu"
        onclick={() => (shapeMenu = null)}
      ></button>
      <div
        class="shape-context-menu"
        style:left={`${shapeMenu.x}px`}
        style:top={`${shapeMenu.y}px`}
        role="menu"
        aria-label="Add comment"
      >
        <button type="button" role="menuitem" onclick={addShapeComment}>Add comment</button>
      </div>
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
    font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    color: #202636;
    background: #e9edf5;
  }
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    min-width: 640px;
    background: #f7f8fb;
  }
  .document-header { display: flex; align-items: center; gap: 1rem; padding: .75rem 1rem; background: #fff; border-bottom: 1px solid #dce3ed; flex-shrink: 0; }
  .app-mark { display: flex; flex-direction: column; font-weight: 800; color: #245eb3; font-size: 1.05rem; line-height: 1; letter-spacing: -.04em; padding-right: 1rem; border-right: 1px solid #dce3ed; }
  .app-mark span { margin-top: .2rem; color: #495a73; font-size: .65rem; font-weight: 600; letter-spacing: .02em; }
  .document-heading { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: .25rem; }
  .document-heading strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: .92rem; font-weight: 650; }
  .document-heading > span { font-size: .7rem; color: #64758b; }
  .document-heading > .unsaved { color: #986000; }
  .document-actions { display: flex; align-items: center; gap: .4rem; }
  .document-actions button, .slide-start button { padding: .5rem .75rem; background: white; color: #2c415d; border: 1px solid #ced8e6; border-radius: 6px; font-size: .76rem; cursor: pointer; }
  .document-actions .present-action, .slide-start .primary { background: #245eb3; color: white; border-color: #245eb3; }
  .document-actions button:disabled { opacity: .5; cursor: default; }
  .status-bar { display: flex; align-items: center; gap: 1rem; min-height: 31px; padding: 0 .8rem; border-top: 1px solid #d5deeb; color: #5b6c82; font-size: .7rem; background: #fff; flex-shrink: 0; }
  .status-bar .offline-status { flex: 1; text-align: center; }
  .status-bar button { font: inherit; border: none; color: #245eb3; background: transparent; padding: .35rem; cursor: pointer; margin-left: auto; }
  .slide-start { position: absolute; max-width: calc(100% - 5rem); text-align: center; color: #344966; background: #ffffffee; border: 1px solid #dce4ee; border-radius: 12px; padding: 1.8rem; box-shadow: 0 10px 36px #1839600c; }
  .start-eyebrow { font-size: .65rem; letter-spacing: .1em; color: #6a7e9b; text-transform: uppercase; font-weight: 700; }
  .slide-start h2 { font-size: clamp(1rem, 1.6vw, 1.45rem); letter-spacing: -.035em; margin: .7rem 0; }
  .slide-start p { font-size: .78rem; line-height: 1.5; color: #6d7b8f; max-width: 24rem; margin: 0 auto 1.1rem; }
  .slide-start > div { display: flex; gap: .5rem; justify-content: center; flex-wrap: wrap; }
  :global(button:focus-visible), :global(select:focus-visible), :global(input:focus-visible) { outline: 2px solid #3077d0; outline-offset: 2px; }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    min-height: 44px;
    padding: 0.38rem 0.65rem;
    border-bottom: 1px solid #d7dce7;
    background: #ffffff;
    box-shadow: 0 1px 2px rgba(27, 39, 65, 0.05);
    flex-wrap: wrap;
    flex-shrink: 0;
  }
  .tb-group {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }
  .tb-sep {
    width: 1px;
    align-self: stretch;
    background: #d0d0d0;
    margin: 0.1rem 0.15rem;
  }
  .tb-spacer {
    flex: 1;
  }
  .tb-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 30px;
    height: 30px;
    padding: 0 0.4rem;
    background: #fff;
    border: 1px solid #d8deea;
    border-radius: 5px;
    color: #273043;
    cursor: pointer;
    line-height: 1;
  }
  .tb-btn:hover:not(:disabled) {
    background: #e8f1fb;
    border-color: #0070c0;
  }
  .tb-btn:active:not(:disabled) {
    background: #d6e9fa;
  }
  .tb-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .tb-btn.active {
    background: #0070c0;
    border-color: #0070c0;
    color: #fff;
  }
  .tb-btn svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .tb-text {
    font-size: 0.85rem;
    font-weight: 600;
    padding: 0 0.5rem;
  }
  .tb-text.sm {
    font-size: 0.72rem;
    font-weight: 500;
  }
  .tb-select {
    height: 30px;
    padding: 0 0.3rem;
    border: 1px solid #d0d0d0;
    border-radius: 4px;
    background: #fff;
    font-size: 0.8rem;
  }
  .format-bar {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.25rem 0.5rem;
    border-bottom: 1px solid #d7dce7;
    background: #f7f8fb;
    flex-wrap: wrap;
    flex-shrink: 0;
  }
  .shape-picker-wrap {
    position: relative;
    display: inline-flex;
  }
  .shape-picker-popover {
    position: absolute;
    top: 100%;
    left: 0;
    z-index: 20;
    margin-top: 0.25rem;
  }
  .code-steps-input {
    width: 9rem;
    padding: 0.3rem 0.4rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.8rem;
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
  .action-error {
    background: #fff0f1;
    border-bottom-color: #efb7bd;
    color: #7b1f2a;
  }
  .workspace {
    display: flex;
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }
  .sidebar {
    width: 196px;
    overflow-y: auto;
    border-right: 1px solid #d7dce7;
    background: #f5f7fb;
    padding: 0.65rem;
    flex-shrink: 0;
  }
  .slide-navigation-heading { display: flex; align-items: center; justify-content: space-between; margin: .1rem .1rem .55rem; font-size: .76rem; color: #344966; }
  .slide-navigation-heading span { font-variant-numeric: tabular-nums; font-size: .68rem; color: #76859a; }
  .slide-actions { display: flex; flex-wrap: wrap; gap: .25rem; margin-bottom: .75rem; }
  .slide-actions button { flex: 1; min-width: 44px; padding: .4rem .15rem; border: 1px solid #d3ddeb; border-radius: 4px; color: #385170; background: #fff; font-size: .65rem; cursor: pointer; white-space: nowrap; }
  .slide-actions button:hover:not(:disabled) { background: #e7effb; border-color: #7da4d5; }
  .slide-actions button:disabled { opacity: .4; cursor: default; }
  .slide-actions .delete-slide { color: #974250; }
  .slide-number { display: block; margin: .25rem .1rem; color: #72829a; font-size: .64rem; font-variant-numeric: tabular-nums; }
  .add-slide-btn {
    display: block;
    width: 100%;
    margin-top: 0.5rem;
    padding: 0.5rem;
    border: 1px solid #b9c8e2;
    background: #ffffff;
    color: #1f5ca8;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .add-slide-btn:hover {
    background: #e8e8e8;
    border-color: #666;
  }
  .canvas-area {
    flex: 1;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, #e9edf5, #dce3ef);
    overflow: auto;
    min-width: 0;
  }
  .canvas-area.text-box-mode,
  .canvas-area.shape-mode {
    cursor: crosshair;
  }
  .empty-canvas {
    color: #666;
  }
  .right-panel {
    width: 252px;
    border-left: 1px solid #d7dce7;
    background: #f8f9fc;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    flex-shrink: 0;
  }
  .right-panel.inspector-hidden { display: none; }
  @media (max-width: 1100px) {
    .sidebar { width: 144px; padding: .5rem; }
    .right-panel { width: 220px; }
    .document-header { padding: .65rem .75rem; gap: .7rem; }
    .slide-start { padding: 1.2rem; }
  }
  @media (max-width: 800px) {
    .sidebar { width: 116px; }
    .right-panel { width: 196px; }
    .document-actions button { padding: .45rem .55rem; }
    .document-heading strong { font-size: .8rem; }
    .app-mark { padding-right: .7rem; }
    .status-bar .offline-status { display: none; }
    .slide-start { max-width: calc(100% - 3rem); padding: .8rem; }
  }
  .tab-bar {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    border-bottom: 1px solid #d7dce7;
  }
  .tab {
    min-width: 0;
    padding: 0.48rem 0.25rem;
    background: #f1f4f9;
    border: none;
    border-right: 1px solid #dce2ec;
    cursor: pointer;
  }
  .tab:last-child {
    border-right: none;
  }
  .tab.active {
    background: #ffffff;
    color: #145ca8;
    font-weight: 700;
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
  .check-field {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.85rem;
  }
  .inspector-actions {
    display: flex;
    gap: 0.35rem;
  }
  .arrange-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.45rem;
  }
  .arrange-grid .field {
    min-width: 0;
  }
  .helper-copy {
    margin: 0.55rem 0 0;
    color: #677084;
    font-size: 0.78rem;
    line-height: 1.4;
  }
  .placeholder {
    color: #888;
  }
  .shape-context-menu {
    position: fixed;
    z-index: 70;
    min-width: 150px;
    display: flex;
    flex-direction: column;
    background: #fff;
    border: 1px solid #ccc;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  }
  .shape-context-menu button {
    text-align: left;
    background: none;
    border: none;
    padding: 0.4rem 0.8rem;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .shape-context-menu button:hover {
    background: #f0f0f0;
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
