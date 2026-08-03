// @ts-nocheck

/**
 * Returns an element's rendered rectangle in container-local pixels. Reading
 * `getBoundingClientRect()` intentionally includes CSS transforms, unlike
 * layout-only offset measurements. This keeps slide overlays aligned when the
 * editor or presenter scales the whole slide surface.
 */
export function renderedRectRelativeTo(containerRect, elementRect) {
  return {
    x: elementRect.left - containerRect.left,
    y: elementRect.top - containerRect.top,
    w: elementRect.width,
    h: elementRect.height,
  }
}
