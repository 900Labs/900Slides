// @ts-nocheck

const EMU_PER_CSS_PIXEL = 9_525
const CANVAS_GUTTER_PX = 48

/** Fits a slide into the usable canvas viewport without enlarging it. */
export function fitCanvasScale(viewportWidth, viewportHeight, widthEmu, heightEmu) {
  if (
    !Number.isFinite(viewportWidth) ||
    !Number.isFinite(viewportHeight) ||
    !Number.isFinite(widthEmu) ||
    !Number.isFinite(heightEmu) ||
    viewportWidth <= CANVAS_GUTTER_PX ||
    viewportHeight <= CANVAS_GUTTER_PX ||
    widthEmu <= 0 ||
    heightEmu <= 0
  ) {
    return 1
  }
  const slideWidth = widthEmu / EMU_PER_CSS_PIXEL
  const slideHeight = heightEmu / EMU_PER_CSS_PIXEL
  return Math.min(
    1,
    (viewportWidth - CANVAS_GUTTER_PX) / slideWidth,
    (viewportHeight - CANVAS_GUTTER_PX) / slideHeight,
  )
}

/**
 * Returns the unscaled slide surface and the scaled layout footprint. The
 * editor keeps the surface at its native CSS-pixel size and applies the scale
 * as one transform to it, so text, table typography, and geometry follow the
 * same zoom rather than wrapping at different apparent sizes.
 */
export function slideSurfaceDimensions(widthEmu, heightEmu, scale) {
  const logicalWidthPx = widthEmu / EMU_PER_CSS_PIXEL
  const logicalHeightPx = heightEmu / EMU_PER_CSS_PIXEL
  const surfaceScale = Number.isFinite(scale) && scale > 0 ? scale : 1
  return {
    logicalWidthPx,
    logicalHeightPx,
    footprintWidthPx: logicalWidthPx * surfaceScale,
    footprintHeightPx: logicalHeightPx * surfaceScale,
    surfaceScale,
  }
}
