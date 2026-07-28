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

/** Shape snapshot union. */
export interface ShapeSnapshot {
  kind: 'text_box' | 'passthrough' | 'image' | 'geometric' | 'table'
  value:
    | TextBoxSnapshot
    | PassthroughSnapshot
    | ImageShapeSnapshot
    | GeometricShapeSnapshot
    | TableShapeSnapshot
}

/** Slide snapshot. */
export interface SlideSnapshot {
  id: string
  notes: string
  shapes: ShapeSnapshot[]
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
  slides: SlideSnapshot[]
  media: MediaMap
  warnings: WarningDto[]
}

/** Loss ledger warning. */
export interface WarningDto {
  slideId: string
  message: string
}

/** Presenter view state. */
export interface PresenterState {
  currentSlide: SlideSnapshot
  nextSlide: SlideSnapshot | null
  slideNumber: number
  total: number
  notes: string
  media: MediaMap
}

/** Recovery snapshot metadata. */
export interface RecoverySnapshot {
  id: string
  timestamp: string
  deckId: string
}
