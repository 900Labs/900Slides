import assert from 'node:assert/strict'
import test from 'node:test'

import { alignParagraphs, preserveEditedRuns } from './preserveTextRuns.js'

const baseline = {
  listStyle: 'none',
  style: { blockquote: false, codeBlock: false, indentLevel: 0 },
}

test('appending text retains bold and italic run formatting', () => {
  const runs = preserveEditedRuns('Bold italic!', {
    ...baseline,
    runs: [
      { text: 'Bold ', bold: true, italic: false, underline: false, strikethrough: false, verticalAlign: 'baseline', code: false },
      { text: 'italic', bold: false, italic: true, underline: false, strikethrough: false, verticalAlign: 'baseline', code: false },
    ],
  })
  assert.deepEqual(
    runs.map(({ text, bold, italic }) => ({ text, bold, italic })),
    [
      { text: 'Bold ', bold: true, italic: false },
      { text: 'italic!', bold: false, italic: true },
    ],
  )
})

test('editing one multi-run span retains formatting on untouched prefix and suffix', () => {
  const runs = preserveEditedRuns('Bold plain italic', {
    ...baseline,
    runs: [
      { text: 'Bold ', bold: true, italic: false, underline: false, strikethrough: false, verticalAlign: 'baseline', code: false },
      { text: 'middle', bold: false, italic: false, underline: false, strikethrough: false, verticalAlign: 'baseline', code: false },
      { text: ' italic', bold: false, italic: true, underline: false, strikethrough: false, verticalAlign: 'baseline', code: false },
    ],
  })
  assert.deepEqual(
    runs.map(({ text, bold, italic }) => ({ text, bold, italic })),
    [
      { text: 'Bold ', bold: true, italic: false },
      { text: 'plain', bold: false, italic: false },
      { text: ' italic', bold: false, italic: true },
    ],
  )
})

test('inserting a paragraph keeps later unchanged paragraph formatting aligned', () => {
  const title = { ...baseline, runs: [{ text: 'Title', bold: true, italic: false }] }
  const body = { ...baseline, runs: [{ text: 'Body', bold: false, italic: true }] }
  const aligned = alignParagraphs(['Title', 'Inserted', 'Body'], [title, body])
  assert.equal(aligned[0], title)
  assert.equal(aligned[1], undefined)
  assert.equal(aligned[2], body)
})

test('deleting an intervening paragraph keeps the remaining runs unchanged', () => {
  const title = { ...baseline, runs: [{ text: 'Title', bold: true, italic: false }] }
  const removed = { ...baseline, runs: [{ text: 'Removed', bold: false, italic: false }] }
  const body = { ...baseline, runs: [{ text: 'Body', bold: false, italic: true }] }
  const aligned = alignParagraphs(['Title', 'Body'], [title, removed, body])
  assert.equal(aligned[0], title)
  assert.equal(aligned[1], body)
})
