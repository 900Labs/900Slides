// @ts-check

/** @typedef {import('./types').DeckSnapshot} DeckSnapshot */
/** @typedef {import('./types').MediaMap} MediaMap */

/** @typedef {{ key: string, deckId: string, revision: number, media: MediaMap, bytes: number }} CachedMedia */

// The current map is the editor's required working set and may exceed this
// budget. Only history adds retained media beyond that active document. Never
// retain an oversized map once a new media revision/deck replaces it.
export const MEDIA_HISTORY_CACHE_LIMITS = Object.freeze({
  maxEntries: 3,
  maxBytes: 16 * 1024 * 1024,
})
/** @type {CachedMedia | undefined} */
let currentMedia
/** @type {Map<string, CachedMedia>} */
const mediaHistory = new Map()
let historyBytes = 0

/** @param {{ id: string, mediaRevision: number }} snapshot */
function mediaCacheKey(snapshot) {
  return `${snapshot.id}:${snapshot.mediaRevision}`
}

/** @param {MediaMap} media */
function mediaBytes(media) {
  let bytes = 128
  for (const [key, entry] of Object.entries(media)) {
    // No serialization or base64 decoding: those would duplicate the payload
    // just to measure it. Two bytes per code unit is a conservative string
    // estimate; include map keys, MIME strings and per-entry overhead too.
    bytes += 2 * (key.length + entry.mime.length + entry.bytes.length) + 192
  }
  return bytes
}

/** @param {CachedMedia} entry */
function rememberPrevious(entry) {
  const previous = mediaHistory.get(entry.key)
  if (previous) historyBytes -= previous.bytes
  mediaHistory.delete(entry.key)
  if (entry.bytes > MEDIA_HISTORY_CACHE_LIMITS.maxBytes) return

  mediaHistory.set(entry.key, entry)
  historyBytes += entry.bytes
  while (
    mediaHistory.size > MEDIA_HISTORY_CACHE_LIMITS.maxEntries ||
    historyBytes > MEDIA_HISTORY_CACHE_LIMITS.maxBytes
  ) {
    const oldest = mediaHistory.keys().next().value
    if (oldest === undefined) break
    historyBytes -= /** @type {CachedMedia} */ (mediaHistory.get(oldest)).bytes
    mediaHistory.delete(oldest)
  }
}

/**
 * Merges an IPC snapshot whose unchanged media payload may be omitted. The
 * backend's media revision is monotonic, so a payload is reused only for the
 * exact deck id and revision that produced it.
 *
 * @param {Omit<DeckSnapshot, 'media'> & { media?: MediaMap }} snapshot
 * @returns {DeckSnapshot}
 */
export function mergeDeckSnapshot(snapshot) {
  const key = mediaCacheKey(snapshot)
  if (snapshot.media !== undefined) {
    const entry = {
      key,
      deckId: snapshot.id,
      revision: snapshot.mediaRevision,
      media: snapshot.media,
      bytes: mediaBytes(snapshot.media) + 2 * (key.length + snapshot.id.length) + 64,
    }
    if (currentMedia && currentMedia.deckId !== snapshot.id) {
      resetDeckSnapshotCache()
    }
    if (currentMedia && snapshot.mediaRevision < currentMedia.revision) {
      // A late completion can supply a valid older snapshot. Keep its exact
      // media in bounded history without displacing the current working set.
      rememberPrevious(entry)
    } else {
      if (currentMedia && currentMedia.key !== key) rememberPrevious(currentMedia)
      currentMedia = entry
    }
    return /** @type {DeckSnapshot} */ (snapshot)
  }

  const entry = currentMedia?.key === key ? currentMedia : mediaHistory.get(key)
  if (entry === undefined) {
    // invokeApp recovers an evicted payload with get_snapshot and checks the
    // exact deck/revision again. Never substitute another revision's media.
    throw new Error(`missing media payload for deck revision ${key}`)
  }
  if (entry !== currentMedia) {
    mediaHistory.delete(key)
    mediaHistory.set(key, entry)
  }
  return { ...snapshot, media: entry.media }
}

/** Releases both current media and retained history, including on deck change. */
export function resetDeckSnapshotCache() {
  currentMedia = undefined
  mediaHistory.clear()
  historyBytes = 0
}
