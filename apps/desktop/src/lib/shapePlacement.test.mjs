import assert from 'node:assert/strict'
import test from 'node:test'

import { MIN_LINE_THICKNESS_EMU, linePlacementFromPoints } from './shapePlacement.js'

test('creates a thin horizontal surface from a horizontal line drag', () => {
  assert.deepEqual(
    linePlacementFromPoints({ x: 100_000, y: 300_000 }, { x: 900_000, y: 300_000 }),
    {
      frame: {
        x: 100_000,
        y: 300_000 - MIN_LINE_THICKNESS_EMU / 2,
        width: 800_000,
        height: MIN_LINE_THICKNESS_EMU,
      },
      rotation: 0,
    },
  )
})

test('creates a rotated thin surface from a vertical line drag', () => {
  const placement = linePlacementFromPoints({ x: 300_000, y: 100_000 }, { x: 300_000, y: 900_000 })
  assert.deepEqual(placement?.frame, {
      x: -100_000,
      y: 500_000 - MIN_LINE_THICKNESS_EMU / 2,
      width: 800_000,
      height: MIN_LINE_THICKNESS_EMU,
  })
  assert.equal(placement?.rotation, 90)
})

test('preserves a diagonal line angle and endpoint length', () => {
  const placement = linePlacementFromPoints({ x: 100_000, y: 200_000 }, { x: 900_000, y: 600_000 })
  assert.ok(placement)
  assert.equal(placement.rotation, 26.56505117707799)
  assert.ok(Math.abs(placement.frame.width - Math.hypot(800_000, 400_000)) < 0.000001)
  assert.equal(placement.frame.height, MIN_LINE_THICKNESS_EMU)
})

test('keeps a click as the default-shape fallback rather than a zero-length line', () => {
  assert.equal(linePlacementFromPoints({ x: 300_000, y: 300_000 }, { x: 300_000, y: 300_000 }), null)
})
