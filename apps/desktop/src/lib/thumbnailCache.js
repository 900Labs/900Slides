// @ts-check

/** @type {Map<string, { svg: string, bytes: number }>} */
const svgByRevision = new Map()
export const THUMBNAIL_CACHE_LIMITS = Object.freeze({
  maxEntries: 200,
  maxBytes: 8 * 1024 * 1024,
})
let cachedBytes = 0
let cacheGeneration = 0

/**
 * Builds the deck-wide half of a correctness-first cache key once per deck
 * snapshot, rather than serializing these shared objects in every thumbnail.
 * @param {unknown} theme
 * @param {unknown} template
 * @param {unknown} layouts
 * @param {unknown} master
 * @param {unknown} slideSize
 * @param {number} mediaRevision
 * @param {string} [deckId]
 */
export function thumbnailDeckRenderKey(
  theme,
  template,
  layouts,
  master,
  slideSize,
  mediaRevision,
  deckId,
) {
  return JSON.stringify({
    deckId: deckId ?? null,
    theme,
    template: template ?? null,
    layouts,
    master,
    slideSize: slideSize ?? null,
    mediaRevision,
  })
}

/** @param {unknown} slide @param {string} deckRenderKey */
export function thumbnailRevisionKey(slide, deckRenderKey) {
  return `${deckRenderKey}\n${JSON.stringify(slide)}`
}

/** @param {string} key */
export function getCachedThumbnail(key) {
  const value = svgByRevision.get(key)
  if (value !== undefined) {
    svgByRevision.delete(key)
    svgByRevision.set(key, value)
  }
  return value?.svg
}

/** @param {string} key @param {string} svg */
export function cacheThumbnail(key, svg) {
  const previous = svgByRevision.get(key)
  if (previous) cachedBytes -= previous.bytes
  svgByRevision.delete(key)

  // Count UTF-16 code units conservatively, including the correctness key and
  // entry overhead. SVGs may embed full-resolution base64 images. A count-only
  // bound would let a small number of photos retain hundreds of megabytes.
  const bytes = 2 * (key.length + svg.length) + 128
  if (bytes > THUMBNAIL_CACHE_LIMITS.maxBytes) return

  svgByRevision.set(key, { svg, bytes })
  cachedBytes += bytes
  while (
    svgByRevision.size > THUMBNAIL_CACHE_LIMITS.maxEntries ||
    cachedBytes > THUMBNAIL_CACHE_LIMITS.maxBytes
  ) {
    const oldest = svgByRevision.keys().next().value
    if (oldest === undefined) break
    cachedBytes -= /** @type {{ bytes: number }} */ (svgByRevision.get(oldest)).bytes
    svgByRevision.delete(oldest)
  }
}

/**
 * Starts a visible thumbnail request. Cancellation also discards its result
 * from the shared cache, so scrolling away releases the SVG. Resetting the
 * cache invalidates requests belonging to an earlier deck/session.
 *
 * @param {string} key
 * @param {() => Promise<string>} render
 * @param {(svg: string | null) => void} onResult
 * @returns {() => void}
 */
export function requestThumbnail(key, render, onResult) {
  let cancelled = false
  const generation = cacheGeneration
  const isCurrent = () => !cancelled && generation === cacheGeneration
  const cached = getCachedThumbnail(key)
  if (cached !== undefined) {
    onResult(cached)
  } else {
    Promise.resolve()
      .then(() => (isCurrent() ? render() : undefined))
      .then((markup) => {
        if (markup !== undefined && isCurrent()) {
          cacheThumbnail(key, markup)
          onResult(markup)
        }
      })
      .catch(() => {
        if (isCurrent()) onResult(null)
      })
  }
  return () => {
    cancelled = true
  }
}

export function resetThumbnailCache() {
  svgByRevision.clear()
  cachedBytes = 0
  cacheGeneration += 1
}
