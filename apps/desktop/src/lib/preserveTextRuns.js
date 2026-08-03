// @ts-nocheck

/**
 * Preserves formatting in untouched portions of a plain-text textarea edit.
 * The inserted middle segment inherits the run at the insertion boundary.
 */
export function preserveEditedRuns(text, original) {
  const previousText = original.runs.map((run) => run.text).join('')
  let prefix = 0
  while (
    prefix < previousText.length &&
    prefix < text.length &&
    previousText[prefix] === text[prefix]
  ) {
    prefix += 1
  }

  let suffix = 0
  while (
    suffix < previousText.length - prefix &&
    suffix < text.length - prefix &&
    previousText[previousText.length - 1 - suffix] === text[text.length - 1 - suffix]
  ) {
    suffix += 1
  }

  const output = []
  appendOriginalRange(output, original, 0, prefix)
  const inserted = text.slice(prefix, text.length - suffix)
  if (inserted) {
    appendStyledRun(output, { ...runAtInsertionPoint(original, prefix), text: inserted })
  }
  appendOriginalRange(output, original, previousText.length - suffix, previousText.length)
  return output
}

/**
 * Aligns new textarea lines with source paragraphs. Exact paragraph matches
 * use an LCS so an inserted or deleted line never shifts formatting from the
 * next unchanged paragraph. A changed line inherits an old paragraph only
 * when its surrounding segment is a one-for-one replacement.
 */
export function alignParagraphs(lines, originals) {
  const oldTexts = originals.map((paragraph) => paragraph.runs.map((run) => run.text).join(''))
  const rows = oldTexts.length + 1
  const cols = lines.length + 1
  const table = Array.from({ length: rows }, () => Array(cols).fill(0))
  for (let oldIndex = oldTexts.length - 1; oldIndex >= 0; oldIndex -= 1) {
    for (let newIndex = lines.length - 1; newIndex >= 0; newIndex -= 1) {
      table[oldIndex][newIndex] =
        oldTexts[oldIndex] === lines[newIndex]
          ? table[oldIndex + 1][newIndex + 1] + 1
          : Math.max(table[oldIndex + 1][newIndex], table[oldIndex][newIndex + 1])
    }
  }

  const matches = []
  let oldIndex = 0
  let newIndex = 0
  while (oldIndex < oldTexts.length && newIndex < lines.length) {
    if (oldTexts[oldIndex] === lines[newIndex]) {
      matches.push([oldIndex, newIndex])
      oldIndex += 1
      newIndex += 1
    } else if (table[oldIndex + 1][newIndex] >= table[oldIndex][newIndex + 1]) {
      oldIndex += 1
    } else {
      newIndex += 1
    }
  }

  const aligned = Array(lines.length).fill(undefined)
  const anchors = [[-1, -1], ...matches, [oldTexts.length, lines.length]]
  for (let index = 0; index < anchors.length - 1; index += 1) {
    const [oldStartAnchor, newStartAnchor] = anchors[index]
    const [oldEnd, newEnd] = anchors[index + 1]
    const oldStart = oldStartAnchor + 1
    const newStart = newStartAnchor + 1
    const oldCount = oldEnd - oldStart
    const newCount = newEnd - newStart
    if (oldCount === newCount) {
      for (let offset = 0; offset < newCount; offset += 1) {
        aligned[newStart + offset] = originals[oldStart + offset]
      }
    }
    if (newEnd < lines.length && oldEnd < originals.length) {
      aligned[newEnd] = originals[oldEnd]
    }
  }
  return aligned
}

function runAtInsertionPoint(original, offset) {
  let cursor = 0
  for (const run of original.runs) {
    const end = cursor + run.text.length
    if (offset < end) return run
    cursor = end
  }
  return original.runs[original.runs.length - 1] ?? {
    text: '',
    bold: false,
    italic: false,
    underline: false,
    strikethrough: false,
    verticalAlign: 'baseline',
    code: false,
  }
}

function appendStyledRun(runs, run) {
  if (!run.text) return
  const previous = runs[runs.length - 1]
  if (
    previous &&
    previous.bold === run.bold &&
    previous.italic === run.italic &&
    previous.underline === run.underline &&
    previous.strikethrough === run.strikethrough &&
    previous.verticalAlign === run.verticalAlign &&
    previous.code === run.code &&
    previous.fontFamily === run.fontFamily
  ) {
    previous.text += run.text
    return
  }
  runs.push({ ...run })
}

function appendOriginalRange(output, original, start, end) {
  let cursor = 0
  for (const run of original.runs) {
    const runStart = cursor
    const runEnd = cursor + run.text.length
    const from = Math.max(start, runStart)
    const to = Math.min(end, runEnd)
    if (from < to) {
      appendStyledRun(output, {
        ...run,
        text: run.text.slice(from - runStart, to - runStart),
      })
    }
    cursor = runEnd
  }
}
