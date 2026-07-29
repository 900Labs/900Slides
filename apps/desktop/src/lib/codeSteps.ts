import type { ParagraphStyleDto, SlideSnapshot, TextBoxSnapshot } from './types'

/**
 * Stepped code highlighting helpers, mirroring the renderer's
 * `parse_code_steps` / `line_step_state` so the editor, presenter, and
 * audience windows all agree on which lines a range string covers.
 *
 * A range string like `"1-3|4|5,7"` is split on `|` into steps; within a step,
 * comma-separated entries are either a single line (`"5"`) or an inclusive
 * range (`"1-3"`). Line numbers are 1-based and index paragraphs within a code
 * block text box.
 */

/** An inclusive 1-based line range `[start, end]`. */
export type LineRange = [number, number]

/** Parses a stepped code range string into one step per pipe segment. */
export function parseCodeSteps(ranges: string): LineRange[][] {
  const trimmed = ranges.trim()
  if (!trimmed) return []
  const steps: LineRange[][] = []
  for (const segment of trimmed.split('|')) {
    const seg: LineRange[] = []
    for (const entry of segment.split(',')) {
      const e = entry.trim()
      if (!e) continue
      const dash = e.indexOf('-')
      if (dash >= 0) {
        const start = parseInt(e.slice(0, dash).trim(), 10)
        const end = parseInt(e.slice(dash + 1).trim(), 10)
        if (!Number.isNaN(start) && !Number.isNaN(end)) seg.push([start, end])
      } else {
        const n = parseInt(e, 10)
        if (!Number.isNaN(n)) seg.push([n, n])
      }
    }
    steps.push(seg)
  }
  return steps
}

/** State of a code line relative to the active step. */
export type CodeLineState = 'active' | 'dimmed' | 'neutral'

/**
 * Returns the render state for a 1-based `line`: a line in the active step is
 * `'active'`, any other line in a stepped block is `'dimmed'`, and a block with
 * no ranges is `'neutral'` (rendered normally).
 */
export function codeLineState(
  ranges: string,
  activeStep: number,
  line: number,
): CodeLineState {
  const steps = parseCodeSteps(ranges)
  if (steps.length === 0) return 'neutral'
  const active = steps[activeStep]
  const inActive = active?.some(([s, e]) => line >= s && line <= e) ?? false
  return inActive ? 'active' : 'dimmed'
}

/** Returns the step range string for a paragraph, or `undefined` when unset. */
function paragraphStepRanges(style: ParagraphStyleDto): string | undefined {
  if (!style.codeBlock) return undefined
  const ranges = style.codeStepRanges
  if (!ranges || ranges.trim() === '') return undefined
  return ranges
}

/**
 * Returns the number of code steps on a slide: the maximum step count across
 * its code blocks (since the active step is shared by every block on the slide).
 * `0` means no stepped code blocks are present.
 */
export function codeStepCount(slide: SlideSnapshot | null | undefined): number {
  if (!slide) return 0
  let max = 0
  for (const shape of slide.shapes) {
    if (shape.kind !== 'text_box') continue
    const textBox = shape.value as TextBoxSnapshot
    for (const paragraph of textBox.paragraphs) {
      const ranges = paragraphStepRanges(paragraph.style)
      if (!ranges) continue
      const count = parseCodeSteps(ranges).length
      if (count > max) max = count
    }
  }
  return max
}
