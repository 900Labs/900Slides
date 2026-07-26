<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
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

  /** Rendered SVG markup from the backend, or null while loading. */
  let svg = $state<string | null>(null)

  /** Builds a short text preview of the slide (fallback while SVG loads). */
  function previewText(): string {
    return slide.shapes
      .map((shape) => {
        if (shape.kind === 'text_box') {
          const textBox = shape.value as TextBoxSnapshot
          return textBox.paragraphs
            .map((paragraph) => paragraph.runs.map((run) => run.text).join(''))
            .join(' ')
        }
        if (shape.kind === 'passthrough') {
          const obj = shape.value as PassthroughSnapshot
          return `[${obj.label}]`
        }
        return ''
      })
      .join(' ')
      .trim()
  }

  // Re-render the thumbnail SVG whenever the slide identity or its shape list
  // changes (the backend hands back fresh snapshot objects after each command).
  $effect(() => {
    const id = slide.id
    const shapeCount = slide.shapes.length
    let cancelled = false
    svg = null
    invoke<string>('render_slide_svg', { slide_id: id })
      .then((markup) => {
        if (!cancelled && slide.id === id && slide.shapes.length === shapeCount) {
          svg = markup
        }
      })
      .catch(() => {
        if (!cancelled) svg = null
      })
    return () => {
      cancelled = true
    }
  })
</script>

<button
  class="thumbnail"
  class:selected
  onclick={onClick}
  type="button"
  aria-label={`Slide ${slide.id}`}
>
  {#if svg}
    <div class="preview-svg">{@html svg}</div>
  {:else}
    <div class="preview">{previewText() || '(blank)'}</div>
  {/if}
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
  .preview-svg {
    width: 100%;
    height: 100%;
  }
  .preview-svg :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
