import assert from 'node:assert/strict'
import test from 'node:test'

import {
  cacheThumbnail,
  getCachedThumbnail,
  resetThumbnailCache,
  requestThumbnail,
  THUMBNAIL_CACHE_LIMITS,
  thumbnailDeckRenderKey,
  thumbnailRevisionKey,
} from './thumbnailCache.js'

const slide = {
  id: 'slide-1',
  shapes: [{ kind: 'text_box', value: { paragraphs: [{ runs: [{ text: 'Alpha' }] }] } }],
}
const theme = { background: '#fff', headingFont: 'Arial', bodyFont: 'Arial' }
const size = { widthEmu: 12_192_000, heightEmu: 6_858_000 }
const layouts = [{ name: 'title', placeholders: [] }]
const master = { backgroundLayers: [], placeholders: [] }

function deckKey(overrides = {}) {
  return thumbnailDeckRenderKey(
    overrides.theme ?? theme,
    overrides.template ?? 'default',
    overrides.layouts ?? layouts,
    overrides.master ?? master,
    overrides.size ?? size,
    overrides.mediaRevision ?? 2,
    overrides.deckId ?? 'deck-a',
  )
}

test('thumbnail key is stable for unchanged render inputs', () => {
  assert.equal(
    thumbnailRevisionKey(slide, deckKey()),
    thumbnailRevisionKey(structuredClone(slide), deckKey()),
  )
})

test('slide text and style invalidate thumbnails', () => {
  const baseline = thumbnailRevisionKey(slide, deckKey())
  const changedText = structuredClone(slide)
  changedText.shapes[0].value.paragraphs[0].runs[0].text = 'Beta'
  const changedStyle = structuredClone(slide)
  changedStyle.shapes[0].value.paragraphs[0].runs[0].bold = true

  assert.notEqual(thumbnailRevisionKey(changedText, deckKey()), baseline)
  assert.notEqual(thumbnailRevisionKey(changedStyle, deckKey()), baseline)
})

test('every deck-wide renderer input invalidates thumbnails', () => {
  const baseline = deckKey()
  assert.notEqual(deckKey({ theme: { ...theme, background: '#000' } }), baseline)
  assert.notEqual(deckKey({ theme: { ...theme, highContrast: true } }), baseline)
  assert.notEqual(deckKey({ template: 'pitch' }), baseline)
  assert.notEqual(deckKey({ layouts: [{ name: 'blank', placeholders: [] }] }), baseline)
  assert.notEqual(deckKey({ master: { backgroundLayers: [{ color: '#eee' }], placeholders: [] } }), baseline)
  assert.notEqual(deckKey({ size: { ...size, widthEmu: 9_000_000 } }), baseline)
  assert.notEqual(deckKey({ mediaRevision: 3 }), baseline)
  assert.notEqual(deckKey({ deckId: 'deck-b' }), baseline)
})

test('cache evicts by entry count and least recent access', () => {
  resetThumbnailCache()
  for (let index = 0; index < THUMBNAIL_CACHE_LIMITS.maxEntries; index++) {
    cacheThumbnail(`${index}`, '<svg />')
  }
  assert.equal(getCachedThumbnail('0'), '<svg />')
  cacheThumbnail('next', '<svg />')
  assert.equal(getCachedThumbnail('0'), '<svg />')
  assert.equal(getCachedThumbnail('1'), undefined)
})

test('cache counts SVG bytes and evicts by bytes before reaching the entry cap', () => {
  resetThumbnailCache()
  const markup = 'a'.repeat(Math.floor(THUMBNAIL_CACHE_LIMITS.maxBytes / 6))
  cacheThumbnail('one', markup)
  cacheThumbnail('two', markup)
  getCachedThumbnail('one')
  cacheThumbnail('three', markup)
  assert.equal(getCachedThumbnail('one'), markup)
  assert.equal(getCachedThumbnail('two'), undefined)
  assert.equal(getCachedThumbnail('three'), markup)
})

test('cache also budgets correctness keys containing large slide metadata', () => {
  resetThumbnailCache()
  const key = 'a'.repeat(THUMBNAIL_CACHE_LIMITS.maxBytes / 2)
  cacheThumbnail(key, '<svg />')
  assert.equal(getCachedThumbnail(key), undefined)
})

test('oversized SVGs are not cached and do not evict useful small entries', () => {
  resetThumbnailCache()
  cacheThumbnail('small', '<svg />')
  cacheThumbnail('huge', 'a'.repeat(THUMBNAIL_CACHE_LIMITS.maxBytes / 2))
  assert.equal(getCachedThumbnail('huge'), undefined)
  assert.equal(getCachedThumbnail('small'), '<svg />')
})

test('replacing an entry subtracts its previous bytes before budgeting', () => {
  resetThumbnailCache()
  const markup = 'a'.repeat(Math.floor(THUMBNAIL_CACHE_LIMITS.maxBytes / 6))
  cacheThumbnail('one', markup)
  cacheThumbnail('one', markup)
  cacheThumbnail('two', markup)
  assert.equal(getCachedThumbnail('one'), markup)
  assert.equal(getCachedThumbnail('two'), markup)
  cacheThumbnail('one', 'a'.repeat(THUMBNAIL_CACHE_LIMITS.maxBytes / 2))
  assert.equal(getCachedThumbnail('one'), undefined)
  assert.equal(getCachedThumbnail('two'), markup)
})

test('reset clears byte accounting as well as entries', () => {
  resetThumbnailCache()
  const markup = 'a'.repeat(Math.floor(THUMBNAIL_CACHE_LIMITS.maxBytes / 6))
  cacheThumbnail('old', markup)
  resetThumbnailCache()
  cacheThumbnail('one', markup)
  cacheThumbnail('two', markup)
  assert.equal(getCachedThumbnail('old'), undefined)
  assert.equal(getCachedThumbnail('one'), markup)
  assert.equal(getCachedThumbnail('two'), markup)
})

async function settleRequests() {
  await new Promise((resolve) => setImmediate(resolve))
}

test('visible requests cache their exact render and cached revisits avoid IPC', async () => {
  resetThumbnailCache()
  const results = []
  let renderCalls = 0
  const render = async () => { renderCalls += 1; return '<svg />' }
  requestThumbnail('key', render, (markup) => results.push(markup))
  await settleRequests()
  requestThumbnail('key', render, (markup) => results.push(markup))
  assert.equal(renderCalls, 1)
  assert.deepEqual(results, ['<svg />', '<svg />'])
})

test('offscreen cancellation discards in-flight SVG without repopulating cache', async () => {
  resetThumbnailCache()
  const results = []
  let finish
  const pending = new Promise((resolve) => { finish = resolve })
  const cancel = requestThumbnail('old', () => pending, (markup) => results.push(markup))
  await settleRequests()
  cancel()
  finish('<svg>late</svg>')
  await settleRequests()
  assert.deepEqual(results, [])
  assert.equal(getCachedThumbnail('old'), undefined)
})

test('deck cache reset invalidates in-flight requests from the previous deck', async () => {
  resetThumbnailCache()
  const results = []
  let finish
  const pending = new Promise((resolve) => { finish = resolve })
  requestThumbnail('same-slide-id', () => pending, (markup) => results.push(markup))
  await settleRequests()
  resetThumbnailCache()
  requestThumbnail('same-slide-id', async () => '<svg>new</svg>', (markup) => results.push(markup))
  await settleRequests()
  finish('<svg>old</svg>')
  await settleRequests()
  assert.deepEqual(results, ['<svg>new</svg>'])
  assert.equal(getCachedThumbnail('same-slide-id'), '<svg>new</svg>')
})

test('cancelling before rendering starts skips unnecessary IPC', async () => {
  resetThumbnailCache()
  let renderCalls = 0
  const cancel = requestThumbnail('key', async () => {
    renderCalls += 1
    return '<svg />'
  }, () => assert.fail('cancelled request should not display'))
  cancel()
  await settleRequests()
  assert.equal(renderCalls, 0)
})

test('render errors produce the fallback and can be retried when visible again', async () => {
  resetThumbnailCache()
  const results = []
  requestThumbnail('key', async () => { throw new Error('render failed') }, (markup) => results.push(markup))
  await settleRequests()
  assert.equal(getCachedThumbnail('key'), undefined)
  requestThumbnail('key', async () => '<svg />', (markup) => results.push(markup))
  await settleRequests()
  assert.deepEqual(results, [null, '<svg />'])
})

test('cache returns only the exact render revision', () => {
  resetThumbnailCache()
  const key = thumbnailRevisionKey(slide, deckKey())
  cacheThumbnail(key, '<svg />')
  assert.equal(getCachedThumbnail(key), '<svg />')
  assert.equal(getCachedThumbnail(thumbnailRevisionKey(slide, deckKey({ mediaRevision: 3 }))), undefined)
})
