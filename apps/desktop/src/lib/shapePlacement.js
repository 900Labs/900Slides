// @ts-nocheck

/** Minimum nonzero line thickness accepted by the desktop command, in EMU. */
export const MIN_LINE_THICKNESS_EMU = 8 * 9_525

/**
 * Builds a horizontal local line surface plus a rotation around its center.
 * This keeps every dragged endpoint pair (horizontal, vertical, and diagonal)
 * intact while retaining a thin, nonzero frame axis for model and PPTX
 * serialization. A zero-length click returns null for the normal click
 * fallback.
 */
export function linePlacementFromPoints(start, end) {
  if (
    !Number.isFinite(start?.x) ||
    !Number.isFinite(start?.y) ||
    !Number.isFinite(end?.x) ||
    !Number.isFinite(end?.y)
  ) {
    return null
  }

  const dx = end.x - start.x
  const dy = end.y - start.y
  const length = Math.hypot(dx, dy)
  if (length === 0) return null
  const centerX = (start.x + end.x) / 2
  const centerY = (start.y + end.y) / 2
  return {
    frame: {
      x: centerX - length / 2,
      y: centerY - MIN_LINE_THICKNESS_EMU / 2,
      width: length,
      height: MIN_LINE_THICKNESS_EMU,
    },
    rotation: (Math.atan2(dy, dx) * 180) / Math.PI,
  }
}
