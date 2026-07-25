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

/** Inline text run. */
export interface RunDto {
  text: string
  bold: boolean
  italic: boolean
  underline: boolean
}

/** Paragraph inside a text box. */
export interface ParagraphDto {
  runs: RunDto[]
  listStyle: 'none' | 'ordered' | 'unordered'
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

/** Shape snapshot union. */
export interface ShapeSnapshot {
  kind: 'text_box' | 'passthrough'
  value: TextBoxSnapshot | PassthroughSnapshot
}

/** Slide snapshot. */
export interface SlideSnapshot {
  id: string
  notes: string
  shapes: ShapeSnapshot[]
}

/** Deck snapshot returned by every mutating command. */
export interface DeckSnapshot {
  id: string
  schemaVersion: number
  theme: ThemeSnapshot
  slides: SlideSnapshot[]
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
}

/** Recovery snapshot metadata. */
export interface RecoverySnapshot {
  id: string
  timestamp: string
  deckId: string
}
