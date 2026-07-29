<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn, type Event } from '@tauri-apps/api/event'
  import SlideCanvas from './SlideCanvas.svelte'
  import type { ColorDto, PresenterState, ProjectorFiltersDto } from './lib/types'
  import {
    PRESENTER_EVENTS,
    defaultProjectorFilters,
    projectorFilterCss,
    runMorph,
    slideRectFromStage,
    type BlankMode,
    type HighlighterStroke,
    type LaserPayload,
    type MorphPayload,
    type SlideRect,
  } from './lib/presenter'

  /** Presenter state mirrored from the control window. */
  let presenterState = $state<PresenterState | null>(null)
  /** Build-step index synced from the control window. */
  let activeBuildStep = $state<number>(Infinity)
  /** Active code-step index synced from the control window. */
  let activeCodeStep = $state<number>(0)
  /** Mirrored laser dot (null when hidden). */
  let laser = $state<LaserPayload | null>(null)
  /** Mirrored highlighter strokes. */
  let strokes = $state<HighlighterStroke[]>([])
  /** Audience blank mode. */
  let blankMode = $state<BlankMode>('none')
  /** Projector CSS filters applied to the slide container. */
  let appliedFilters = $state<ProjectorFiltersDto>(defaultProjectorFilters())
  /** CSS `filter` string derived from {@link appliedFilters}. */
  let filterCss = $derived(projectorFilterCss(appliedFilters))
  /** Bound stage element, used to measure the rendered slide. */
  let stageEl = $state<HTMLElement | null>(null)
  /** Rendered slide box in stage-local pixels. */
  let slideRect = $state<SlideRect | null>(null)
  /** Active Magic Move morph mirrored from the control window. */
  let morph = $state<MorphPayload | null>(null)
  /** Bound morph overlay root, used to locate its canvas during playback. */
  let morphOverlayEl = $state<HTMLElement | null>(null)

  $effect(() => {
    let cancelled = false
    const unlisteners: Array<() => void> = []

    invoke<PresenterState>('get_presenter_state').then((state) => {
      if (!cancelled) {
        presenterState = state
        appliedFilters = state.presenterSettings.projectorFilters ?? defaultProjectorFilters()
      }
    })

    /** Registers a listener and ensures it is torn down even if the effect is. */
    function on<T>(event: string, handler: (payload: T) => void): void {
      listen<T>(event, (e: Event<T>) => handler(e.payload)).then(
        (un: UnlistenFn) => {
          if (cancelled) un()
          else unlisteners.push(un)
        },
      )
    }

    on<PresenterState>(PRESENTER_EVENTS.state, (state) => {
      presenterState = state
      appliedFilters = state.presenterSettings.projectorFilters ?? defaultProjectorFilters()
    })
    on<{ step: number }>(PRESENTER_EVENTS.buildStep, (payload) => {
      activeBuildStep = payload.step
    })
    on<{ step: number }>(PRESENTER_EVENTS.codeStep, (payload) => {
      activeCodeStep = payload.step
    })
    on<LaserPayload>(PRESENTER_EVENTS.laser, (payload) => {
      laser = payload.visible ? payload : null
    })
    on<{ strokes: HighlighterStroke[] }>(PRESENTER_EVENTS.highlighter, (payload) => {
      strokes = payload.strokes
    })
    on<{ mode: BlankMode }>(PRESENTER_EVENTS.blank, (payload) => {
      blankMode = payload.mode
    })
    on<ProjectorFiltersDto>(PRESENTER_EVENTS.filters, (payload) => {
      appliedFilters = payload
    })
    on<MorphPayload>(PRESENTER_EVENTS.morph, (payload) => {
      morph = payload
    })
    on<unknown>(PRESENTER_EVENTS.exit, () => {
      window.close()
    })

    return () => {
      cancelled = true
      for (const un of unlisteners) un()
    }
  })

  // Keep the slide box measurement current as the window resizes.
  $effect(() => {
    const stage = stageEl
    if (!stage) return
    const update = (): void => {
      slideRect = slideRectFromStage(stage)
    }
    update()
    const observer = new ResizeObserver(update)
    observer.observe(stage)
    return () => observer.disconnect()
  })

  // Re-measure after a slide change, since re-rendering resets the layout.
  $effect(() => {
    void presenterState?.currentSlide.id
    if (stageEl) slideRect = slideRectFromStage(stageEl)
  })

  // Mirror the control window's Magic Move morph once both canvases render.
  $effect(() => {
    const active = morph
    const stage = stageEl
    const overlayRoot = morphOverlayEl
    if (!active || !stage || !overlayRoot) return
    const base = stage.querySelector<HTMLElement>('.stage-content .canvas')
    const overlay = overlayRoot.querySelector<HTMLElement>('.canvas')
    if (!base || !overlay) return
    let cancelled = false
    const handle = window.requestAnimationFrame(() => {
      if (cancelled) return
      void runMorph(base, overlay, active.frames, active.durationMs).then(() => {
        if (!cancelled) morph = null
      })
    })
    return () => {
      cancelled = true
      cancelAnimationFrame(handle)
    }
  })

  /** White slide background. */
  function backgroundColor(): ColorDto {
    return { r: 255, g: 255, b: 255, a: 255 }
  }

  /** CSS class for the current slide's transition kind. */
  function transitionClass(): string {
    const kind = presenterState?.currentSlide.transition?.kind ?? 'none'
    return `transition-${kind}`
  }

  /** Duration of the current slide's transition in milliseconds. */
  function transitionDurationMs(): number {
    return presenterState?.currentSlide.transition?.durationMs ?? 0
  }

  /** Converts a stroke to an SVG `points` string in the 0..100 viewBox. */
  function strokePoints(stroke: HighlighterStroke): string {
    return stroke.points.map((p) => `${(p.x * 100).toFixed(2)},${(p.y * 100).toFixed(2)}`).join(' ')
  }
</script>

<div class="audience" role="application" aria-label="Audience view">
  {#if presenterState}
    <div class="stage" bind:this={stageEl}>
      {#key presenterState.currentSlide.id}
        <div
          class="stage-content {transitionClass()}"
          style:--transition-duration="{transitionDurationMs()}ms"
          style:filter={filterCss || 'none'}
        >
          <SlideCanvas
            slide={presenterState.currentSlide}
            background={backgroundColor()}
            media={presenterState.media}
            slideSize={presenterState.slideSize}
            highContrast={presenterState.highContrast}
            readonly
            {activeBuildStep}
            codeActiveStep={activeCodeStep}
          />
        </div>
      {/key}

      {#if morph}
        <div class="morph-overlay" bind:this={morphOverlayEl} aria-hidden="true">
          <SlideCanvas
            slide={morph.prev}
            background={{ r: 255, g: 255, b: 255, a: 0 }}
            media={presenterState.media}
            slideSize={presenterState.slideSize}
            highContrast={presenterState.highContrast}
            readonly
          />
        </div>
      {/if}

      {#if slideRect && strokes.length > 0}
        <svg
          class="overlay highlighter-overlay"
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          style:left="{slideRect.x}px"
          style:top="{slideRect.y}px"
          style:width="{slideRect.w}px"
          style:height="{slideRect.h}px"
          aria-hidden="true"
        >
          {#each strokes as stroke}
            <polyline
              points={strokePoints(stroke)}
              fill="none"
              stroke={stroke.color}
              stroke-width="8"
              stroke-linecap="round"
              stroke-linejoin="round"
              vector-effect="non-scaling-stroke"
              opacity="0.55"
            />
          {/each}
        </svg>
      {/if}

      {#if slideRect && laser}
        <div
          class="overlay laser-dot"
          style:left="{slideRect.x + laser.x * slideRect.w}px"
          style:top="{slideRect.y + laser.y * slideRect.h}px"
          style:background={laser.color}
          aria-hidden="true"
        ></div>
      {/if}
    </div>

    {#if blankMode !== 'none'}
      <div class="blank" class:black={blankMode === 'black'} class:white={blankMode === 'white'}></div>
    {/if}
  {/if}
</div>

<style>
  :global(body) {
    margin: 0;
    background: #000;
  }
  .audience {
    position: relative;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }
  .stage {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .stage-content {
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .stage-content.transition-fade {
    animation: transition-fade var(--transition-duration, 500ms) ease forwards;
  }
  .stage-content.transition-slide {
    animation: transition-slide var(--transition-duration, 500ms) ease forwards;
  }
  .stage-content.transition-push {
    animation: transition-push var(--transition-duration, 500ms) ease forwards;
  }
  .stage-content.transition-wipe {
    animation: transition-wipe var(--transition-duration, 500ms) ease forwards;
  }
  .morph-overlay {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }
  .morph-overlay :global(.canvas) {
    box-shadow: none;
  }
  @keyframes transition-fade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes transition-slide {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }
  @keyframes transition-push {
    from {
      transform: translateX(50%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }
  @keyframes transition-wipe {
    from {
      clip-path: inset(0 100% 0 0);
    }
    to {
      clip-path: inset(0 0 0 0);
    }
  }
  .overlay {
    position: absolute;
    pointer-events: none;
    z-index: 10;
  }
  .highlighter-overlay {
    overflow: visible;
  }
  .laser-dot {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    box-shadow: 0 0 12px 4px rgba(0, 0, 0, 0.35);
    mix-blend-mode: screen;
  }
  .blank {
    position: fixed;
    inset: 0;
    z-index: 100;
  }
  .blank.black {
    background: #000;
  }
  .blank.white {
    background: #fff;
  }
</style>
