import { test } from 'node:test'
import assert from 'node:assert/strict'
import { confirmDocumentTransition, documentFileName } from './documentLifecycle.js'

function actions({ dirty = true, choice = 'save', saved = true, flushError, saveError } = {}) {
  const calls = []
  return {
    calls,
    flush: async () => { calls.push('flush'); if (flushError) throw flushError },
    isDirty: async () => { calls.push('status'); return dirty },
    recover: async () => { calls.push('recover') },
    decide: async () => { calls.push('decide'); return choice },
    save: async () => { calls.push('save'); if (saveError) throw saveError; return saved },
  }
}

test('save flushes drafts and writes recovery before asking to replace work', async () => {
  const a = actions()
  assert.equal(await confirmDocumentTransition(a), 'continue')
  assert.deepEqual(a.calls, ['flush', 'status', 'recover', 'decide', 'save'])
})

test('cancelling Save As keeps the document open', async () => {
  assert.equal(await confirmDocumentTransition(actions({ saved: false })), 'cancel')
})

test('failed draft or disk save prevents replacement', async () => {
  const failedDraft = actions({ flushError: Error('edit failed') })
  await assert.rejects(confirmDocumentTransition(failedDraft), /edit failed/)
  assert.deepEqual(failedDraft.calls, ['flush'])
  await assert.rejects(confirmDocumentTransition(actions({ saveError: Error('disk full') })), /disk full/)
})

test('only explicit discard allows replacement without saving', async () => {
  for (const choice of ['discard', 'cancel']) {
    const a = actions({ choice })
    assert.equal(await confirmDocumentTransition(a), choice)
    assert.ok(!a.calls.includes('save'))
  }
})

test('clean documents still flush pending editor changes before checking status', async () => {
  const a = actions({ dirty: false })
  assert.equal(await confirmDocumentTransition(a), 'continue')
  assert.deepEqual(a.calls, ['flush', 'status'])
})

test('document names support native paths from all desktop platforms', () => {
  assert.equal(documentFileName('C:\\Lessons\\Week 1.pptx'), 'Week 1.pptx')
  assert.equal(documentFileName('/lessons/Week 1.pptx'), 'Week 1.pptx')
  assert.equal(documentFileName(null), 'Untitled presentation')
})
