<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import SlideCanvas from './SlideCanvas.svelte'
  import type { ColorDto, PresenterState, SlideSnapshot } from './lib/types'

  /** Presenter view state from Rust. */
  let presenterState = $state<PresenterState | null>(null)
  /** Elapsed seconds since the presenter opened. */
  let elapsed = $state(0)
  /** Timer interval handle. */
  let timer = $state<ReturnType<typeof setInterval> | null>(null)

  $effect(() => {
    refresh()
    timer = setInterval(() => {
      elapsed += 1
    }, 1000)
    const keyHandler = (event: KeyboardEvent) => handleKey(event)
    window.addEventListener('keydown', keyHandler)
    return () => {
      if (timer) clearInterval(timer)
      window.removeEventListener('keydown', keyHandler)
    }
  })

  /** Refreshes presenter state from the backend. */
  async function refresh(): Promise<void> {
    presenterState = await invoke<PresenterState>('get_presenter_state')
  }

  /** Advances to the next slide. */
  async function next(): Promise<void> {
    presenterState = await invoke<PresenterState>('presenter_next')
  }

  /** Goes to the previous slide. */
  async function previous(): Promise<void> {
    presenterState = await invoke<PresenterState>('presenter_previous')
  }

  /** Jumps to the first slide. */
  async function first(): Promise<void> {
    while (presenterState && presenterState.slideNumber > 1) {
      presenterState = await invoke<PresenterState>('presenter_previous')
    }
  }

  /** Jumps to the last slide. */
  async function last(): Promise<void> {
    while (presenterState && presenterState.slideNumber < presenterState.total) {
      presenterState = await invoke<PresenterState>('presenter_next')
    }
  }

  /** Closes the presenter window. */
  function close(): void {
    window.close()
  }

  /** Keyboard control for presenter navigation. */
  function handleKey(event: KeyboardEvent): void {
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown' || event.key === ' ' || event.key === 'PageDown') {
      event.preventDefault()
      next()
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp' || event.key === 'PageUp') {
      event.preventDefault()
      previous()
    } else if (event.key === 'Home') {
      event.preventDefault()
      first()
    } else if (event.key === 'End') {
      event.preventDefault()
      last()
    } else if (event.key === 'Escape') {
      event.preventDefault()
      close()
    }
  }

  /** Formats elapsed seconds as mm:ss. */
  function formatTime(seconds: number): string {
    const m = Math.floor(seconds / 60)
      .toString()
      .padStart(2, '0')
    const s = (seconds % 60).toString().padStart(2, '0')
    return `${m}:${s}`
  }

  /** Returns a readable background color for a slide, or white. */
  function backgroundColor(): ColorDto {
    return presenterState?.currentSlide ? { r: 255, g: 255, b: 255, a: 255 } : { r: 255, g: 255, b: 255, a: 255 }
  }
</script>

<div class="presenter" role="application" aria-label="Presenter view" tabindex="-1">
  {#if presenterState}
    <div class="stage">
      <SlideCanvas
        slide={presenterState.currentSlide}
        background={backgroundColor()}
        readonly
      />
    </div>

    <div class="hud">
      <div class="next">
        {#if presenterState.nextSlide}
          <SlideCanvas
            slide={presenterState.nextSlide}
            background={backgroundColor()}
            readonly
          />
        {:else}
          <div class="end">End of presentation</div>
        {/if}
      </div>

      <div class="info">
        <span class="counter">{presenterState.slideNumber} / {presenterState.total}</span>
        <span class="timer">{formatTime(elapsed)}</span>
      </div>

      <div class="notes" aria-label="Speaker notes">
        {presenterState.notes || 'No notes'}
      </div>
    </div>
  {/if}
</div>

<style>
  :global(body) {
    margin: 0;
    background: #111;
    color: #fff;
    font-family: system-ui, sans-serif;
  }
  .presenter {
    display: flex;
    height: 100vh;
    padding: 1rem;
    gap: 1rem;
  }
  .stage {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .hud {
    width: 320px;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .next {
    flex: 0 0 auto;
  }
  .end {
    width: 320px;
    height: 180px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #222;
    border: 1px solid #444;
  }
  .info {
    display: flex;
    justify-content: space-between;
    font-size: 1.5rem;
    font-variant-numeric: tabular-nums;
  }
  .notes {
    flex: 1;
    background: #222;
    border: 1px solid #444;
    padding: 0.75rem;
    overflow-y: auto;
    white-space: pre-wrap;
  }
  :global(.hud .canvas) {
    width: 320px !important;
    height: 180px !important;
  }
</style>
