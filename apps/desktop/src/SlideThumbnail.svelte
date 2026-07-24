<script lang="ts">
  import type {
    PassthroughSnapshot,
    SlideSnapshot,
    TextBoxSnapshot,
  } from './lib/types'

  /** Props for a slide thumbnail. */
  interface Props {
    /** Slide data to preview. */
    slide: SlideSnapshot
    /** Whether this thumbnail is currently selected. */
    selected: boolean
    /** Click handler to select the slide. */
    onClick: () => void
  }

  let { slide, selected, onClick }: Props = $props()

  /** Builds a short text preview of the slide. */
  function previewText(): string {
    return slide.shapes
      .map((shape) => {
        if (shape.kind === 'text_box') {
          const textBox = shape.value as TextBoxSnapshot
          return textBox.paragraphs
            .map((paragraph) => paragraph.runs.map((run) => run.text).join(''))
            .join(' ')
        }
        const obj = shape.value as PassthroughSnapshot
        return `[${obj.label}]`
      })
      .join(' ')
      .trim()
  }
</script>

<button
  class="thumbnail"
  class:selected
  onclick={onClick}
  type="button"
  aria-label={`Slide ${slide.id}`}
>
  <div class="preview">{previewText() || '(blank)'}</div>
</button>

<style>
  .thumbnail {
    width: 100%;
    aspect-ratio: 16 / 9;
    padding: 0.25rem;
    margin-bottom: 0.5rem;
    border: 1px solid #ccc;
    background: #fff;
    cursor: pointer;
    text-align: left;
    overflow: hidden;
  }
  .thumbnail.selected {
    border-color: #0070c0;
    box-shadow: 0 0 0 2px #0070c0;
  }
  .preview {
    font-size: 0.6rem;
    line-height: 1.2;
    color: #333;
    word-break: break-word;
  }
</style>
