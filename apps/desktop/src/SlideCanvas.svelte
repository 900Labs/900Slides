<script lang="ts">
  import type {
    ColorDto,
    ParagraphDto,
    PassthroughSnapshot,
    RunDto,
    SlideSnapshot,
    TextBoxSnapshot,
  } from './lib/types'

  /** Props for the slide canvas. */
  interface Props {
    /** Slide to render. */
    slide: SlideSnapshot
    /** Deck background color. */
    background: ColorDto
    /** Callback invoked when a text box is edited. */
    onEditTextBox?: (detail: {
      slideId: string
      shapeIndex: number
      paragraphs: ParagraphDto[]
    }) => void
    /** Whether the canvas is read-only (presenter mode). */
    readonly?: boolean
  }

  let { slide, background, onEditTextBox, readonly = false }: Props = $props()

  /** EMU to CSS pixels for a 1280x720 (16:9) canvas. */
  const EMU_TO_PX = 1.0 / 9525.0

  /** Converts EMU to a pixel CSS string. */
  function toPx(emu: number): string {
    return `${emu * EMU_TO_PX}px`
  }

  /** Converts a ColorDto to a CSS rgba string. */
  function toRgba(color: ColorDto): string {
    return `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a / 255})`
  }

  /** Joins all runs in all paragraphs into a single editable string. */
  function textFromParagraphs(paragraphs: ParagraphDto[]): string {
    return paragraphs
      .map((paragraph) => paragraph.runs.map((run) => run.text).join(''))
      .join('\n')
  }

  /** Concatenates the run text of a single paragraph. */
  function paragraphText(paragraph: ParagraphDto): string {
    return paragraph.runs.map((run) => run.text).join('')
  }

  /** Builds a paragraph DTO, preserving original runs when the text is unchanged. */
  function buildParagraph(
    text: string,
    original: ParagraphDto | undefined,
  ): ParagraphDto {
    if (original && paragraphText(original) === text) {
      return original
    }
    return {
      runs: text ? [{ text, bold: false, italic: false, underline: false }] : [],
      listStyle: original?.listStyle ?? 'none',
    }
  }

  /** Emits a text-box edit command after splitting the textarea value into paragraphs. */
  function handleBlur(event: FocusEvent, shapeIndex: number): void {
    if (!onEditTextBox) return
    const target = event.target as HTMLTextAreaElement
    const textBox = (slide.shapes[shapeIndex].value as TextBoxSnapshot)
    const originalParagraphs = textBox.paragraphs
    const lines = target.value.split('\n')

    const newParagraphs: ParagraphDto[] = lines.map((line, index) =>
      buildParagraph(line, originalParagraphs[index]),
    )

    const changed =
      newParagraphs.length !== originalParagraphs.length ||
      newParagraphs.some((paragraph, index) => {
        const original = originalParagraphs[index]
        if (!original) return true
        if (paragraph.runs.length !== original.runs.length) return true
        return paragraph.runs.some(
          (run, runIndex) =>
            run.text !== original.runs[runIndex]?.text ||
            run.bold !== original.runs[runIndex]?.bold ||
            run.italic !== original.runs[runIndex]?.italic ||
            run.underline !== original.runs[runIndex]?.underline,
        )
      })

    if (changed) {
      onEditTextBox({
        slideId: slide.id,
        shapeIndex,
        paragraphs: newParagraphs,
      })
    }
  }
</script>

<div
  class="canvas"
  style:background-color={toRgba(background)}
  role="application"
  aria-label="Slide canvas"
>
  {#each slide.shapes as shape, shapeIndex}
    {#if shape.kind === 'text_box'}
      {@const textBox = shape.value as TextBoxSnapshot}
      <div
        class="text-box-container"
        style:left={toPx(textBox.frame.x)}
        style:top={toPx(textBox.frame.y)}
        style:width={toPx(textBox.frame.width)}
        style:height={toPx(textBox.frame.height)}
      >
        {#if readonly}
          <div class="text-box-readonly">
            {#each textBox.paragraphs as paragraph}
              <p>
                {#each paragraph.runs as run}
                  <span class:bold={run.bold} class:italic={run.italic} class:underline={run.underline}>{run.text}</span>
                {/each}
              </p>
            {/each}
          </div>
        {:else}
          <textarea
            class="text-box"
            value={textFromParagraphs(textBox.paragraphs)}
            onblur={(event) => handleBlur(event, shapeIndex)}
            aria-label="Editable text box"
          ></textarea>
        {/if}
      </div>
    {:else if shape.kind === 'passthrough'}
      {@const obj = shape.value as PassthroughSnapshot}
      {@const passthroughIndex = slide.shapes.filter((s, i) => s.kind === 'passthrough' && i < shapeIndex).length}
      <div
        class="passthrough"
        style:left={obj.frame ? toPx(obj.frame.x) : undefined}
        style:top={obj.frame ? toPx(obj.frame.y) : `${1 + passthroughIndex * 0.5}rem`}
        style:right={obj.frame ? undefined : '1rem'}
        style:width={obj.frame ? toPx(obj.frame.width) : undefined}
        style:height={obj.frame ? toPx(obj.frame.height) : undefined}
      >
        [preserved object: {obj.label}]
      </div>
    {/if}
  {/each}
</div>

<style>
  .canvas {
    position: relative;
    width: 1280px;
    height: 720px;
    flex-shrink: 0;
    box-shadow: 0 0 0 1px #ccc;
    overflow: hidden;
  }
  .text-box-container {
    position: absolute;
  }
  .text-box {
    width: 100%;
    height: 100%;
    border: 1px dashed #999;
    background: transparent;
    resize: none;
    font-family: inherit;
    font-size: 1rem;
    line-height: 1.3;
    padding: 0.25rem;
  }
  .text-box:focus {
    outline: 2px solid #0070c0;
  }
  .text-box-readonly {
    width: 100%;
    height: 100%;
    padding: 0.25rem;
  }
  .text-box-readonly p {
    margin: 0 0 0.5rem;
  }
  .bold {
    font-weight: bold;
  }
  .italic {
    font-style: italic;
  }
  .underline {
    text-decoration: underline;
  }
  .passthrough {
    position: absolute;
    padding: 0.5rem;
    border: 2px dashed #c00;
    color: #c00;
    background: rgba(255, 255, 255, 0.9);
  }
</style>
