/** Vertical alignment of a run. */
export type VerticalAlignDto = 'baseline' | 'superscript' | 'subscript'

/** RGBA color. */
export interface ColorDto {
  r: number
  g: number
  b: number
  a: number
}

/** Theme snapshot from slides-core. */
export interface ThemeSnapshot {
  background: ColorDto
  headingFont: string
  bodyFont: string
  accentColor: ColorDto
  /** High-contrast accessibility mode. */
  highContrast: boolean
}

/** Fixed slide dimensions (aspect ratio), in EMU. Mirrors slides-core SlideSize. */
export interface SlideSizeDto {
  widthEmu: number
  heightEmu: number
}

/** A named slide section that starts at a given slide. Mirrors slides-core SlideSection. */
export interface SlideSectionDto {
  name: string
  startSlideId: string
}

/** Heading level for a paragraph style. */
export type HeadingLevelDto = 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6'

/** Paragraph-level style. */
export interface ParagraphStyleDto {
  heading?: HeadingLevelDto
  blockquote: boolean
  codeBlock: boolean
  indentLevel: number
}

/** Inline text run. */
export interface RunDto {
  text: string
  bold: boolean
  italic: boolean
  underline: boolean
  strikethrough: boolean
  verticalAlign: VerticalAlignDto
  code: boolean
  fontFamily?: string
}

/** Paragraph inside a text box. */
export interface ParagraphDto {
  runs: RunDto[]
  listStyle: 'none' | 'ordered' | 'unordered'
  style: ParagraphStyleDto
}

/** Bounding rectangle in EMU. */
export interface RectDto {
  x: number
  y: number
  width: number
  height: number
}

/** Text box shape snapshot. */
export interface TextBoxSnapshot {
  frame: RectDto
  paragraphs: ParagraphDto[]
}

/** Opaque passthrough shape snapshot. */
export interface PassthroughSnapshot {
  id: string
  label: string
  sourcePart: string
  frame?: { x: number; y: number; width: number; height: number }
}

/** Placement of a shape: a bounding frame plus a rotation around its center. */
export interface TransformDto {
  frame: RectDto
  rotation: number
}

/** The geometric primitive a shape is built from (mirrors slides-core Geometry). */
export type GeometryDto =
  | 'rectangle'
  | 'ellipse'
  | 'triangle'
  | 'line'
  | 'arrow'
  | 'right_arrow_callout'
  | 'star5'
  | { rounded_rectangle: { radius: number } }

/** Fill applied to a shape's interior. */
export type FillDto = { solid: ColorDto }

/** Dash pattern for an outline. */
export type DashStyleDto = 'solid' | 'dash' | 'dot' | 'dash_dot'

/** Outline (stroke) of a shape. */
export interface OutlineDto {
  color: ColorDto
  widthEmu: number
  dash: DashStyleDto
}

/** Drop shadow drawn behind a shape. */
export interface ShadowDto {
  offsetX: number
  offsetY: number
  blur: number
  color: ColorDto
  opacity: number
}

/** Visual style applied to a geometric shape. */
export interface StyleDto {
  fill?: FillDto
  outline?: OutlineDto
  shadow?: ShadowDto
}

/** Crop applied to an image, as fractions of its native size in 0..=1. */
export interface CropDto {
  left: number
  top: number
  right: number
  bottom: number
}

/** Image shape snapshot, referencing bytes in the deck media store. */
export interface ImageShapeSnapshot {
  transform: TransformDto
  mediaRef: string
  crop?: CropDto
}

/** Geometric shape snapshot. */
export interface GeometricShapeSnapshot {
  transform: TransformDto
  geometry: GeometryDto
  style: StyleDto
}

/** Horizontal alignment of cell text. */
export type CellAlignDto = 'left' | 'center' | 'right'

/** A single border edge: color, width, and dash style. */
export interface BorderEdgeDto {
  color: ColorDto
  widthEmu: number
  dash: DashStyleDto
}

/** The four borders of a cell (or the table default). */
export interface TableBordersDto {
  top?: BorderEdgeDto
  bottom?: BorderEdgeDto
  left?: BorderEdgeDto
  right?: BorderEdgeDto
}

/** A single cell in a table. */
export interface TableCellDto {
  text: string
  fill?: FillDto
  borders?: TableBordersDto
  align: CellAlignDto
}

/** A single row of cells in a table. */
export interface TableRowDto {
  height: number
  cells: TableCellDto[]
}

/** Table shape snapshot: a grid of editable cells. */
export interface TableShapeSnapshot {
  transform: TransformDto
  rows: TableRowDto[]
  columnWidths: number[]
  defaultBorders: TableBordersDto
  headerRow: boolean
}

/** Chart type (mirrors slides-core ChartType). */
export type ChartTypeDto = 'bar' | 'column' | 'line' | 'area' | 'pie' | 'scatter'

/** A single (x, y) point in a scatter series. */
export interface XYPointDto {
  x: number
  y: number
}

/** A series of (x, y) points for scatter charts. */
export interface XYSeriesDto {
  name: string
  points: XYPointDto[]
}

/** A value series aligned with a set of categories. */
export interface CategorySeriesDto {
  name: string
  values: number[]
}

/** Category-based chart data. */
export interface CategoryChartDataDto {
  kind: 'category'
  value: {
    categories: string[]
    series: CategorySeriesDto[]
  }
}

/** XY (scatter) chart data. */
export interface XYChartDataDto {
  kind: 'xy'
  value: {
    series: XYSeriesDto[]
  }
}

/** Chart data union. */
export type ChartDataDto = CategoryChartDataDto | XYChartDataDto

/** Chart shape snapshot. */
export interface ChartShapeSnapshot {
  transform: TransformDto
  chartType: ChartTypeDto
  data: ChartDataDto
  title?: string
}

/** Shape snapshot union. */
export interface ShapeSnapshot {
  kind: 'text_box' | 'passthrough' | 'image' | 'geometric' | 'table' | 'chart'
  value:
    | TextBoxSnapshot
    | PassthroughSnapshot
    | ImageShapeSnapshot
    | GeometricShapeSnapshot
    | TableShapeSnapshot
    | ChartShapeSnapshot
}

/** Slide-to-slide transition kind (mirrors slides-core TransitionKind). */
export type TransitionKindDto = 'none' | 'fade' | 'slide' | 'push' | 'wipe'

/** Slide-to-slide transition. */
export interface TransitionDto {
  kind: TransitionKindDto
  durationMs: number
}

/** Build-in effect kind (mirrors slides-core BuildEffect). */
export type BuildEffectDto =
  | 'fade'
  | 'slide_in_left'
  | 'slide_in_right'
  | 'slide_in_top'
  | 'slide_in_bottom'
  | 'appear'
  | 'disappear'

/** One build-in step targeting a shape by index. */
export interface BuildStepDto {
  shapeIndex: number
  effect: BuildEffectDto
  durationMs: number
}

/** Ordered build-in animation sequence (mirrors slides-core Animation). */
export interface AnimationDto {
  steps: BuildStepDto[]
}

/** Slide snapshot. */
export interface SlideSnapshot {
  id: string
  notes: string
  shapes: ShapeSnapshot[]
  transition?: TransitionDto
  animation?: AnimationDto
  /** Rich-text speaker notes, when present. When absent, plain notes are used. */
  richNotes?: ParagraphDto[]
  /** Name of the layout (from the deck's layouts) this slide uses, if any. */
  layoutRef?: string
}

/** A named placeholder frame, mirroring slides-core PlaceholderDef. */
export interface PlaceholderDefDto {
  name: string
  frame: RectDto
}

/** A named layout variant, mirroring slides-core Layout. */
export interface LayoutDto {
  name: string
  placeholders: PlaceholderDefDto[]
}

/** A background shape painted by a slide master, mirroring slides-core BackgroundShape. */
export interface BackgroundShapeDto {
  geometry: GeometryDto
  style: StyleDto
  transform: TransformDto
}

/** A slide master, mirroring slides-core Master. */
export interface MasterDto {
  backgroundShapes: BackgroundShapeDto[]
  placeholders: PlaceholderDefDto[]
}

/** Summary of a built-in template for the picker, with a theme preview. */
export interface TemplateInfoDto {
  name: string
  displayName: string
  /** Theme background color as a CSS hex string (e.g. '#ffffff'). */
  backgroundHex: string
  /** Theme accent color as a CSS hex string. */
  accentHex: string
  headingFont: string
  bodyFont: string
}

/** Media entry with bytes base64-encoded, keyed by media reference. */
export interface MediaEntryDto {
  mime: string
  bytes: string
  width: number
  height: number
}

/** Map of media reference to its base64-encoded entry. */
export type MediaMap = Record<string, MediaEntryDto>

/** Deck snapshot returned by every mutating command. */
export interface DeckSnapshot {
  id: string
  schemaVersion: number
  theme: ThemeSnapshot
  /** Built-in template this deck is based on (e.g. 'default', 'pitch'), if any. */
  template?: string
  /** The deck's available layouts, derived from its template. */
  layouts: LayoutDto[]
  /** The deck's slide master: background layers and placeholder definitions. */
  master: MasterDto
  /** Fixed slide dimensions (aspect ratio), when set. */
  slideSize?: SlideSizeDto
  /** Named slide sections, in slide order. */
  sections: SlideSectionDto[]
  slides: SlideSnapshot[]
  media: MediaMap
  /** Presenter settings (laser pointer + highlighter). */
  presenterSettings: PresenterSettingsDto
  warnings: WarningDto[]
}

/** Loss ledger warning. */
export interface WarningDto {
  slideId: string
  message: string
}

/** Presenter settings: laser pointer and highlighter defaults and colors. */
export interface PresenterSettingsDto {
  /** Whether the laser pointer is enabled by default. */
  laserPointer: boolean
  /** Laser pointer color as a CSS hex string (e.g. `#ff0000`). */
  laserColor: string
  /** Whether the highlighter tool is enabled by default. */
  highlighter: boolean
  /** Highlighter color as a CSS hex string (e.g. `#ffff00`). */
  highlighterColor: string
}

/** Presenter view state. */
export interface PresenterState {
  currentSlide: SlideSnapshot
  nextSlide: SlideSnapshot | null
  slideNumber: number
  total: number
  notes: string
  media: MediaMap
  /** Deck slide size (aspect ratio), when set. */
  slideSize?: SlideSizeDto
  /** Whether the deck is rendered in high-contrast mode. */
  highContrast: boolean
  /** Presenter settings (laser pointer + highlighter). */
  presenterSettings: PresenterSettingsDto
}

/** Recovery snapshot metadata. */
export interface RecoverySnapshot {
  id: string
  timestamp: string
  deckId: string
}

/** A misspelled word with its byte span within the checked text. */
export interface MisspellingDto {
  word: string
  byteStart: number
  byteEnd: number
}
