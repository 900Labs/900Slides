<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import type {
    AnimationDto,
    BorderEdgeDto,
    BuildEffectDto,
    CellAlignDto,
    ChartShapeSnapshot,
    ColorDto,
    CropDto,
    DashStyleDto,
    FillDto,
    GeometryDto,
    GeometricShapeSnapshot,
    HeadingLevelDto,
    ImageShapeSnapshot,
    MediaMap,
    MisspellingDto,
    ParagraphDto,
    ParagraphStyleDto,
    PassthroughSnapshot,
    RunDto,
    SlideSizeDto,
    SlideSnapshot,
    StyleDto,
    TableBordersDto,
    TableCellDto,
    TableShapeSnapshot,
    TextBoxSnapshot,
    VerticalAlignDto,
  } from './lib/types'
  import { codeLineState } from './lib/codeSteps'

  /** Props for the slide canvas. */
  interface Props {
    /** Slide to render. */
    slide: SlideSnapshot
    /** Deck background color. */
    background: ColorDto
    /** Media store, base64-encoded, used to render image shapes. */
    media?: MediaMap
    /** Callback invoked when a text box is edited. */
    onEditTextBox?: (detail: {
      slideId: string
      shapeIndex: number
      paragraphs: ParagraphDto[]
    }) => void
    /** Callback invoked when a table cell's text is edited. */
    onSetCellText?: (detail: {
      slideId: string
      shapeIndex: number
      row: number
      col: number
      text: string
    }) => void
    /** Callback invoked when a table cell gains focus. */
    onCellFocus?: (detail: { shapeIndex: number; row: number; col: number }) => void
    /** Callback invoked when a chart shape is double-clicked. */
    onEditChart?: (detail: { slideId: string; shapeIndex: number }) => void
    /** Callback invoked when a shape is clicked to select it. */
    onSelectShape?: (detail: { shapeIndex: number }) => void
    /** Whether the canvas is read-only (presenter mode). */
    readonly?: boolean
    /** Current build step index for presenter playback. */
    activeBuildStep?: number
    /** Active code-step index (0-based) for stepped code highlighting. */
    codeActiveStep?: number
    /** Deck slide size (aspect ratio). Defaults to 16:9 when unset. */
    slideSize?: SlideSizeDto
    /** Whether the deck is rendered in high-contrast mode. */
    highContrast?: boolean
  }

  let {
    slide,
    background,
    media,
    onEditTextBox,
    onSetCellText,
    onCellFocus,
    onEditChart,
    onSelectShape,
    readonly = false,
    activeBuildStep = Infinity,
    codeActiveStep = 0,
    slideSize,
    highContrast = false,
  }: Props = $props()

  /** Canvas width in pixels, derived from the deck slide size or 16:9 default. */
  const canvasWidthPx = $derived(
    toPx(slideSize?.widthEmu ?? 12_192_000),
  )
  /** Canvas height in pixels, derived from the deck slide size or 16:9 default. */
  const canvasHeightPx = $derived(toPx(slideSize?.heightEmu ?? 6_858_000))
  /** Background color, forced to black when high-contrast is on. */
  const effectiveBackground = $derived<ColorDto>(
    highContrast ? { r: 0, g: 0, b: 0, a: 255 } : background,
  )

  /** Current build-in state per shape index for presenter playback. */
  const shapeBuildStates = $derived<Map<number, ShapeBuildState | null>>(
    readonly && slide.animation
      ? new Map(
          slide.shapes.map((_, index) => [
            index,
            shapeBuildState(slide.animation, index, activeBuildStep),
          ]),
        )
      : new Map(),
  )

  /** Looks up the build-in state for a shape, returning undefined if none. */
  function buildStateFor(shapeIndex: number): ShapeBuildState | undefined {
    return shapeBuildStates.get(shapeIndex) ?? undefined
  }

  /** Combines an existing image rotation with the build-in transform. */
  function imageTransform(rotation: number, shapeIndex: number): string | undefined {
    const parts: string[] = []
    if (rotation) parts.push(`rotate(${rotation}deg)`)
    const buildTransform = buildStateFor(shapeIndex)?.transform
    if (buildTransform && buildTransform !== 'none') parts.push(buildTransform)
    return parts.length > 0 ? parts.join(' ') : undefined
  }

  /** Combines an existing table rotation with the build-in transform. */
  function tableTransform(rotation: number, shapeIndex: number): string | undefined {
    const parts: string[] = []
    if (rotation) parts.push(`rotate(${rotation}deg)`)
    const buildTransform = buildStateFor(shapeIndex)?.transform
    if (buildTransform && buildTransform !== 'none') parts.push(buildTransform)
    return parts.length > 0 ? parts.join(' ') : undefined
  }

  /** EMU to CSS pixels for a 1280x720 (16:9) canvas. */
  const EMU_TO_PX = 1.0 / 9525.0

  /** Converts EMU to a pixel CSS string. */
  function toPx(emu: number): string {
    return `${emu * EMU_TO_PX}px`
  }

  /** Converts a ColorDto to a CSS rgba string. */
  function toRgba(color: ColorDto): string {
    return `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a / 255})`
  }

  /** Formats a ColorDto as an opaque `#rrggbb` hex string. */
  function toHex(color: ColorDto): string {
    const h = (n: number) => n.toString(16).padStart(2, '0')
    return `#${h(color.r)}${h(color.g)}${h(color.b)}`
  }

  /** Builds a `data:` URI for a media entry. */
  function dataUri(entry: { mime: string; bytes: string }): string {
    return `data:${entry.mime};base64,${entry.bytes}`
  }

  /** Returns the off-screen transform for a slide-in effect before activation. */
  function buildInitialTransform(effect: BuildEffectDto): string {
    switch (effect) {
      case 'slide_in_left':
        return 'translateX(-100%)'
      case 'slide_in_right':
        return 'translateX(100%)'
      case 'slide_in_top':
        return 'translateY(-100%)'
      case 'slide_in_bottom':
        return 'translateY(100%)'
      default:
        return 'none'
    }
  }

  /** Returns a CSS transition string for a build effect. */
  function buildTransition(effect: BuildEffectDto, durationMs: number): string | undefined {
    switch (effect) {
      case 'fade':
        return `opacity ${durationMs}ms ease, visibility ${durationMs}ms step-end`
      case 'slide_in_left':
      case 'slide_in_right':
      case 'slide_in_top':
      case 'slide_in_bottom':
        return `opacity ${durationMs}ms ease, transform ${durationMs}ms ease, visibility ${durationMs}ms step-end`
      case 'disappear':
        return `opacity ${durationMs}ms ease, visibility ${durationMs}ms step-end`
      case 'appear':
      default:
        return undefined
    }
  }

  interface ShapeBuildState {
    opacity: number
    visibility: 'visible' | 'hidden'
    transform: string
    transition: string | undefined
  }

  /** Computes the current build-in state for a shape at the active step. */
  function shapeBuildState(
    animation: AnimationDto | undefined,
    shapeIndex: number,
    activeStep: number,
  ): ShapeBuildState | null {
    if (!animation || animation.steps.length === 0) return null
    const activeSteps = animation.steps
      .map((step, index) => ({ ...step, index }))
      .filter((step) => step.shapeIndex === shapeIndex && step.index <= activeStep)
    const pendingSteps = animation.steps
      .map((step, index) => ({ ...step, index }))
      .filter((step) => step.shapeIndex === shapeIndex && step.index > activeStep)

    if (activeSteps.length === 0) {
      const nextStep = pendingSteps[0]
      if (!nextStep) return null
      if (nextStep.effect === 'disappear') {
        return {
          opacity: 1,
          visibility: 'visible',
          transform: 'none',
          transition: undefined,
        }
      }
      return {
        opacity: 0,
        visibility: 'hidden',
        transform: buildInitialTransform(nextStep.effect),
        transition: buildTransition(nextStep.effect, nextStep.durationMs),
      }
    }

    const lastActive = activeSteps[activeSteps.length - 1]
    if (lastActive.effect === 'disappear') {
      return {
        opacity: 0,
        visibility: 'hidden',
        transform: 'none',
        transition: buildTransition('disappear', lastActive.durationMs),
      }
    }
    return {
      opacity: 1,
      visibility: 'visible',
      transform: 'none',
      transition: buildTransition(lastActive.effect, lastActive.durationMs),
    }
  }

  /** Joins all runs in all paragraphs into a single editable string. */
  function textFromParagraphs(paragraphs: ParagraphDto[]): string {
    return paragraphs
      .map((paragraph) => paragraph.runs.map((run) => run.text).join(''))
      .join('\n')
  }

  /** Concatenates the run text of a single paragraph. */
  function paragraphText(paragraph: ParagraphDto): string {
    return paragraph.runs.map((run) => run.text).join('')
  }

  /** Compares two paragraph styles for equality. */
  function paragraphStyleEqual(a: ParagraphStyleDto, b: ParagraphStyleDto): boolean {
    return (
      a.heading === b.heading &&
      a.blockquote === b.blockquote &&
      a.codeBlock === b.codeBlock &&
      a.codeStepRanges === b.codeStepRanges &&
      a.indentLevel === b.indentLevel
    )
  }

  /** Class name for a paragraph based on its style. */
  function paragraphClass(style: ParagraphStyleDto): string {
    const classes: string[] = []
    if (style.heading === 'h1') classes.push('heading-1')
    if (style.heading === 'h2') classes.push('heading-2')
    if (style.heading === 'h3') classes.push('heading-3')
    if (style.heading === 'h4') classes.push('heading-4')
    if (style.heading === 'h5') classes.push('heading-5')
    if (style.heading === 'h6') classes.push('heading-6')
    if (style.blockquote) classes.push('blockquote')
    if (style.codeBlock) classes.push('code-block')
    return classes.join(' ')
  }

  /** Class for a readonly paragraph, combining its base style with the active
   *  code-step highlight/dim state (line is 0-based paragraph index). */
  function readonlyParagraphClass(style: ParagraphStyleDto, lineIndex: number): string {
    const base = paragraphClass(style)
    if (!style.codeBlock) return base
    const ranges = style.codeStepRanges
    if (!ranges || ranges.trim() === '') return base
    const state = codeLineState(ranges, codeActiveStep, lineIndex + 1)
    const stepClass = state === 'active' ? 'code-step-active' : state === 'dimmed' ? 'code-step-dimmed' : ''
    return [base, stepClass].filter(Boolean).join(' ')
  }

  /** Class name for a run based on its style. */
  function runClass(run: RunDto): string {
    const classes: string[] = []
    if (run.bold) classes.push('bold')
    if (run.italic) classes.push('italic')
    if (run.underline) classes.push('underline')
    if (run.strikethrough) classes.push('strikethrough')
    if (run.verticalAlign === 'superscript') classes.push('superscript')
    if (run.verticalAlign === 'subscript') classes.push('subscript')
    if (run.code) classes.push('code')
    return classes.join(' ')
  }

  /** Builds a paragraph DTO, preserving original runs and style when the text is unchanged. */
  function buildParagraph(
    text: string,
    original: ParagraphDto | undefined,
  ): ParagraphDto {
    if (original && paragraphText(original) === text) {
      return original
    }
    return {
      runs: text
        ? [
            {
              text,
              bold: false,
              italic: false,
              underline: false,
              strikethrough: false,
              verticalAlign: 'baseline' as VerticalAlignDto,
              code: false,
            },
          ]
        : [],
      listStyle: original?.listStyle ?? 'none',
      style: original?.style ?? {
        blockquote: false,
        codeBlock: false,
        indentLevel: 0,
      },
    }
  }

  /** Emits a text-box edit command for the given textarea value, if it changed. */
  function commitTextBox(textarea: HTMLTextAreaElement, shapeIndex: number): void {
    if (!onEditTextBox) return
    const textBox = slide.shapes[shapeIndex].value as TextBoxSnapshot
    const originalParagraphs = textBox.paragraphs
    const lines = textarea.value.split('\n')

    const newParagraphs: ParagraphDto[] = lines.map((line, index) =>
      buildParagraph(line, originalParagraphs[index]),
    )

    const changed =
      newParagraphs.length !== originalParagraphs.length ||
      newParagraphs.some((paragraph, index) => {
        const original = originalParagraphs[index]
        if (!original) return true
        if (paragraph.runs.length !== original.runs.length) return true
        if (paragraph.listStyle !== original.listStyle) return true
        if (!paragraphStyleEqual(paragraph.style, original.style)) return true
        return paragraph.runs.some(
          (run, runIndex) =>
            run.text !== original.runs[runIndex]?.text ||
            run.bold !== original.runs[runIndex]?.bold ||
            run.italic !== original.runs[runIndex]?.italic ||
            run.underline !== original.runs[runIndex]?.underline ||
            run.strikethrough !== original.runs[runIndex]?.strikethrough ||
            run.verticalAlign !== original.runs[runIndex]?.verticalAlign ||
            run.code !== original.runs[runIndex]?.code ||
            run.fontFamily !== original.runs[runIndex]?.fontFamily,
        )
      })

    if (changed) {
      onEditTextBox({
        slideId: slide.id,
        shapeIndex,
        paragraphs: newParagraphs,
      })
    }
  }

  /** Emits a text-box edit command on blur. */
  function handleBlur(event: FocusEvent, shapeIndex: number): void {
    commitTextBox(event.target as HTMLTextAreaElement, shapeIndex)
  }

  /** Dash pattern in EMU for a given dash style, matching slides-render. */
  function dashArray(dash: DashStyleDto): string | undefined {
    switch (dash) {
      case 'dash':
        return '300000,150000'
      case 'dot':
        return '60000,60000'
      case 'dash_dot':
        return '300000,150000,60000,150000'
      default:
        return undefined
    }
  }

  /** Builds the `fill`, `stroke`, and `stroke-dasharray` attributes for a style. */
  function styleAttributes(style: StyleDto): string {
    let attrs = ''
    const fill = style.fill as FillDto | undefined
    if (fill && fill.solid) {
      attrs += ` fill="${toHex(fill.solid)}"`
    } else {
      attrs += ' fill="none"'
    }
    if (style.outline) {
      attrs += ` stroke="${toHex(style.outline.color)}" stroke-width="${style.outline.widthEmu}"`
      const dash = dashArray(style.outline.dash)
      if (dash) attrs += ` stroke-dasharray="${dash}"`
    }
    return attrs
  }

  /** Returns the transform attribute for a non-zero rotation around the frame center. */
  function rotateAttr(rotation: number, frameWidth: number, frameHeight: number): string {
    if (!rotation) return ''
    return ` transform="rotate(${rotation},${frameWidth / 2},${frameHeight / 2})"`
  }

  /** Builds the SVG path for a block arrow filling the frame, matching slides-render. */
  function arrowPath(w: number, h: number): string {
    const shaftTop = h / 3
    const shaftBot = (2 * h) / 3
    const headBase = (2 * w) / 3
    const tip = w
    const mid = h / 2
    return `M 0,${shaftTop} L ${headBase},${shaftTop} L ${headBase},0 L ${tip},${mid} L ${headBase},${h} L ${headBase},${shaftBot} L 0,${shaftBot} Z`
  }

  /** Builds the SVG path for a right-arrow callout, matching slides-render. */
  function rightArrowCalloutPath(w: number, h: number): string {
    const bodyRight = (2 * w) / 3
    const tip = w
    const mid = h / 2
    const q1 = h / 4
    const q3 = (3 * h) / 4
    return `M 0,0 L ${bodyRight},0 L ${bodyRight},${q1} L ${tip},${mid} L ${bodyRight},${q3} L ${bodyRight},${h} L 0,${h} Z`
  }

  /** Builds the SVG path for a five-pointed star, matching slides-render. */
  function star5Path(w: number, h: number): string {
    const cx = w / 2
    const cy = h / 2
    const outer = Math.min(w, h) / 2
    const inner = outer * 0.3819660112501051
    let d = 'M'
    for (let i = 0; i < 10; i += 1) {
      const angle = ((-90 + i * 36) * Math.PI) / 180
      const radius = i % 2 === 0 ? outer : inner
      const px = cx + radius * Math.cos(angle)
      const py = cy + radius * Math.sin(angle)
      d += ` ${px},${py}`
    }
    return `${d} Z`
  }

  /** Builds the inner SVG element for a geometry, sized to its frame in EMU. */
  function geometryMarkup(
    geometry: GeometryDto,
    style: StyleDto,
    frameWidth: number,
    frameHeight: number,
    rotation: number,
  ): string {
    const attrs = styleAttributes(style)
    const rotate = rotateAttr(rotation, frameWidth, frameHeight)
    let shape = ''
    if (geometry === 'rectangle') {
      shape = `<rect x="0" y="0" width="${frameWidth}" height="${frameHeight}"${attrs}${rotate}/>`
    } else if (geometry === 'ellipse') {
      shape = `<ellipse cx="${frameWidth / 2}" cy="${frameHeight / 2}" rx="${frameWidth / 2}" ry="${frameHeight / 2}"${attrs}${rotate}/>`
    } else if (geometry === 'triangle') {
      shape = `<polygon points="0,${frameHeight} ${frameWidth},${frameHeight} ${frameWidth / 2},0"${attrs}${rotate}/>`
    } else if (geometry === 'line') {
      shape = `<line x1="0" y1="${frameHeight / 2}" x2="${frameWidth}" y2="${frameHeight / 2}"${attrs}${rotate}/>`
    } else if (geometry === 'arrow') {
      shape = `<path d="${arrowPath(frameWidth, frameHeight)}"${attrs}${rotate}/>`
    } else if (geometry === 'right_arrow_callout') {
      shape = `<path d="${rightArrowCalloutPath(frameWidth, frameHeight)}"${attrs}${rotate}/>`
    } else if (geometry === 'star5') {
      shape = `<path d="${star5Path(frameWidth, frameHeight)}"${attrs}${rotate}/>`
    } else if (typeof geometry === 'object' && 'rounded_rectangle' in geometry) {
      const radius = geometry.rounded_rectangle.radius
      shape = `<rect x="0" y="0" width="${frameWidth}" height="${frameHeight}" rx="${radius}" ry="${radius}"${attrs}${rotate}/>`
    }
    return shape
  }

  /** Builds a complete `<svg>` document for a geometric shape. */
  function geometricSvg(shape: GeometricShapeSnapshot): string {
    const { frame } = shape.transform
    const inner = geometryMarkup(
      shape.geometry,
      shape.style,
      frame.width,
      frame.height,
      shape.transform.rotation,
    )
    let filterOpen = ''
    let filterClose = ''
    if (shape.style.shadow) {
      const s = shape.style.shadow
      filterOpen = `<defs><filter id="sh" x="-50%" y="-50%" width="200%" height="200%"><feDropShadow dx="${s.offsetX}" dy="${s.offsetY}" stdDeviation="${s.blur}" flood-color="${toHex(s.color)}" flood-opacity="${s.opacity}"/></filter></defs>`
      filterOpen += `<g filter="url(#sh)">`
      filterClose = `</g>`
    }
    return `<svg viewBox="0 0 ${frame.width} ${frame.height}" xmlns="http://www.w3.org/2000/svg" preserveAspectRatio="none">${filterOpen}${inner}${filterClose}</svg>`
  }

  /** Inner image style: fills the frame, or reveals only the cropped region. */
  function imageInnerStyle(crop: CropDto | undefined): string {
    if (!crop) {
      return 'width:100%;height:100%;object-fit:fill;'
    }
    const visW = Math.max(1e-6, 1 - crop.left - crop.right)
    const visH = Math.max(1e-6, 1 - crop.top - crop.bottom)
    const scaledW = 100 / visW
    const scaledH = 100 / visH
    const offsetX = crop.left * scaledW
    const offsetY = crop.top * scaledH
    return `position:absolute;width:${scaledW}%;height:${scaledH}%;left:-${offsetX}%;top:-${offsetY}%;object-fit:fill;`
  }

  /** Background color for a cell: cell fill, header default, or transparent. */
  function cellBackground(cell: TableCellDto, isHeader: boolean): string {
    if (cell.fill && cell.fill.solid) return toHex(cell.fill.solid)
    if (isHeader) return '#d9e1f2'
    return 'transparent'
  }

  /** Maps a dash style to a CSS border-style keyword. */
  function dashToCss(dash: DashStyleDto): string {
    switch (dash) {
      case 'dash':
        return 'dashed'
      case 'dot':
        return 'dotted'
      case 'dash_dot':
        return 'dashed'
      default:
        return 'solid'
    }
  }

  /** CSS shorthand for a single border edge. */
  function edgeBorder(edge: BorderEdgeDto): string {
    return `${edge.widthEmu * EMU_TO_PX}px ${dashToCss(edge.dash)} ${toHex(edge.color)}`
  }

  /** CSS `border` declaration for a cell, honoring per-cell or default borders. */
  function cellBorders(cell: TableCellDto, defaults: TableBordersDto): string {
    const borders = cell.borders ?? defaults
    return [
      `border-top:${borders.top ? edgeBorder(borders.top) : 'none'}`,
      `border-bottom:${borders.bottom ? edgeBorder(borders.bottom) : 'none'}`,
      `border-left:${borders.left ? edgeBorder(borders.left) : 'none'}`,
      `border-right:${borders.right ? edgeBorder(borders.right) : 'none'}`,
    ].join(';')
  }

  /** CSS `text-align` for a cell alignment. */
  function cellTextAlign(align: CellAlignDto): string {
    return align
  }

  /** Cumulative column left edges, in EMU, starting at 0. */
  function columnOffsets(widths: number[]): number[] {
    const offsets: number[] = []
    let acc = 0
    for (const w of widths) {
      offsets.push(acc)
      acc += w
    }
    return offsets
  }

  /** Cumulative row top edges, in EMU, starting at 0. */
  function rowOffsets(rows: { height: number }[]): number[] {
    const offsets: number[] = []
    let acc = 0
    for (const row of rows) {
      offsets.push(acc)
      acc += row.height
    }
    return offsets
  }

  /** Emits a cell-text edit command if the cell value changed on blur. */
  function handleCellBlur(
    event: FocusEvent,
    shapeIndex: number,
    row: number,
    col: number,
    original: string,
  ): void {
    if (!onSetCellText) return
    const target = event.target as HTMLTextAreaElement
    if (target.value !== original) {
      onSetCellText({ slideId: slide.id, shapeIndex, row, col, text: target.value })
    }
  }

  /** Cache for rendered chart SVGs, keyed by a stable shape key. */
  const chartSvgCache = new Map<string, string>()

  // --- Spell-check (Wave 6, component 2) -------------------------------------
  // Squiggles are rendered in a transparent overlay positioned exactly behind
  // the textarea; the textarea itself stays fully editable. Spell-check runs
  // via async invoke on a debounce so it never blocks typing.

  /** Live (possibly uncommitted) textarea text per text-box shape index. */
  let liveText = $state<Record<number, string>>({})
  /** Last spell-check result per shape, paired with the exact text it covers. */
  let spellChecked = $state<Record<number, { text: string; errors: MisspellingDto[] }>>({})
  /** Per-shape debounce timers and tokens (cancel stale checks on new input). */
  const spellTimers = new Map<number, ReturnType<typeof setTimeout>>()
  const spellTokens = new Map<number, number>()
  /** Spell-check debounce window in milliseconds. */
  const SPELL_DEBOUNCE_MS = 350
  /** Maximum suggestions requested per word. */
  const SPELL_MAX_SUGGESTIONS = 5

  /** Open spell-check context menu state (null when closed). */
  let spellMenu = $state<{
    word: string
    start: number
    end: number
    x: number
    y: number
    suggestions: string[]
  } | null>(null)
  /** Textbox the open menu targets (kept outside reactive state on purpose). */
  let spellMenuTextarea: HTMLTextAreaElement | null = null
  let spellMenuShapeIndex = 0

  /** Tracks the rendered slide so per-shape spell state resets on slide change. */
  let lastSlideId = ''
  /** Root canvas element, used to keep the squiggle overlay scroll in sync. */
  let canvasEl: HTMLElement

  $effect(() => {
    if (slide.id === lastSlideId) return
    lastSlideId = slide.id
    liveText = {}
    spellChecked = {}
    spellMenu = null
    spellMenuTextarea = null
    // Seed an initial check for every text box so squiggles appear on load.
    for (let index = 0; index < slide.shapes.length; index += 1) {
      const shape = slide.shapes[index]
      if (shape.kind === 'text_box') {
        const tb = shape.value as TextBoxSnapshot
        scheduleSpellCheck(index, textFromParagraphs(tb.paragraphs))
      }
    }
  })

  // Re-applies each textarea's scroll offset to its squiggle overlay after the
  // overlay content is re-rendered (which resets scrollTop to 0).
  $effect(() => {
    void spellChecked
    void liveText
    if (!canvasEl) return
    for (const overlay of Array.from(canvasEl.querySelectorAll<HTMLElement>('.text-box-overlay'))) {
      const textarea = overlay.parentElement?.querySelector('textarea')
      if (textarea instanceof HTMLTextAreaElement) {
        overlay.scrollTop = textarea.scrollTop
        overlay.scrollLeft = textarea.scrollLeft
      }
    }
  })

  /** Maps a UTF-8 byte offset (as returned by the checker) to a JS string index. */
  function byteOffsetToChar(text: string, byteOffset: number): number {
    if (byteOffset <= 0) return 0
    const encoder = new TextEncoder()
    let bytes = 0
    let i = 0
    while (i < text.length) {
      if (bytes >= byteOffset) return i
      const cp = text.codePointAt(i) as number
      bytes += encoder.encode(String.fromCodePoint(cp)).length
      i += cp > 0xffff ? 2 : 1
    }
    return text.length
  }

  /** Escapes the HTML-significant characters in a string. */
  function escapeHtml(s: string): string {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  }

  /** Builds the overlay HTML: escaped text with misspelled words wrapped in
   *  spans carrying their JS char offsets for hit-testing. */
  function overlayMarkup(text: string, errors: MisspellingDto[]): string {
    const spans = errors
      .map((e) => ({
        start: byteOffsetToChar(text, e.byteStart),
        end: byteOffsetToChar(text, e.byteEnd),
      }))
      .filter((s) => s.start <= s.end)
      .sort((a, b) => a.start - b.start)

    let html = ''
    let pos = 0
    for (const span of spans) {
      if (span.start < pos) continue
      html += escapeHtml(text.slice(pos, span.start))
      const word = text.slice(span.start, span.end)
      html += `<span class="misspelled" data-start="${span.start}" data-end="${span.end}">${escapeHtml(word)}</span>`
      pos = span.end
    }
    html += escapeHtml(text.slice(pos))
    return html
  }

  /** Overlay markup for a text box: the live text with squiggles when the last
   *  check matches the current text (avoids misaligned stale squiggles). */
  function textBoxOverlay(shapeIndex: number, paragraphs: ParagraphDto[]): string {
    const text = liveText[shapeIndex] ?? textFromParagraphs(paragraphs)
    const checked = spellChecked[shapeIndex]
    const errors = checked && checked.text === text ? checked.errors : []
    return overlayMarkup(text, errors)
  }

  /** Schedules a debounced spell-check for a text box. */
  function scheduleSpellCheck(shapeIndex: number, text: string): void {
    const prev = spellTimers.get(shapeIndex)
    if (prev !== undefined) clearTimeout(prev)
    const token = (spellTokens.get(shapeIndex) ?? 0) + 1
    spellTokens.set(shapeIndex, token)
    const timer = setTimeout(async () => {
      if (spellTokens.get(shapeIndex) !== token) return
      try {
        const errors = await invoke<MisspellingDto[]>('spell_check', { text })
        if (spellTokens.get(shapeIndex) !== token) return
        spellChecked = { ...spellChecked, [shapeIndex]: { text, errors } }
      } catch {
        // Never block editing on a spell-check failure.
      }
    }, SPELL_DEBOUNCE_MS)
    spellTimers.set(shapeIndex, timer)
  }

  /** Handles textarea input: records live text and schedules a check. */
  function handleInput(event: Event, shapeIndex: number): void {
    const textarea = event.currentTarget as HTMLTextAreaElement
    liveText = { ...liveText, [shapeIndex]: textarea.value }
    syncScroll(event)
    scheduleSpellCheck(shapeIndex, textarea.value)
  }

  /** Mirrors the textarea scroll position onto the squiggle overlay. */
  function syncScroll(event: Event): void {
    const textarea = event.currentTarget as HTMLTextAreaElement
    const overlay = textarea.parentElement?.querySelector('.text-box-overlay') as HTMLElement | null
    if (overlay) {
      overlay.scrollTop = textarea.scrollTop
      overlay.scrollLeft = textarea.scrollLeft
    }
  }

  /** Hit-tests the click point against rendered misspelled-word spans. */
  function findMisspellingAt(
    textarea: HTMLTextAreaElement,
    x: number,
    y: number,
  ): { word: string; start: number; end: number } | null {
    const overlay = textarea.parentElement?.querySelector('.text-box-overlay')
    if (!overlay) return null
    const spans = Array.from(overlay.querySelectorAll<HTMLElement>('.misspelled'))
    for (const span of spans) {
      const rect = span.getBoundingClientRect()
      if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
        return {
          word: span.textContent ?? '',
          start: Number(span.dataset.start ?? '0'),
          end: Number(span.dataset.end ?? '0'),
        }
      }
    }
    return null
  }

  /** Opens the spell-check context menu for the misspelled word under the cursor. */
  async function openSpellMenu(
    textarea: HTMLTextAreaElement,
    shapeIndex: number,
    word: string,
    start: number,
    end: number,
    x: number,
    y: number,
  ): Promise<void> {
    let suggestions: string[] = []
    try {
      suggestions = await invoke<string[]>('spell_suggest', {
        word,
        max: SPELL_MAX_SUGGESTIONS,
      })
    } catch {
      suggestions = []
    }
    spellMenuTextarea = textarea
    spellMenuShapeIndex = shapeIndex
    spellMenu = { word, start, end, x, y, suggestions }
  }

  /** Right-click handler: shows the spell menu only on a misspelled word. */
  function handleContextMenu(event: MouseEvent, shapeIndex: number): void {
    if (readonly) return
    const textarea = event.currentTarget as HTMLTextAreaElement
    const hit = findMisspellingAt(textarea, event.clientX, event.clientY)
    if (!hit) return
    event.preventDefault()
    void openSpellMenu(textarea, shapeIndex, hit.word, hit.start, hit.end, event.clientX, event.clientY)
  }

  /** Replaces the misspelled word with the chosen suggestion and re-checks. */
  function applySuggestion(replacement: string): void {
    const menu = spellMenu
    const textarea = spellMenuTextarea
    spellMenu = null
    spellMenuTextarea = null
    if (!menu || !textarea) return
    const text = textarea.value
    const next = text.slice(0, menu.start) + replacement + text.slice(menu.end)
    textarea.value = next
    const caret = menu.start + replacement.length
    textarea.focus()
    textarea.setSelectionRange(caret, caret)
    liveText = { ...liveText, [spellMenuShapeIndex]: next }
    commitTextBox(textarea, spellMenuShapeIndex)
    scheduleSpellCheck(spellMenuShapeIndex, next)
  }

  /** Learns the misspelled word into the user dictionary and re-checks. */
  async function addToDictionary(): Promise<void> {
    const menu = spellMenu
    const textarea = spellMenuTextarea
    spellMenu = null
    spellMenuTextarea = null
    if (!menu || !textarea) return
    try {
      await invoke('spell_add_word', { word: menu.word })
    } catch {
      // Persistence failure should not crash the editor.
    }
    const text = textarea.value
    liveText = { ...liveText, [spellMenuShapeIndex]: text }
    scheduleSpellCheck(spellMenuShapeIndex, text)
  }

  /** Closes the spell-check context menu. */
  function closeSpellMenu(): void {
    spellMenu = null
    spellMenuTextarea = null
  }

  /** Builds a stable cache key for a chart shape. */
  function chartKey(shapeIndex: number, chart: ChartShapeSnapshot): string {
    return `${slide.id}:${shapeIndex}:${chart.chartType}:${chart.title ?? ''}:${JSON.stringify(chart.data)}`
  }

  /**
   * Renders the slide SVG and extracts the nested <svg> for the chart at the
   * given shape index. The chart is identified by counting chart shapes before
   * `shapeIndex` in the slide.
   */
  async function chartSvg(shapeIndex: number, chart: ChartShapeSnapshot): Promise<string> {
    const key = chartKey(shapeIndex, chart)
    const cached = chartSvgCache.get(key)
    if (cached !== undefined) return cached

    const svg = await invoke<string>('render_slide_svg', {
      slide_id: slide.id,
      code_active_step: codeActiveStep,
    })
    const parsed = new DOMParser().parseFromString(svg, 'image/svg+xml')
    const svgs = Array.from(parsed.querySelectorAll('svg'))
    // The first <svg> is the slide root; nested <svg> elements are charts.
    const chartIndex = slide.shapes
      .slice(0, shapeIndex)
      .filter((s) => s.kind === 'chart').length
    const nested = svgs[chartIndex + 1]
    if (!nested) {
      throw new Error('chart SVG not found in rendered slide')
    }
    const clone = nested.cloneNode(true) as SVGSVGElement
    clone.removeAttribute('x')
    clone.removeAttribute('y')
    clone.setAttribute('width', '100%')
    clone.setAttribute('height', '100%')
    const serializer = new XMLSerializer()
    const result = serializer.serializeToString(clone)
    chartSvgCache.set(key, result)
    return result
  }
</script>

<div
  class="canvas"
  class:high-contrast={highContrast}
  bind:this={canvasEl}
  style:width={canvasWidthPx}
  style:height={canvasHeightPx}
  style:background-color={toRgba(effectiveBackground)}
  role="application"
  aria-label="Slide canvas"
>
  {#each slide.shapes as shape, shapeIndex}
    {#if shape.kind === 'text_box'}
      {@const textBox = shape.value as TextBoxSnapshot}
      <div
        class="text-box-container"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        data-shape-id={textBox.id}
        style:left={toPx(textBox.frame.x)}
        style:top={toPx(textBox.frame.y)}
        style:width={toPx(textBox.frame.width)}
        style:height={toPx(textBox.frame.height)}
        style:opacity={buildStateFor(shapeIndex)?.opacity}
        style:visibility={buildStateFor(shapeIndex)?.visibility}
        style:transform={buildStateFor(shapeIndex)?.transform}
        style:transition={buildStateFor(shapeIndex)?.transition}
      >
        {#if readonly}
          <div class="text-box-readonly">
            {#each textBox.paragraphs as paragraph, pIndex}
              <p class={readonlyParagraphClass(paragraph.style, pIndex)}>
                {#each paragraph.runs as run}
                  <span class={runClass(run)}>{run.text}</span>
                {/each}
              </p>
            {/each}
          </div>
        {:else}
          <div class="text-box-editor">
            <div class="text-box-overlay" aria-hidden="true">
              {@html textBoxOverlay(shapeIndex, textBox.paragraphs)}
            </div>
            <textarea
              class="text-box"
              data-slide-id={slide.id}
              data-shape-index={shapeIndex}
              value={textFromParagraphs(textBox.paragraphs)}
              oninput={(event) => handleInput(event, shapeIndex)}
              onscroll={syncScroll}
              oncontextmenu={(event) => handleContextMenu(event, shapeIndex)}
              onblur={(event) => {
                handleBlur(event, shapeIndex)
                const cleared = { ...liveText }
                delete cleared[shapeIndex]
                liveText = cleared
              }}
              aria-label="Editable text box"
            ></textarea>
          </div>
        {/if}
      </div>
    {:else if shape.kind === 'passthrough'}
      {@const obj = shape.value as PassthroughSnapshot}
      {@const passthroughIndex = slide.shapes.filter((s, i) => s.kind === 'passthrough' && i < shapeIndex).length}
      <div
        class="passthrough"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        data-shape-id={obj.id}
        style:left={obj.frame ? toPx(obj.frame.x) : undefined}
        style:top={obj.frame ? toPx(obj.frame.y) : `${1 + passthroughIndex * 0.5}rem`}
        style:right={obj.frame ? undefined : '1rem'}
        style:width={obj.frame ? toPx(obj.frame.width) : undefined}
        style:height={obj.frame ? toPx(obj.frame.height) : undefined}
        style:opacity={buildStateFor(shapeIndex)?.opacity}
        style:visibility={buildStateFor(shapeIndex)?.visibility}
        style:transform={buildStateFor(shapeIndex)?.transform}
        style:transition={buildStateFor(shapeIndex)?.transition}
      >
        [preserved object: {obj.label}]
      </div>
    {:else if shape.kind === 'image'}
      {@const image = shape.value as ImageShapeSnapshot}
      {@const entry = media?.[image.mediaRef]}
      {@const frame = image.transform.frame}
      {@const rotation = image.transform.rotation}
      <div
        class="image-container"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        data-shape-id={image.id}
        style:left={toPx(frame.x)}
        style:top={toPx(frame.y)}
        style:width={toPx(frame.width)}
        style:height={toPx(frame.height)}
        style:transform={imageTransform(rotation, shapeIndex)}
        style:opacity={buildStateFor(shapeIndex)?.opacity}
        style:visibility={buildStateFor(shapeIndex)?.visibility}
        style:transition={buildStateFor(shapeIndex)?.transition}
      >
        {#if entry}
          <img
            class="image"
            style={imageInnerStyle(image.crop)}
            src={dataUri(entry)}
            alt=""
            draggable="false"
          />
        {:else}
          <div class="image-missing">Missing image: {image.mediaRef}</div>
        {/if}
      </div>
    {:else if shape.kind === 'geometric'}
      {@const geometric = shape.value as GeometricShapeSnapshot}
      {@const frame = geometric.transform.frame}
      <div
        class="geometric-container"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        data-shape-id={geometric.id}
        style:left={toPx(frame.x)}
        style:top={toPx(frame.y)}
        style:width={toPx(frame.width)}
        style:height={toPx(frame.height)}
        style:opacity={buildStateFor(shapeIndex)?.opacity}
        style:visibility={buildStateFor(shapeIndex)?.visibility}
        style:transform={buildStateFor(shapeIndex)?.transform}
        style:transition={buildStateFor(shapeIndex)?.transition}
      >
        {@html geometricSvg(geometric)}
      </div>
    {:else if shape.kind === 'table'}
      {@const table = shape.value as TableShapeSnapshot}
      {@const tframe = table.transform.frame}
      {@const trot = table.transform.rotation}
      {@const colX = columnOffsets(table.columnWidths)}
      {@const rowY = rowOffsets(table.rows)}
      <div
        class="table-container"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        data-shape-id={table.id}
        style:left={toPx(tframe.x)}
        style:top={toPx(tframe.y)}
        style:width={toPx(tframe.width)}
        style:height={toPx(tframe.height)}
        style:transform={tableTransform(trot, shapeIndex)}
        style:opacity={buildStateFor(shapeIndex)?.opacity}
        style:visibility={buildStateFor(shapeIndex)?.visibility}
        style:transition={buildStateFor(shapeIndex)?.transition}
      >
        {#each table.rows as row, rowIndex}
          {@const isHeader = table.headerRow && rowIndex === 0}
          {#each row.cells as cell, colIndex}
            {@const cleft = colX[colIndex] ?? 0}
            {@const ctop = rowY[rowIndex] ?? 0}
            {@const cwidth = table.columnWidths[colIndex] ?? 0}
            {@const cheight = row.height}
            <div
              class="table-cell"
              style:left={toPx(cleft)}
              style:top={toPx(ctop)}
              style:width={toPx(cwidth)}
              style:height={toPx(cheight)}
              style:background-color={cellBackground(cell, isHeader)}
              style:font-weight={isHeader ? 'bold' : undefined}
              style={cellBorders(cell, table.defaultBorders)}
            >
              {#if readonly}
                <div class="table-cell-text" style:text-align={cellTextAlign(cell.align)}>
                  {cell.text}
                </div>
              {:else}
                <textarea
                  class="table-cell-input"
                  data-slide-id={slide.id}
                  data-shape-index={shapeIndex}
                  data-row={rowIndex}
                  data-col={colIndex}
                  style:text-align={cellTextAlign(cell.align)}
                  value={cell.text}
                  onfocus={() => onCellFocus?.({ shapeIndex, row: rowIndex, col: colIndex })}
                  onblur={(event) => handleCellBlur(event, shapeIndex, rowIndex, colIndex, cell.text)}
                  aria-label={`Cell row ${rowIndex + 1} column ${colIndex + 1}`}
                ></textarea>
              {/if}
            </div>
          {/each}
        {/each}
      </div>
    {:else if shape.kind === 'chart'}
      {@const chart = shape.value as ChartShapeSnapshot}
      {@const frame = chart.transform.frame}
      <div
        class="chart-container"
        class:chart-readonly={readonly}
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        data-shape-id={chart.id}
        style:left={toPx(frame.x)}
        style:top={toPx(frame.y)}
        style:width={toPx(frame.width)}
        style:height={toPx(frame.height)}
        style:opacity={buildStateFor(shapeIndex)?.opacity}
        style:visibility={buildStateFor(shapeIndex)?.visibility}
        style:transform={buildStateFor(shapeIndex)?.transform}
        style:transition={buildStateFor(shapeIndex)?.transition}
        ondblclick={() => !readonly && onEditChart?.({ slideId: slide.id, shapeIndex })}
        role="img"
        aria-label={chart.title ? `Chart: ${chart.title}` : 'Chart'}
      >
        {#await chartSvg(shapeIndex, chart)}
          <div class="chart-loading">Loading chart…</div>
        {:then svg}
          {@html svg}
        {:catch}
          <div class="chart-fallback">Chart</div>
        {/await}
      </div>
    {/if}
  {/each}
</div>

{#if spellMenu}
  <button
    type="button"
    class="spell-menu-backdrop"
    aria-label="Close spell-check menu"
    onclick={closeSpellMenu}
  ></button>
  <div
    class="spell-menu"
    style:left={`${spellMenu.x}px`}
    style:top={`${spellMenu.y}px`}
    role="menu"
    aria-label="Spell-check suggestions"
  >
    {#each spellMenu.suggestions as suggestion}
      <button
        type="button"
        class="spell-menu-item"
        role="menuitem"
        onclick={() => applySuggestion(suggestion)}
      >
        {suggestion}
      </button>
    {:else}
      <div class="spell-menu-empty">No suggestions</div>
    {/each}
    <div class="spell-menu-divider"></div>
    <button
      type="button"
      class="spell-menu-item spell-menu-add"
      role="menuitem"
      onclick={addToDictionary}
    >
      Add &ldquo;{spellMenu.word}&rdquo; to dictionary
    </button>
  </div>
{/if}

<style>
  .canvas {
    position: relative;
    flex-shrink: 0;
    box-shadow: 0 0 0 1px #ccc;
    overflow: hidden;
  }
  .canvas.high-contrast .text-box,
  .canvas.high-contrast .table-cell-input,
  .canvas.high-contrast .table-cell-text {
    color: #ffffff;
  }
  .canvas.high-contrast .text-box-readonly {
    color: #ffffff;
  }
  .canvas.high-contrast .text-box:focus {
    outline: 2px solid #ffd700;
  }
  .canvas.high-contrast .table-cell-input:focus {
    outline: 1px solid #ffd700;
  }
  .text-box-container {
    position: absolute;
  }
  .text-box-editor {
    position: relative;
    width: 100%;
    height: 100%;
  }
  .text-box-overlay {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    margin: 0;
    border: 1px solid transparent;
    padding: 0.25rem;
    font-family: inherit;
    font-size: 1rem;
    line-height: 1.3;
    white-space: pre-wrap;
    overflow-wrap: break-word;
    color: transparent;
    overflow: hidden;
    pointer-events: none;
    z-index: 0;
  }
  .text-box-overlay :global(.misspelled) {
    text-decoration: underline wavy #d00;
    text-decoration-skip-ink: none;
  }
  .text-box {
    position: relative;
    z-index: 1;
  }
  .text-box {
    width: 100%;
    height: 100%;
    border: 1px dashed #999;
    background: transparent;
    resize: none;
    font-family: inherit;
    font-size: 1rem;
    line-height: 1.3;
    padding: 0.25rem;
  }
  .text-box:focus {
    outline: 2px solid #0070c0;
  }
  .text-box-readonly {
    width: 100%;
    height: 100%;
    padding: 0.25rem;
  }
  .text-box-readonly p {
    margin: 0 0 0.5rem;
  }
  .bold {
    font-weight: bold;
  }
  .italic {
    font-style: italic;
  }
  .underline {
    text-decoration: underline;
  }
  .strikethrough {
    text-decoration: line-through;
  }
  .superscript {
    vertical-align: super;
    font-size: 0.7em;
  }
  .subscript {
    vertical-align: sub;
    font-size: 0.7em;
  }
  .code {
    font-family: 'Courier New', monospace;
  }
  .text-box-readonly p.heading-1 {
    font-size: 2rem;
    font-weight: bold;
  }
  .text-box-readonly p.heading-2 {
    font-size: 1.5rem;
    font-weight: bold;
  }
  .text-box-readonly p.heading-3 {
    font-size: 1.25rem;
    font-weight: bold;
  }
  .text-box-readonly p.heading-4 {
    font-size: 1.1rem;
    font-weight: bold;
  }
  .text-box-readonly p.heading-5 {
    font-size: 1rem;
    font-weight: bold;
  }
  .text-box-readonly p.heading-6 {
    font-size: 0.9rem;
    font-weight: bold;
  }
  .text-box-readonly p.blockquote {
    font-style: italic;
    border-left: 3px solid #ccc;
    padding-left: 0.5rem;
    margin-left: 0;
  }
  .text-box-readonly p.code-block {
    font-family: 'Courier New', monospace;
    background: #f5f5f5;
    padding: 0.25rem;
  }
  .text-box-readonly p.code-step-active {
    background: #fff3cd;
  }
  .text-box-readonly p.code-step-dimmed {
    opacity: 0.4;
  }
  .passthrough {
    position: absolute;
    padding: 0.5rem;
    border: 2px dashed #c00;
    color: #c00;
    background: rgba(255, 255, 255, 0.9);
  }
  .image-container {
    position: absolute;
    overflow: hidden;
  }
  .image {
    display: block;
  }
  .image-missing {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #ccc;
    color: #444;
    border: 1px solid #666;
    font-size: 0.8rem;
    text-align: center;
    padding: 0.25rem;
  }
  .geometric-container {
    position: absolute;
  }
  .geometric-container :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
    overflow: visible;
  }
  .table-container {
    position: absolute;
    box-sizing: content-box;
  }
  .table-cell {
    position: absolute;
    box-sizing: border-box;
    overflow: hidden;
  }
  .table-cell-input {
    width: 100%;
    height: 100%;
    border: none;
    background: transparent;
    resize: none;
    padding: 0.1rem 0.2rem;
    font-family: inherit;
    font-size: 0.85rem;
    line-height: 1.2;
    outline: none;
  }
  .table-cell-input:focus {
    outline: 1px solid #0070c0;
  }
  .table-cell-text {
    width: 100%;
    height: 100%;
    padding: 0.1rem 0.2rem;
    font-size: 0.85rem;
    line-height: 1.2;
    white-space: pre-wrap;
    overflow: hidden;
  }
  .chart-container {
    position: absolute;
    background: #fff;
    cursor: pointer;
  }
  .chart-container:not(.chart-readonly):hover {
    outline: 1px dashed #0070c0;
  }
  .chart-container :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
  .chart-loading,
  .chart-fallback {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #666;
    font-size: 0.85rem;
  }
  .spell-menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .spell-menu {
    position: fixed;
    z-index: 51;
    min-width: 160px;
    max-width: 280px;
    padding: 0.2rem 0;
    background: #fff;
    border: 1px solid #ccc;
    border-radius: 4px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.2);
    display: flex;
    flex-direction: column;
  }
  .spell-menu-item {
    text-align: left;
    background: none;
    border: none;
    padding: 0.35rem 0.75rem;
    font-family: inherit;
    font-size: 0.85rem;
    cursor: pointer;
    color: #222;
  }
  .spell-menu-item:hover,
  .spell-menu-item:focus-visible {
    background: #e8f0fe;
  }
  .spell-menu-empty {
    padding: 0.35rem 0.75rem;
    font-size: 0.85rem;
    color: #888;
  }
  .spell-menu-divider {
    height: 1px;
    margin: 0.2rem 0;
    background: #e5e5e5;
  }
  .spell-menu-add {
    color: #444;
  }
</style>
