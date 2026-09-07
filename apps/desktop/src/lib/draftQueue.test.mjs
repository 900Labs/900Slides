import assert from 'node:assert/strict'
import test from 'node:test'
import { createDraftQueue } from './draftQueue.js'

test('flush coalesces drafts and preserves edits typed during an older commit', async () => {
  const calls = []
  const dirty = []
  let release
  const gate = new Promise((resolve) => { release = resolve })
  const queue = createDraftQueue({
    commit: async (value) => { calls.push(value); if (value === 'second') await gate },
    onChange: (value) => dirty.push(value),
  })
  queue.set('text', 'first')
  queue.set('text', 'second')
  const flushed = queue.flush()
  await Promise.resolve()
  queue.set('text', 'third')
  queue.set('cell', 'table value')
  assert.equal(queue.peek('text'), 'third')
  assert.equal(queue.flush(), flushed)
  release()
  await flushed
  assert.deepEqual(calls, ['second', 'third', 'table value'])
  assert.equal(dirty.at(-1), false)
  assert.equal(queue.isDirty(), false)
})

test('a failed commit retains the newest draft and explicit flush retries it', async () => {
  let fail = true
  const calls = []
  const queue = createDraftQueue({ commit: async (value) => {
    calls.push(value)
    if (fail) { queue.set('notes', 'newest'); throw new Error('storage unavailable') }
  } })
  queue.set('notes', 'initial')
  await assert.rejects(queue.flush(), /storage unavailable/)
  assert.equal(queue.isDirty(), true)
  assert.equal(queue.peek('notes'), 'newest')
  fail = false
  await queue.flush()
  assert.deepEqual(calls, ['initial', 'newest'])
  assert.equal(queue.isDirty(), false)
})

test('continuous typing commits by the maximum wait and dispose cancels timers', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  const calls = []
  const queue = createDraftQueue({ commit: (value) => { calls.push(value) }, delay: 600, maxWait: 2000 })
  for (let index = 0; index < 4; index += 1) {
    queue.set('text', `version ${index}`)
    t.mock.timers.tick(500)
  }
  await Promise.resolve()
  assert.deepEqual(calls, ['version 3'])
  await queue.flush()
  queue.set('text', 'unmounted')
  queue.dispose()
  t.mock.timers.tick(3000)
  await Promise.resolve()
  assert.deepEqual(calls, ['version 3'])
})
