import type { MorphFrameDto, ProjectorFiltersDto, SlideSnapshot } from './types'

/**
 * Shared contract for the dual-display presenter.
 *
 * The presenter (control) window owns navigation and the transient laser /
 * highlighter / black-white state, and broadcasts changes to the audience
 * (fullscreen) window over Tauri events. Both windows use the names and payload
 * shapes declared here so they never drift apart.
 *
 * Laser and highlighter coordinates are NORMALIZED to the visible slide area:
 * `{ x, y }` where both components are in `0..1` relative to the rendered
 * `.canvas` element. Each window converts these to its own pixel space, so the
 * two windows can render the slide at different scales and still stay aligned.
 */

/** A normalized point within the slide area, in `0..1`. */
export interface Vec2 {
  x: number
  y: number
}

/** A single freehand highlighter stroke (normalized points + color). */
export interface HighlighterStroke {
  points: Vec2[]
  color: string
}

/** Audience blank mode for Q&A: restore the slide, or blank to a solid color. */
export type BlankMode = 'none' | 'black' | 'white'

/** Tauri event channel names used by the dual-display presenter. */
export const PRESENTER_EVENTS = {
  /** Full presenter state, emitted on slide changes. */
  state: 'presenter:state',
  /** Current build-step index, emitted when the build timeline advances. */
  buildStep: 'presenter:build-step',
  /** Current code-step index, emitted when stepped code highlighting advances. */
  codeStep: 'presenter:code-step',
  /** Laser pointer position / visibility (throttled). */
  laser: 'presenter:laser',
  /** Full highlighter stroke list (throttled while drawing). */
  highlighter: 'presenter:highlighter',
  /** Audience blank mode. */
  blank: 'presenter:blank',
  /** Projector CSS filters applied to the audience slide container. */
  filters: 'presenter:filters',
  /** Magic Move morph payload: previous slide + interpolation frames. */
  morph: 'presenter:morph',
  /** Both windows should close. */
  exit: 'presenter:exit',
} as const

/** Payload for {@link PRESENTER_EVENTS.laser}. */
export interface LaserPayload {
  /** Normalized x within the slide area, in `0..1`. */
  x: number
  /** Normalized y within the slide area, in `0..1`. */
  y: number
  /** Whether the laser dot is currently visible. */
  visible: boolean
  /** Laser dot color (CSS hex). */
  color: string
}

/** Payload for {@link PRESENTER_EVENTS.highlighter}. */
export interface HighlighterPayload {
  strokes: HighlighterStroke[]
}

/** Payload for {@link PRESENTER_EVENTS.blank}. */
export interface BlankPayload {
  mode: BlankMode
}

/** Payload for {@link PRESENTER_EVENTS.buildStep}. */
export interface BuildStepPayload {
  step: number
}

/** Payload for {@link PRESENTER_EVENTS.morph}: the previous slide plus the
 *  interpolation frames computed for a Magic Move transition. */
export interface MorphPayload {
  /** The slide being left, rendered as an overlay during the morph. */
  prev: SlideSnapshot
  /** Per-shape interpolation frames from `compute_morph`. */
  frames: MorphFrameDto[]
  /** Morph duration in milliseconds. */
  durationMs: number
}

/** Slide rect in stage-local pixels, used to position overlays. */
export interface SlideRect {
  x: number
  y: number
  w: number
  h: number
}

/**
 * Reads the rendered `.canvas` element's box relative to its stage container.
 * Returns `null` until the slide has rendered. The stage must be
 * `position: relative` (or otherwise be the canvas's offset parent).
 */
export function slideRectFromStage(stage: HTMLElement | null): SlideRect | null {
  if (!stage) return null
  const canvas = stage.querySelector<HTMLElement>('.canvas')
  if (!canvas) return null
  return {
    x: canvas.offsetLeft,
    y: canvas.offsetTop,
    w: canvas.offsetWidth,
    h: canvas.offsetHeight,
  }
}

/** Clamps a number into `0..1`. */
export function clamp01(v: number): number {
  if (v < 0) return 0
  if (v > 1) return 1
  return v
}

/** Neutral projector filters (no visible effect), matching the Rust default. */
export function defaultProjectorFilters(): ProjectorFiltersDto {
  return {
    invert: false,
    brightness: 1,
    contrast: 1,
    saturation: 1,
    sepia: 0,
    hueRotate: 0,
  }
}

/** Trims floating-point noise from a slider value for a clean CSS number. */
function cleanNumber(v: number): number {
  return Number(v.toFixed(4))
}

/**
 * Builds a CSS `filter` string for the audience window's slide container, e.g.
 * `invert(1) brightness(1.2) contrast(1.1) saturate(0.9) sepia(0.2)
 * hue-rotate(45deg)`. Only properties that differ from their neutral default
 * are emitted, so a reset yields an empty string (clearing the filter).
 */
export function projectorFilterCss(filters: ProjectorFiltersDto): string {
  const parts: string[] = []
  if (filters.invert) parts.push('invert(1)')
  if (filters.brightness !== 1) parts.push(`brightness(${cleanNumber(filters.brightness)})`)
  if (filters.contrast !== 1) parts.push(`contrast(${cleanNumber(filters.contrast)})`)
  if (filters.saturation !== 1) parts.push(`saturate(${cleanNumber(filters.saturation)})`)
  if (filters.sepia !== 0) parts.push(`sepia(${cleanNumber(filters.sepia)})`)
  if (filters.hueRotate !== 0) parts.push(`hue-rotate(${cleanNumber(filters.hueRotate)}deg)`)
  return parts.join(' ')
}

/**
 * Leading + trailing throttle. The first call runs immediately; subsequent
 * calls within `ms` are coalesced into a single trailing call carrying the
 * latest arguments. Used to cap laser / highlighter event emission to ~30fps.
 */
export function throttle<A extends unknown[]>(
  fn: (...args: A) => void,
  ms: number,
): (...args: A) => void {
  let last = 0
  let trailing: ReturnType<typeof setTimeout> | null = null
  let pending: A | null = null
  return (...args: A): void => {
    pending = args
    const remaining = ms - (Date.now() - last)
    if (remaining <= 0) {
      last = Date.now()
      if (trailing) {
        clearTimeout(trailing)
        trailing = null
      }
      pending = null
      fn(...args)
    } else if (trailing === null) {
      trailing = setTimeout(() => {
        last = Date.now()
        trailing = null
        if (pending) {
          const p = pending
          pending = null
          fn(...p)
        }
      }, remaining)
    }
  }
}

/**
 * Trailing debounce: collapses a burst of calls into a single trailing call
 * carrying the latest arguments, fired `ms` after the last call. Used to
 * coalesce rapid slider drags into one persistence write.
 */
export function debounce<A extends unknown[]>(
  fn: (...args: A) => void,
  ms: number,
): (...args: A) => void {
  let handle: ReturnType<typeof setTimeout> | null = null
  return (...args: A): void => {
    if (handle) clearTimeout(handle)
    handle = setTimeout(() => {
      handle = null
      fn(...args)
    }, ms)
  }
}

/** EMU -> CSS pixels, matching `SlideCanvas`. */
const MORPH_EMU_TO_PX = 1 / 9525

/** Converts an EMU length to a CSS pixel string. */
function morphPx(emu: number): string {
  return `${emu * MORPH_EMU_TO_PX}px`
}

/** Saved inline styles for a morphed element, restored once the morph ends. */
interface MorphSavedStyle {
  transition: string
  left: string
  top: string
  width: string
  height: string
  opacity: string
}

/**
 * Drives a Magic Move morph between the just-rendered next slide (`base`) and
 * the previous slide (`overlay`). Matching shape elements are located by their
 * `data-shape-id` attribute and animated via CSS transitions:
 *
 * - **Interpolate** (shape on both slides): in `base`, jump to the source frame
 *   with no transition, then transition `left/top/width/height` to the target.
 * - **Fade-in** (shape new on next): in `base`, start at opacity 0 and
 *   transition to 1.
 * - **Fade-out** (shape removed): in `overlay`, transition opacity 1 -> 0.
 *
 * Every other shape in the `overlay` is hidden, since the base canvas already
 * renders the incoming slide. After `durationMs`, inline overrides are restored
 * so the rendered DOM returns to its reactive state. Resolves immediately when
 * either canvas is missing or there is nothing to animate.
 */
export function runMorph(
  base: HTMLElement | null,
  overlay: HTMLElement | null,
  frames: MorphFrameDto[],
  durationMs: number,
): Promise<void> {
  if (!base || !overlay || frames.length === 0 || durationMs <= 0) {
    return Promise.resolve()
  }

  const collect = (root: HTMLElement): Map<string, HTMLElement> => {
    const map = new Map<string, HTMLElement>()
    for (const el of Array.from(root.querySelectorAll<HTMLElement>('[data-shape-id]'))) {
      const id = el.getAttribute('data-shape-id')
      if (id) map.set(id, el)
    }
    return map
  }

  const baseShapes = collect(base)
  const overlayShapes = collect(overlay)
  const transition =
    `left ${durationMs}ms ease, top ${durationMs}ms ease, ` +
    `width ${durationMs}ms ease, height ${durationMs}ms ease, opacity ${durationMs}ms ease`
  const saved = new Map<HTMLElement, MorphSavedStyle>()

  const remember = (el: HTMLElement): void => {
    if (saved.has(el)) return
    const s = el.style
    saved.set(el, {
      transition: s.transition,
      left: s.left,
      top: s.top,
      width: s.width,
      height: s.height,
      opacity: s.opacity,
    })
  }

  // 1. Apply the start state with transitions disabled.
  for (const frame of frames) {
    const baseEl = baseShapes.get(frame.shapeId)
    if (baseEl) {
      remember(baseEl)
      baseEl.style.transition = 'none'
      if (frame.from && frame.to) {
        baseEl.style.left = morphPx(frame.from.x)
        baseEl.style.top = morphPx(frame.from.y)
        baseEl.style.width = morphPx(frame.from.width)
        baseEl.style.height = morphPx(frame.from.height)
      } else if (frame.to) {
        baseEl.style.opacity = '0'
      }
    }

    const overlayEl = overlayShapes.get(frame.shapeId)
    if (overlayEl) {
      remember(overlayEl)
      overlayEl.style.transition = 'none'
      // Only fade-out shapes stay visible in the overlay; everything else is
      // already shown by the incoming base canvas and is hidden here.
      if (!(frame.from && !frame.to)) {
        overlayEl.style.opacity = '0'
      }
    }
  }

  // 2. Commit the start state, then enable transitions toward the targets.
  void base.offsetWidth

  for (const frame of frames) {
    const baseEl = baseShapes.get(frame.shapeId)
    if (baseEl) {
      if (frame.from && frame.to) {
        baseEl.style.transition = transition
        baseEl.style.left = morphPx(frame.to.x)
        baseEl.style.top = morphPx(frame.to.y)
        baseEl.style.width = morphPx(frame.to.width)
        baseEl.style.height = morphPx(frame.to.height)
      } else if (frame.to) {
        baseEl.style.transition = transition
        baseEl.style.opacity = '1'
      }
    }

    const overlayEl = overlayShapes.get(frame.shapeId)
    if (overlayEl && frame.from && !frame.to) {
      overlayEl.style.transition = transition
      overlayEl.style.opacity = '0'
    }
  }

  // 3. After the duration, restore the original inline styles.
  return new Promise<void>((resolve) => {
    window.setTimeout(() => {
      for (const [el, s] of saved) {
        el.style.transition = s.transition
        el.style.left = s.left
        el.style.top = s.top
        el.style.width = s.width
        el.style.height = s.height
        el.style.opacity = s.opacity
      }
      resolve()
    }, durationMs)
  })
}
