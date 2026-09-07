import assert from 'node:assert/strict'
import test from 'node:test'

import { renderedRectRelativeTo } from './slideRect.js'

test('uses the centered transformed canvas rectangle for overlay placement', () => {
  const rect = renderedRectRelativeTo(
    { left: 100, top: 50, width: 1200, height: 800 },
    { left: 348, top: 252, width: 704, height: 396 },
  )

  assert.deepEqual(rect, { x: 248, y: 202, w: 704, h: 396 })
})
