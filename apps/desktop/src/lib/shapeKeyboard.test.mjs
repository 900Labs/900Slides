import assert from 'node:assert/strict'
import test from 'node:test'
import { shapeKeyboardAction } from './shapeKeyboard.js'

const shape = {}
const editor = {}
const event = (key, target = shape) => ({ key, target, currentTarget: shape, defaultPrevented: false, isComposing: false })

test('spaces, newlines and Escape inside text and table editors remain native input', () => {
  for (const key of [' ', 'Enter', 'Escape', 'Backspace', 'ArrowLeft']) {
    assert.equal(shapeKeyboardAction(event(key, editor), true), null)
    assert.equal(shapeKeyboardAction(event(key, editor), false), null)
  }
})

test('focused shapes still support keyboard selection and text editing', () => {
  assert.equal(shapeKeyboardAction(event(' '), true), 'select')
  assert.equal(shapeKeyboardAction(event('Enter'), true), 'edit')
  assert.equal(shapeKeyboardAction(event('Enter'), false), 'select')
  assert.equal(shapeKeyboardAction(event('Escape'), true), 'exit')
})

test('IME confirmation and already-handled keys cannot activate a shape', () => {
  assert.equal(shapeKeyboardAction({ ...event('Enter'), isComposing: true }, true), null)
  assert.equal(shapeKeyboardAction({ ...event(' '), defaultPrevented: true }, true), null)
})
