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
  assert.deepEqual(aligned[1], { ...title, runs: [{ ...title.runs[0], text: '' }] })
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

test('editing retains adjacent runs with different sizes, colors and hyperlinks', () => {
  const runs = [
    { text: 'Large', fontSize: 457200, color: { r: 10, g: 20, b: 30, a: 255 } },
    { text: ' small', fontSize: 228600, color: { r: 10, g: 20, b: 30, a: 255 } },
    { text: ' blue', fontSize: 228600, color: { r: 20, g: 80, b: 190, a: 255 } },
    { text: ' link', fontSize: 228600, color: { r: 20, g: 80, b: 190, a: 255 }, link: { url: 'https://example.org' } },
  ]
  const edited = preserveEditedRuns('Large small blue link!', { ...baseline, runs })
  assert.deepEqual(edited, [...runs.slice(0, 3), { ...runs[3], text: ' link!' }])
})

test('Return then later typing keeps an empty title paragraph ready with the same typography', () => {
  const title = {
    ...baseline,
    runs: [{ text: 'Community learning day', bold: true, fontSize: 457200, fontFamily: 'Georgia', color: { r: 30, g: 60, b: 90, a: 255 } }],
  }
  const afterReturn = alignParagraphs(['Community learning day', ''], [title])
  assert.equal(afterReturn[0], title)
  assert.deepEqual(afterReturn[1], { ...title, runs: [{ ...title.runs[0], text: '' }] })
  // Canvas retains that empty source paragraph; serializing models an
  // autosave before the next keystroke arrives.
  const saved = JSON.parse(JSON.stringify(afterReturn))
  const afterTyping = alignParagraphs(['Community learning day', 'Everyone is welcome'], saved)
  const runs = preserveEditedRuns('Everyone is welcome', afterTyping[1])
  assert.deepEqual(runs, [{ ...title.runs[0], text: 'Everyone is welcome' }])
})

test('splitting mixed-size runs retains each fragment and the later body paragraph', () => {
  const title = {
    ...baseline,
    runs: [{ text: 'AB', bold: true, fontSize: 457200 }, { text: 'CD', italic: true, fontSize: 228600 }],
  }
  const body = { ...baseline, runs: [{ text: 'Body', fontSize: 190500 }] }
  const aligned = alignParagraphs(['AB', 'CD', 'Body'], [title, body])
  assert.deepEqual(aligned[0].runs, [title.runs[0]])
  assert.deepEqual(aligned[1].runs, [title.runs[1]])
  assert.equal(aligned[2], body)
})

test('inserting a paragraph between mixed-format paragraphs inherits only the boundary run', () => {
  const previous = { ...baseline, runs: [{ text: 'Large ', fontSize: 457200 }, { text: 'small', fontSize: 228600 }] }
  const next = { ...baseline, runs: [{ text: 'Next', fontSize: 190500 }] }
  const aligned = alignParagraphs(['Large small', '', 'Next'], [previous, next])
  assert.deepEqual(aligned[1].runs, [{ text: '', fontSize: 228600 }])
  assert.equal(aligned[2], next)
})
