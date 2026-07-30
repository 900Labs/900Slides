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
    PlaceholderDefDto,
    RectDto,
    RunDto,
    ShapeSnapshot,
    SlideSizeDto,
    SlideSnapshot,
    StyleDto,
    TransformDto,
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
    /** Callback invoked when a shape is clicked to select it. Pass `null` to
     *  clear the selection (click on empty canvas). */
    onSelectShape?: (detail: { shapeIndex: number | null }) => void
    /** Callback invoked when a text box enters (`index`) or leaves (`null`)
     *  text-edit mode. */
    onEditShape?: (detail: { shapeIndex: number | null }) => void
    /** Callback invoked to commit a shape's new transform after a drag/resize. */
    onUpdateShapeTransform?: (detail: { shapeIndex: number; transform: TransformDto }) => void
    /** Callback invoked when a non-text shape is right-clicked, so the host can
     *  offer an "Add comment" (shape-anchored) action. */
    onShapeContextMenu?: (detail: { shapeId: string; shapeIndex: number; x: number; y: number }) => void
    /** Callback invoked when the user right-clicks a text selection, so the host
     *  can offer a "Comment on selection" (text-range-anchored) action. The
     *  offsets are UTF-8 byte offsets into the text box's concatenated text. */
    onCommentOnSelection?: (detail: {
      shapeId: string
      shapeIndex: number
      start: number
      end: number
      x: number
      y: number
    }) => void
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
    /** Index of a shape to highlight as selected (e.g. from the accessibility
     *  panel). `null` (or omitted) draws no selection ring. */
    selectedShapeIndex?: number | null
    /** Index of a text box currently in text-edit mode (textarea visible), or
     *  `null` when none is being edited. */
    editingShapeIndex?: number | null
    /** Placeholder frames for the slide's active layout, drawn as non-editable
     *  guide outlines in the editor. Omitted or empty draws no guides. */
    placeholderGuides?: PlaceholderDefDto[]
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
    onEditShape,
    onUpdateShapeTransform,
    onShapeContextMenu,
    onCommentOnSelection,
    readonly = false,
    activeBuildStep = Infinity,
    codeActiveStep = 0,
    slideSize,
    highContrast = false,
    selectedShapeIndex = null,
    editingShapeIndex = null,
    placeholderGuides,
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

  /** Open "comment on selection" menu state (null when closed). */
  let commentMenu = $state<{
    shapeId: string
    shapeIndex: number
    start: number
    end: number
    x: number
    y: number
  } | null>(null)

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

  /** Maps a JS string index to a UTF-8 byte offset (for comment text ranges). */
  function charOffsetToByte(text: string, charIndex: number): number {
    return new TextEncoder().encode(text.slice(0, Math.max(0, charIndex))).length
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

  /** Right-click handler: shows the spell menu on a misspelled word, or offers
   *  a "Comment on selection" action when text is selected. */
  function handleContextMenu(event: MouseEvent, shapeIndex: number): void {
    if (readonly) return
    const textarea = event.currentTarget as HTMLTextAreaElement
    const hit = findMisspellingAt(textarea, event.clientX, event.clientY)
    if (hit) {
      event.preventDefault()
      void openSpellMenu(textarea, shapeIndex, hit.word, hit.start, hit.end, event.clientX, event.clientY)
      return
    }
    if (onCommentOnSelection) {
      const selStart = textarea.selectionStart ?? 0
      const selEnd = textarea.selectionEnd ?? 0
      if (selStart !== selEnd) {
        event.preventDefault()
        const shape = slide.shapes[shapeIndex]
        const shapeId =
          shape && shape.kind === 'text_box' ? (shape.value as TextBoxSnapshot).id ?? '' : ''
        const text = textarea.value
        commentMenu = {
          shapeId,
          shapeIndex,
          start: charOffsetToByte(text, Math.min(selStart, selEnd)),
          end: charOffsetToByte(text, Math.max(selStart, selEnd)),
          x: event.clientX,
          y: event.clientY,
        }
      }
    }
  }

  /** Fires the "Comment on selection" callback with the captured text range. */
  function applyCommentOnSelection(): void {
    const menu = commentMenu
    commentMenu = null
    if (!menu) return
    onCommentOnSelection?.({
      shapeId: menu.shapeId,
      shapeIndex: menu.shapeIndex,
      start: menu.start,
      end: menu.end,
      x: menu.x,
      y: menu.y,
    })
  }

  /** Closes the "comment on selection" context menu. */
  function closeCommentMenu(): void {
    commentMenu = null
  }

  /** Right-click handler for non-text shapes: offers "Add comment". */
  function handleShapeContextMenu(
    event: MouseEvent,
    shapeId: string | undefined,
    shapeIndex: number,
  ): void {
    if (readonly || !onShapeContextMenu || !shapeId) return
    event.preventDefault()
    onShapeContextMenu({ shapeId, shapeIndex, x: event.clientX, y: event.clientY })
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

  // --- Canvas interaction (Wave 21): selection, drag-to-move, resize -------

  /** Inverse of `EMU_TO_PX`: converts CSS pixels back to EMU. */
  const PX_TO_EMU = 9525.0
  /** Pointer-drag movement threshold in CSS pixels (below this it is a click). */
  const DRAG_THRESHOLD_PX = 3
  /** Minimum shape side, in EMU, enforced while resizing. */
  const MIN_SIDE_EMU = 8 * PX_TO_EMU

  /** The eight resize-handle directions, clockwise from top-left. */
  type DragMode = 'move' | 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w'
  const HANDLE_DIRECTIONS: DragMode[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']

  /** Returns the bounding frame (EMU) for any shape kind, or undefined when the
   *  shape has no placeable frame. */
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

  /** Returns the rotation (degrees) for a shape kind (0 for text boxes). */
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

  /** The frame to render for a shape: the live drag preview while it is being
   *  dragged/resized, otherwise its stored frame. */
  function liveFrame(shapeIndex: number, fallback: RectDto | undefined): RectDto | undefined {
    if (dragPreview && dragPreview.shapeIndex === shapeIndex) return dragPreview.frame
    return fallback
  }

  /** Whether a shape should show its move cursor and resize handles. */
  function isInteractiveShape(shapeIndex: number): boolean {
    return (
      !readonly &&
      shapeIndex === selectedShapeIndex &&
      shapeIndex !== editingShapeIndex &&
      shapeFrameOf(slide.shapes[shapeIndex]) !== undefined
    )
  }

  /** Active pointer-drag session (imperative, not reactive). */
  let dragStart: {
    shapeIndex: number
    mode: DragMode
    startX: number
    startY: number
    startFrame: RectDto
    rotation: number
    moved: boolean
  } | null = null
  /** Live preview frame during a drag, driving optimistic re-rendering. */
  let dragPreview = $state<{ shapeIndex: number; frame: RectDto } | null>(null)

  /** Frame of the currently selected, non-edited shape, used to position the
   *  resize-handle overlay (follows the live drag preview). Null when no shape
   *  is selected, is being edited, or has no placeable frame. */
  const selectionFrame = $derived.by<RectDto | null>(() => {
    if (readonly || selectedShapeIndex === null || selectedShapeIndex === editingShapeIndex) {
      return null
    }
    const shape = slide.shapes[selectedShapeIndex]
    if (!shape) return null
    return liveFrame(selectedShapeIndex, shapeFrameOf(shape)) ?? shapeFrameOf(shape) ?? null
  })

  /** Computes a moved/resized frame for the given mode and EMU deltas. */
  function applyDrag(mode: DragMode, start: RectDto, dxEmu: number, dyEmu: number): RectDto {
    if (mode === 'move') {
      return { x: start.x + dxEmu, y: start.y + dyEmu, width: start.width, height: start.height }
    }
    let x = start.x
    let y = start.y
    let width = start.width
    let height = start.height
    if (mode.includes('e')) width = Math.max(MIN_SIDE_EMU, start.width + dxEmu)
    if (mode.includes('s')) height = Math.max(MIN_SIDE_EMU, start.height + dyEmu)
    if (mode.includes('w')) {
      width = Math.max(MIN_SIDE_EMU, start.width - dxEmu)
      x = start.x + (start.width - width)
    }
    if (mode.includes('n')) {
      height = Math.max(MIN_SIDE_EMU, start.height - dyEmu)
      y = start.y + (start.height - height)
    }
    return { x, y, width, height }
  }

  /** Begins a drag (move or resize) session for a shape. */
  function beginDrag(event: PointerEvent, shapeIndex: number, mode: DragMode): void {
    if (event.button !== 0) return
    const shape = slide.shapes[shapeIndex]
    const frame = shapeFrameOf(shape)
    if (!frame) return
    // Resize handles sit above the body; stop the event so the body's move
    // pointer-down handler does not also run.
    if (mode !== 'move') event.stopPropagation()
    dragStart = {
      shapeIndex,
      mode,
      startX: event.clientX,
      startY: event.clientY,
      startFrame: { ...frame },
      rotation: shapeRotationOf(shape),
      moved: false,
    }
    window.addEventListener('pointermove', onDragMove)
    window.addEventListener('pointerup', onDragUp)
  }

  /** Updates the live preview frame as the pointer moves. */
  function onDragMove(event: PointerEvent): void {
    if (!dragStart) return
    const dx = event.clientX - dragStart.startX
    const dy = event.clientY - dragStart.startY
    if (!dragStart.moved && Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) return
    dragStart.moved = true
    const frame = applyDrag(dragStart.mode, dragStart.startFrame, dx * PX_TO_EMU, dy * PX_TO_EMU)
    dragPreview = { shapeIndex: dragStart.shapeIndex, frame }
  }

  /** Ends the drag session, committing the transform if the pointer moved. */
  function onDragUp(): void {
    window.removeEventListener('pointermove', onDragMove)
    window.removeEventListener('pointerup', onDragUp)
    const start = dragStart
    const preview = dragPreview
    dragStart = null
    dragPreview = null
    if (!start || !start.moved || !preview) return
    onUpdateShapeTransform?.({
      shapeIndex: start.shapeIndex,
      transform: { frame: preview.frame, rotation: start.rotation },
    })
  }

  /** Pointer-down on a shape body: selects it and begins a move drag (unless
   *  the shape is currently being text-edited). */
  function onShapePointerDown(event: PointerEvent, shapeIndex: number): void {
    if (readonly) return
    onSelectShape?.({ shapeIndex })
    if (shapeIndex === editingShapeIndex) return
    beginDrag(event, shapeIndex, 'move')
  }

  /** Double-click on a text box: enters text-edit mode. */
  function onTextBoxDblClick(shapeIndex: number): void {
    if (readonly) return
    if (shapeIndex !== editingShapeIndex) onEditShape?.({ shapeIndex })
  }

  /** Click on the empty canvas background clears the selection. */
  function onCanvasClick(event: MouseEvent): void {
    if (readonly) return
    if (event.target === canvasEl) onSelectShape?.({ shapeIndex: null })
  }

  /** Escape while editing exits text-edit mode but keeps the shape selected. */
  function onEditKeydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return
    event.preventDefault()
    const target = event.currentTarget as HTMLTextAreaElement
    onEditShape?.({ shapeIndex: null })
    target.blur()
  }

  /** Reference to the textarea of the text box currently being edited. */
  let editBoxTextarea = $state<HTMLTextAreaElement | null>(null)

  // Focus the edited text box's textarea as soon as it mounts.
  $effect(() => {
    if (editingShapeIndex !== null && editBoxTextarea) {
      editBoxTextarea.focus()
    }
  })

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

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<div
  class="canvas"
  class:high-contrast={highContrast}
  bind:this={canvasEl}
  style:width={canvasWidthPx}
  style:height={canvasHeightPx}
  style:background-color={toRgba(effectiveBackground)}
  role="application"
  aria-label="Slide canvas"
  onclick={onCanvasClick}
>
  {#if !readonly && placeholderGuides && placeholderGuides.length > 0}
    {#each placeholderGuides as guide}
      <div
        class="placeholder-guide"
        style:left={toPx(guide.frame.x)}
        style:top={toPx(guide.frame.y)}
        style:width={toPx(guide.frame.width)}
        style:height={toPx(guide.frame.height)}
        aria-hidden="true"
      >
        <span class="placeholder-guide-label">{guide.name}</span>
      </div>
    {/each}
  {/if}
  {#each slide.shapes as shape, shapeIndex}
    {#if shape.kind === 'text_box'}
      {@const textBox = shape.value as TextBoxSnapshot}
      {@const frame = liveFrame(shapeIndex, textBox.frame) ?? textBox.frame}
      {@const isEditing = !readonly && shapeIndex === editingShapeIndex}
      {@const interactive = isInteractiveShape(shapeIndex)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="text-box-container"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        class:selected={shapeIndex === selectedShapeIndex}
        class:draggable={interactive}
        data-shape-id={textBox.id}
        style:left={toPx(frame.x)}
        style:top={toPx(frame.y)}
        style:width={toPx(frame.width)}
        style:height={toPx(frame.height)}
        style:opacity={buildStateFor(shapeIndex)?.opacity}
        style:visibility={buildStateFor(shapeIndex)?.visibility}
        style:transform={buildStateFor(shapeIndex)?.transform}
        style:transition={buildStateFor(shapeIndex)?.transition}
        onpointerdown={(event) => onShapePointerDown(event, shapeIndex)}
        ondblclick={() => onTextBoxDblClick(shapeIndex)}
        oncontextmenu={(event) => handleShapeContextMenu(event, textBox.id, shapeIndex)}
      >
        {#if isEditing}
          <div class="text-box-editor">
            <div class="text-box-overlay" aria-hidden="true">
              {@html textBoxOverlay(shapeIndex, textBox.paragraphs)}
            </div>
            <textarea
              class="text-box"
              bind:this={editBoxTextarea}
              data-slide-id={slide.id}
              data-shape-index={shapeIndex}
              value={textFromParagraphs(textBox.paragraphs)}
              oninput={(event) => handleInput(event, shapeIndex)}
              onscroll={syncScroll}
              onkeydown={onEditKeydown}
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
        {:else}
          <div class="text-box-readonly">
            {#each textBox.paragraphs as paragraph, pIndex}
              <p class={readonlyParagraphClass(paragraph.style, pIndex)}>
                {#each paragraph.runs as run}
                  <span class={runClass(run)}>{run.text}</span>
                {/each}
              </p>
            {/each}
          </div>
        {/if}
      </div>
    {:else if shape.kind === 'passthrough'}
      {@const obj = shape.value as PassthroughSnapshot}
      {@const passthroughIndex = slide.shapes.filter((s, i) => s.kind === 'passthrough' && i < shapeIndex).length}
      {@const interactive = isInteractiveShape(shapeIndex)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="passthrough"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        class:selected={shapeIndex === selectedShapeIndex}
        class:draggable={interactive}
        data-shape-id={obj.id}
        onpointerdown={(event) => onShapePointerDown(event, shapeIndex)}
        oncontextmenu={(event) => handleShapeContextMenu(event, obj.id, shapeIndex)}
        style:left={obj.frame ? toPx(liveFrame(shapeIndex, obj.frame)?.x ?? obj.frame.x) : undefined}
        style:top={obj.frame ? toPx(liveFrame(shapeIndex, obj.frame)?.y ?? obj.frame.y) : `${1 + passthroughIndex * 0.5}rem`}
        style:right={obj.frame ? undefined : '1rem'}
        style:width={obj.frame ? toPx(liveFrame(shapeIndex, obj.frame)?.width ?? obj.frame.width) : undefined}
        style:height={obj.frame ? toPx(liveFrame(shapeIndex, obj.frame)?.height ?? obj.frame.height) : undefined}
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
      {@const frame = liveFrame(shapeIndex, image.transform.frame) ?? image.transform.frame}
      {@const rotation = image.transform.rotation}
      {@const interactive = isInteractiveShape(shapeIndex)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="image-container"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        class:selected={shapeIndex === selectedShapeIndex}
        class:draggable={interactive}
        data-shape-id={image.id}
        onpointerdown={(event) => onShapePointerDown(event, shapeIndex)}
        oncontextmenu={(event) => handleShapeContextMenu(event, image.id, shapeIndex)}
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
      {@const frame = liveFrame(shapeIndex, geometric.transform.frame) ?? geometric.transform.frame}
      {@const interactive = isInteractiveShape(shapeIndex)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="geometric-container"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        class:selected={shapeIndex === selectedShapeIndex}
        class:draggable={interactive}
        data-shape-id={geometric.id}
        onpointerdown={(event) => onShapePointerDown(event, shapeIndex)}
        oncontextmenu={(event) => handleShapeContextMenu(event, geometric.id, shapeIndex)}
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
      {@const tframe = liveFrame(shapeIndex, table.transform.frame) ?? table.transform.frame}
      {@const trot = table.transform.rotation}
      {@const colX = columnOffsets(table.columnWidths)}
      {@const rowY = rowOffsets(table.rows)}
      {@const interactive = isInteractiveShape(shapeIndex)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="table-container"
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        class:selected={shapeIndex === selectedShapeIndex}
        class:draggable={interactive}
        data-shape-id={table.id}
        onpointerdown={(event) => onShapePointerDown(event, shapeIndex)}
        oncontextmenu={(event) => handleShapeContextMenu(event, table.id, shapeIndex)}
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
      {@const frame = liveFrame(shapeIndex, chart.transform.frame) ?? chart.transform.frame}
      {@const interactive = isInteractiveShape(shapeIndex)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="chart-container"
        class:chart-readonly={readonly}
        class:build-shape={buildStateFor(shapeIndex) !== undefined}
        class:selected={shapeIndex === selectedShapeIndex}
        class:draggable={interactive}
        data-shape-id={chart.id}
        onpointerdown={(event) => onShapePointerDown(event, shapeIndex)}
        oncontextmenu={(event) => handleShapeContextMenu(event, chart.id, shapeIndex)}
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

  {#if selectionFrame}
    <div
      class="selection-overlay"
      style:left={toPx(selectionFrame.x)}
      style:top={toPx(selectionFrame.y)}
      style:width={toPx(selectionFrame.width)}
      style:height={toPx(selectionFrame.height)}
    >
      {#each HANDLE_DIRECTIONS as dir}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="resize-handle"
          data-handle={dir}
          onpointerdown={(event) => beginDrag(event, selectedShapeIndex!, dir)}
        ></div>
      {/each}
    </div>
  {/if}
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

{#if commentMenu}
  <button
    type="button"
    class="spell-menu-backdrop"
    aria-label="Close comment menu"
    onclick={closeCommentMenu}
  ></button>
  <div
    class="spell-menu"
    style:left={`${commentMenu.x}px`}
    style:top={`${commentMenu.y}px`}
    role="menu"
    aria-label="Comment on selection"
  >
    <button
      type="button"
      class="spell-menu-item"
      role="menuitem"
      onclick={applyCommentOnSelection}
    >
      Comment on selection
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
  .selected {
    outline: 3px solid #0070c0;
    outline-offset: 2px;
    z-index: 5;
  }
  .draggable {
    cursor: move;
  }
  .resize-handle {
    position: absolute;
    width: 9px;
    height: 9px;
    background: #fff;
    border: 1px solid #0070c0;
    border-radius: 1px;
    box-sizing: border-box;
    z-index: 6;
    pointer-events: auto;
  }
  .selection-overlay {
    position: absolute;
    pointer-events: none;
    z-index: 6;
  }
  .resize-handle[data-handle='nw'] {
    left: -5px;
    top: -5px;
    cursor: nwse-resize;
  }
  .resize-handle[data-handle='n'] {
    left: calc(50% - 4.5px);
    top: -5px;
    cursor: ns-resize;
  }
  .resize-handle[data-handle='ne'] {
    right: -5px;
    top: -5px;
    cursor: nesw-resize;
  }
  .resize-handle[data-handle='e'] {
    right: -5px;
    top: calc(50% - 4.5px);
    cursor: ew-resize;
  }
  .resize-handle[data-handle='se'] {
    right: -5px;
    bottom: -5px;
    cursor: nwse-resize;
  }
  .resize-handle[data-handle='s'] {
    left: calc(50% - 4.5px);
    bottom: -5px;
    cursor: ns-resize;
  }
  .resize-handle[data-handle='sw'] {
    left: -5px;
    bottom: -5px;
    cursor: nesw-resize;
  }
  .resize-handle[data-handle='w'] {
    left: -5px;
    top: calc(50% - 4.5px);
    cursor: ew-resize;
  }
  .placeholder-guide {
    position: absolute;
    box-sizing: border-box;
    border: 1.5px dashed rgba(0, 112, 192, 0.55);
    background: rgba(0, 112, 192, 0.04);
    pointer-events: none;
    z-index: 0;
  }
  .placeholder-guide-label {
    position: absolute;
    top: 2px;
    left: 4px;
    font-size: 0.7rem;
    color: rgba(0, 112, 192, 0.85);
    text-transform: capitalize;
    font-family: system-ui, sans-serif;
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
