import assert from 'node:assert/strict'
import test from 'node:test'

import { composeShapeTransform, supportsDirectResize } from './shapeTransform.js'

test('keeps a rotated line container aligned with its build transform', () => {
  assert.equal(
    composeShapeTransform(90, 'translateX(100%)'),
    'rotate(90deg) translateX(100%)',
  )
})

test('does not add a transform for an unrotated static shape', () => {
  assert.equal(composeShapeTransform(0, 'none'), undefined)
})

test('does not expose misleading resize handles for rotated line surfaces', () => {
  assert.equal(supportsDirectResize('geometric', 'line'), false)
  assert.equal(supportsDirectResize('geometric', 'rectangle'), true)
  assert.equal(supportsDirectResize('image'), true)
})
