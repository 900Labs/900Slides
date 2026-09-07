import assert from 'node:assert/strict'
import test from 'node:test'

import { fitCanvasScale, slideSurfaceDimensions } from './canvasScale.js'

test('fits a 1280x720 slide in the default editor content width', () => {
  const scale = fitCanvasScale(752, 744, 12_192_000, 6_858_000)
  assert.equal(scale, 0.55)
  assert.ok(1280 * scale <= 752 - 48)
  assert.ok(720 * scale <= 744 - 48)
})

test('does not enlarge a slide when the canvas has excess space', () => {
  assert.equal(fitCanvasScale(1800, 1200, 12_192_000, 6_858_000), 1)
})

test('keeps an unscaled slide surface inside a scaled layout footprint', () => {
  const dimensions = slideSurfaceDimensions(12_192_000, 6_858_000, 0.55)

  assert.equal(dimensions.logicalWidthPx, 1280)
  assert.equal(dimensions.logicalHeightPx, 720)
  assert.equal(dimensions.footprintWidthPx, 704)
  assert.equal(dimensions.footprintHeightPx, 396.00000000000006)
  assert.equal(dimensions.surfaceScale, 0.55)
})
