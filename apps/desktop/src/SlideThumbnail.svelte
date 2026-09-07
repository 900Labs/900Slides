<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import type {
    PassthroughSnapshot,
    SlideSnapshot,
    TextBoxSnapshot,
  } from './lib/types'
  import {
    requestThumbnail,
    thumbnailRevisionKey,
  } from './lib/thumbnailCache.js'

  /** Props for a slide thumbnail. */
  interface Props {
    /** Slide data to preview. */
    slide: SlideSnapshot
    /** Signature of every deck-wide input used by the backend renderer. */
    deckRenderKey: string
    /** Aspect ratio for the thumbnail frame. */
    aspectRatio: string
    /** Whether this thumbnail is currently selected. */
    selected: boolean
    /** Click handler to select the slide. */
    onClick: () => void
  }

  let { slide, deckRenderKey, aspectRatio, selected, onClick }: Props = $props()

  /** Rendered SVG markup from the backend, or null while loading. */
  let svg = $state<string | null>(null)
  let visible = $state(false)
  let thumbnailElement = $state<HTMLButtonElement>()

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

  // Render only thumbnails in or near the viewport. Environments without
  // IntersectionObserver (including test harnesses) fall back to eager render.
  $effect(() => {
    const element = thumbnailElement
    if (!element) return
    if (typeof IntersectionObserver === 'undefined') {
      visible = true
      return
    }
    const observer = new IntersectionObserver(
      (entries) => {
        visible = entries.some((entry) => entry.isIntersecting)
      },
      { rootMargin: '160px 0px' },
    )
    observer.observe(element)
    return () => observer.disconnect()
  })

  $effect(() => {
    // Each offscreen component must release its SVG, otherwise it keeps an
    // unbounded second copy/reference after the shared LRU evicts the entry.
    svg = null
    if (!visible) return

    const key = thumbnailRevisionKey(slide, deckRenderKey)
    const slideId = slide.id
    return requestThumbnail(
      key,
      () => invoke<string>('render_slide_svg', { slideId }),
      (markup) => { svg = markup },
    )
  })
</script>

<button
  bind:this={thumbnailElement}
  class="thumbnail"
  class:selected
  onclick={onClick}
  type="button"
  aria-label={`Slide ${slide.id}`}
  style:aspect-ratio={aspectRatio}
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
