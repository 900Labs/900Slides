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
    onEdit?: (detail: {
      slideId: string
      shapeIndex: number
      paragraphIndex: number
      runs: RunDto[]
    }) => void
    /** Whether the canvas is read-only (presenter mode). */
    readonly?: boolean
  }

  let { slide, background, onEdit, readonly = false }: Props = $props()

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

  /** Emits a text edit command for the given shape and paragraph. */
  function handleBlur(
    event: FocusEvent,
    shapeIndex: number,
    paragraphIndex: number,
  ): void {
    if (!onEdit) return
    const target = event.target as HTMLTextAreaElement
    const text = target.value
    const runs: RunDto[] = text
      ? [{ text, bold: false, italic: false, underline: false }]
      : []
    onEdit({
      slideId: slide.id,
      shapeIndex,
      paragraphIndex,
      runs,
    })
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
            onblur={(event) => handleBlur(event, shapeIndex, 0)}
            aria-label="Editable text box"
          ></textarea>
        {/if}
      </div>
    {:else if shape.kind === 'passthrough'}
      {@const obj = shape.value as PassthroughSnapshot}
      <div class="passthrough">
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
    top: 1rem;
    right: 1rem;
    padding: 0.5rem;
    border: 2px dashed #c00;
    color: #c00;
    background: rgba(255, 255, 255, 0.9);
  }
</style>
