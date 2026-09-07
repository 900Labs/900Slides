import assert from 'node:assert/strict'
import test from 'node:test'

import {
  MEDIA_HISTORY_CACHE_LIMITS,
  mergeDeckSnapshot,
  resetDeckSnapshotCache,
} from './deckSnapshot.js'

function snapshot(overrides = {}) {
  return {
    id: 'deck-a',
    mediaRevision: 4,
    media: { image: { mime: 'image/png', bytes: 'abc', width: 1, height: 1 } },
    ...overrides,
  }
}

test('reuses media only for the exact deck revision', () => {
  resetDeckSnapshotCache()
  const initial = mergeDeckSnapshot(snapshot())
  const merged = mergeDeckSnapshot(snapshot({ media: undefined }))
  assert.equal(merged.media, initial.media)
})

function mediaWithBytes(bytes) {
  return { image: { mime: 'image/png', bytes, width: 1, height: 1 } }
}

test('same-length media replacements reuse only their exact generation', () => {
  resetDeckSnapshotCache()
  const before = mergeDeckSnapshot(snapshot({ media: mediaWithBytes('aaaa') }))
  const after = mergeDeckSnapshot(snapshot({
    mediaRevision: 5,
    media: mediaWithBytes('bbbb'),
  }))
  assert.equal(mergeDeckSnapshot(snapshot({ media: undefined })).media, before.media)
  assert.equal(mergeDeckSnapshot(snapshot({ mediaRevision: 5, media: undefined })).media, after.media)
})

test('media history has a count bound and preserves the current revision', () => {
  resetDeckSnapshotCache()
  for (let revision = 0; revision <= MEDIA_HISTORY_CACHE_LIMITS.maxEntries + 1; revision++) {
    mergeDeckSnapshot(snapshot({ mediaRevision: revision }))
  }
  assert.throws(
    () => mergeDeckSnapshot(snapshot({ mediaRevision: 0, media: undefined })),
    /missing media payload/,
  )
  assert.doesNotThrow(() => mergeDeckSnapshot(snapshot({
    mediaRevision: MEDIA_HISTORY_CACHE_LIMITS.maxEntries + 1,
    media: undefined,
  })))
})

test('media history evicts by bytes and least recent access', () => {
  resetDeckSnapshotCache()
  // Each map is just over one third of the history budget: two fit, three do
  // not, although the count limit permits three historical entries.
  const bytes = 'a'.repeat(Math.floor(MEDIA_HISTORY_CACHE_LIMITS.maxBytes / 6))
  for (let revision = 0; revision <= 2; revision++) {
    mergeDeckSnapshot(snapshot({ mediaRevision: revision, media: mediaWithBytes(bytes) }))
  }
  mergeDeckSnapshot(snapshot({ mediaRevision: 0, media: undefined }))
  mergeDeckSnapshot(snapshot({ mediaRevision: 3, media: mediaWithBytes(bytes) }))
  assert.doesNotThrow(() => mergeDeckSnapshot(snapshot({ mediaRevision: 0, media: undefined })))
  assert.throws(
    () => mergeDeckSnapshot(snapshot({ mediaRevision: 1, media: undefined })),
    /missing media payload/,
  )
  assert.doesNotThrow(() => mergeDeckSnapshot(snapshot({ mediaRevision: 3, media: undefined })))
})

test('oversized current media remains reusable but is released when superseded', () => {
  resetDeckSnapshotCache()
  const initial = mergeDeckSnapshot(snapshot({
    media: mediaWithBytes('a'.repeat(MEDIA_HISTORY_CACHE_LIMITS.maxBytes / 2)),
  }))
  assert.equal(mergeDeckSnapshot(snapshot({ media: undefined })).media, initial.media)
  mergeDeckSnapshot(snapshot({ mediaRevision: 5 }))
  assert.throws(() => mergeDeckSnapshot(snapshot({ media: undefined })), /missing media payload/)
})

test('late older payloads do not replace the current oversized working set', () => {
  resetDeckSnapshotCache()
  const current = mergeDeckSnapshot(snapshot({
    mediaRevision: 10,
    media: mediaWithBytes('a'.repeat(MEDIA_HISTORY_CACHE_LIMITS.maxBytes / 2)),
  }))
  for (let revision = 0; revision <= MEDIA_HISTORY_CACHE_LIMITS.maxEntries; revision++) {
    mergeDeckSnapshot(snapshot({ mediaRevision: revision }))
  }
  assert.equal(mergeDeckSnapshot(snapshot({ mediaRevision: 10, media: undefined })).media, current.media)
})

test('opening another deck releases current media and history from the previous deck', () => {
  resetDeckSnapshotCache()
  mergeDeckSnapshot(snapshot({ mediaRevision: 3 }))
  mergeDeckSnapshot(snapshot())
  const other = mergeDeckSnapshot(snapshot({ id: 'deck-b' }))
  for (const mediaRevision of [3, 4]) {
    assert.throws(
      () => mergeDeckSnapshot(snapshot({ mediaRevision, media: undefined })),
      /missing media payload/,
    )
  }
  assert.equal(mergeDeckSnapshot(snapshot({ id: 'deck-b', media: undefined })).media, other.media)
})

test('replacing a cached history revision updates byte accounting', () => {
  resetDeckSnapshotCache()
  mergeDeckSnapshot(snapshot({ mediaRevision: 10 }))
  const bytes = 'a'.repeat(Math.floor(MEDIA_HISTORY_CACHE_LIMITS.maxBytes / 6))
  mergeDeckSnapshot(snapshot({ mediaRevision: 1, media: mediaWithBytes(bytes) }))
  // A fresh full response for the same revision must replace its accounting.
  mergeDeckSnapshot(snapshot({ mediaRevision: 1, media: mediaWithBytes(bytes) }))
  mergeDeckSnapshot(snapshot({ mediaRevision: 2, media: mediaWithBytes(bytes) }))
  assert.doesNotThrow(() => mergeDeckSnapshot(snapshot({ mediaRevision: 1, media: undefined })))
  assert.doesNotThrow(() => mergeDeckSnapshot(snapshot({ mediaRevision: 2, media: undefined })))
})

test('reset releases all revisions and resets historical byte accounting', () => {
  resetDeckSnapshotCache()
  const media = mediaWithBytes('a'.repeat(MEDIA_HISTORY_CACHE_LIMITS.maxBytes / 4))
  mergeDeckSnapshot(snapshot({ mediaRevision: 1, media }))
  mergeDeckSnapshot(snapshot({ mediaRevision: 2 }))
  resetDeckSnapshotCache()
  assert.throws(
    () => mergeDeckSnapshot(snapshot({ mediaRevision: 1, media: undefined })),
    /missing media payload/,
  )
  mergeDeckSnapshot(snapshot({ mediaRevision: 3, media }))
  mergeDeckSnapshot(snapshot({ mediaRevision: 4 }))
  assert.doesNotThrow(() => mergeDeckSnapshot(snapshot({ mediaRevision: 3, media: undefined })))
})

test('rejects omitted media after a deck or revision change', () => {
  resetDeckSnapshotCache()
  mergeDeckSnapshot(snapshot())
  assert.throws(
    () => mergeDeckSnapshot(snapshot({ mediaRevision: 5, media: undefined })),
    /missing media payload/,
  )
  assert.throws(
    () => mergeDeckSnapshot(snapshot({ id: 'deck-b', media: undefined })),
    /missing media payload/,
  )
})
