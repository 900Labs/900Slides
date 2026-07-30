<script lang="ts">
  import type { RectDto } from './lib/types'

  interface Props {
    /** Current waypoints (EMU rects relative to the shape). */
    waypoints: RectDto[]
    /** Discards the in-progress path without saving. */
    onCancel: () => void
    /** Persists the path; pass null to clear all waypoints. */
    onSave: (path: RectDto[] | null) => void
  }

  let { waypoints, onCancel, onSave }: Props = $props()

  // svelte-ignore state_referenced_locally
  let draft = $state<RectDto[]>(waypoints.map((w) => ({ ...w })))
  /** Bound canvas element for imperative 2D rendering. */
  let canvasEl = $state<HTMLCanvasElement | null>(null)

  /** Fixed canvas size (CSS pixels) for the editor overlay. The canvas maps a
   *  logical region centered on the shape onto this fixed pixel grid. */
  const CANVAS = 320
  /** Half-extent of the logical region, in EMU, around the shape's top-left. A
   *  region of ~10 inches on each side gives room to draw a path. */
  const EXTENT_EMU = 9_144_000

  /** Maps a canvas pixel coordinate to an EMU offset relative to the shape's
   *  top-left corner. */
  function pixelToEmu(px: number, py: number): { x: number; y: number } {
    const x = ((px / CANVAS) * 2 - 1) * EXTENT_EMU
    const y = ((py / CANVAS) * 2 - 1) * EXTENT_EMU
    return { x, y }
  }

  /** Maps an EMU offset back to a canvas pixel coordinate. */
  function emuToPixel(x: number, y: number): { px: number; py: number } {
    const px = ((x / EXTENT_EMU + 1) / 2) * CANVAS
    const py = ((y / EXTENT_EMU + 1) / 2) * CANVAS
    return { px, py }
  }

  const SHAPE_SIZE = 22

  function handleClick(event: MouseEvent): void {
    const canvas = event.currentTarget as HTMLCanvasElement
    const rect = canvas.getBoundingClientRect()
    const px = event.clientX - rect.left
    const py = event.clientY - rect.top
    const { x, y } = pixelToEmu(px, py)
    // Store the waypoint as a degenerate rect (a point) in EMU.
    draft = [...draft, { x, y, width: 0, height: 0 }]
  }

  function undo(): void {
    draft = draft.slice(0, -1)
  }

  function clearAll(): void {
    draft = []
  }

  function save(): void {
    onSave(draft.length > 0 ? draft : null)
  }

  /** Renders the canvas: grid, shape origin, path segments, and waypoints. */
  function render(): void {
    if (!canvasEl) return
    const ctx = canvasEl.getContext('2d')
    if (!ctx) return
    ctx.clearRect(0, 0, CANVAS, CANVAS)

    // Background + border.
    ctx.fillStyle = '#fafafa'
    ctx.fillRect(0, 0, CANVAS, CANVAS)
    ctx.strokeStyle = '#ccc'
    ctx.lineWidth = 1
    ctx.strokeRect(0.5, 0.5, CANVAS - 1, CANVAS - 1)

    // Axes through the shape origin (canvas center).
    ctx.strokeStyle = '#e2e2e2'
    ctx.beginPath()
    ctx.moveTo(CANVAS / 2, 0)
    ctx.lineTo(CANVAS / 2, CANVAS)
    ctx.moveTo(0, CANVAS / 2)
    ctx.lineTo(CANVAS, CANVAS / 2)
    ctx.stroke()

    const origin = { px: CANVAS / 2, py: CANVAS / 2 }

    // Shape origin marker.
    ctx.fillStyle = '#4a90d9'
    ctx.fillRect(origin.px - SHAPE_SIZE / 2, origin.py - SHAPE_SIZE / 2, SHAPE_SIZE, SHAPE_SIZE)
    ctx.strokeStyle = '#2c6fb0'
    ctx.strokeRect(origin.px - SHAPE_SIZE / 2, origin.py - SHAPE_SIZE / 2, SHAPE_SIZE, SHAPE_SIZE)

    if (draft.length === 0) return

    // Path from origin through each waypoint.
    ctx.strokeStyle = '#d94a4a'
    ctx.lineWidth = 2
    ctx.beginPath()
    ctx.moveTo(origin.px, origin.py)
    for (const w of draft) {
      const { px, py } = emuToPixel(w.x, w.y)
      ctx.lineTo(px, py)
    }
    ctx.stroke()

    // Waypoint dots.
    ctx.fillStyle = '#d94a4a'
    for (const w of draft) {
      const { px, py } = emuToPixel(w.x, w.y)
      ctx.beginPath()
      ctx.arc(px, py, 4, 0, Math.PI * 2)
      ctx.fill()
    }
  }

  // Re-render whenever the draft changes or the canvas mounts.
  $effect(() => {
    // Track draft length and the bound canvas element.
    draft.length
    canvasEl
    render()
  })
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="Motion path editor"
  tabindex="-1"
  onclick={(e) => {
    if (e.target === e.currentTarget) onCancel()
  }}
  onkeydown={(e) => {
    if (e.key === 'Escape') onCancel()
  }}
>
  <div class="editor">
    <h4>Motion Path</h4>
    <p class="hint">Click to add waypoints. Red line shows the path from the shape.</p>
    <canvas
      bind:this={canvasEl}
      width={CANVAS}
      height={CANVAS}
      onclick={handleClick}
    ></canvas>
    <div class="editor-actions">
      <button type="button" onclick={undo} disabled={draft.length === 0}>Undo</button>
      <button type="button" onclick={clearAll} disabled={draft.length === 0}>Clear</button>
      <span class="spacer"></span>
      <button type="button" onclick={onCancel}>Cancel</button>
      <button type="button" onclick={save}>Save</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .editor {
    background: #fff;
    border-radius: 8px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25);
  }
  .editor h4 {
    margin: 0;
  }
  .hint {
    margin: 0;
    font-size: 0.8rem;
    opacity: 0.75;
  }
  canvas {
    border: 1px solid #ccc;
    cursor: crosshair;
    background: #fafafa;
  }
  .editor-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .spacer {
    flex: 1;
  }
</style>
