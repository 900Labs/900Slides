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
  /** Laser pointer position / visibility (throttled). */
  laser: 'presenter:laser',
  /** Full highlighter stroke list (throttled while drawing). */
  highlighter: 'presenter:highlighter',
  /** Audience blank mode. */
  blank: 'presenter:blank',
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
