<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { emit } from '@tauri-apps/api/event'
  import SlideCanvas from './SlideCanvas.svelte'
  import type {
    ColorDto,
    MorphFrameDto,
    PresenterSettingsDto,
    PresenterState,
    ProjectorFiltersDto,
  } from './lib/types'
  import {
    PRESENTER_EVENTS,
    clamp01,
    debounce,
    defaultProjectorFilters,
    projectorFilterCss,
    runMorph,
    slideRectFromStage,
    throttle,
    type BlankMode,
    type HighlighterStroke,
    type LaserPayload,
    type MorphPayload,
    type SlideRect,
    type Vec2,
  } from './lib/presenter'
  import { codeStepCount } from './lib/codeSteps'

  /** Presenter view state from Rust. */
  let presenterState = $state<PresenterState | null>(null)
  /** Elapsed seconds since the presenter opened. */
  let elapsed = $state(0)
  /** Timer interval handle. */
  let timer = $state<ReturnType<typeof setInterval> | null>(null)
  /** Current build step index within the current slide; -1 = before first step. */
  let activeBuildStep = $state<number>(Infinity)
  /** Current code-step index (0-based) for stepped code highlighting. */
  let activeCodeStep = $state<number>(0)

  /** Bound main slide stage element, used for coordinate mapping and overlays. */
  let stageEl = $state<HTMLElement | null>(null)
  /** Rendered main slide box in stage-local pixels. */
  let slideRect = $state<SlideRect | null>(null)

  /** Whether the laser pointer tool is active. */
  let laserOn = $state(false)
  /** Whether the highlighter tool is active. */
  let highlighterOn = $state(false)
  /** Current audience blank mode. */
  let blankMode = $state<BlankMode>('none')
  /** Current laser position (normalized), or null when hidden. */
  let laserPos = $state<Vec2 | null>(null)
  /** Transient highlighter strokes (source of truth). */
  let strokes = $state<HighlighterStroke[]>([])
  /** Whether a highlighter stroke is currently being drawn. */
  let drawing = $state(false)

  /** Whether the projector filter popover is open. */
  let filterPanelOpen = $state(false)
  /** Bound filter popover root, used to ignore clicks inside it. */
  let filterPanelEl = $state<HTMLElement | null>(null)
  /** Debounced persistence of projector filters (coalesces slider drags). */
  let persistFilters: ((settings: PresenterSettingsDto) => void) | null = null

  /** Active Magic Move morph: the previous slide rendered as an overlay while
   *  matching shapes interpolate between slides. */
  let morph = $state<MorphPayload | null>(null)
  /** Bound morph overlay root, used to locate its canvas during playback. */
  let morphOverlayEl = $state<HTMLElement | null>(null)

  /** Lazily-created throttled event emitters (capped to ~30fps). */
  let emitLaser: ((payload: LaserPayload) => void) | null = null
  let emitHighlighter: ((s: HighlighterStroke[]) => void) | null = null

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

  // Create the throttled emitters once. They broadcast to all windows; the
  // audience window listens while the presenter owns the source of truth.
  $effect(() => {
    emitLaser = throttle((payload: LaserPayload) => {
      void emit(PRESENTER_EVENTS.laser, payload)
    }, 33)
    emitHighlighter = throttle((s: HighlighterStroke[]) => {
      void emit(PRESENTER_EVENTS.highlighter, { strokes: s })
    }, 33)
    // Projector filter writes are debounced so a slider drag produces a single
    // SetPresenterSettings call rather than one per input tick.
    persistFilters = debounce((settings: PresenterSettingsDto) => {
      void invoke('set_presenter_settings', { settings })
    }, 300)
  })

  // Keep the main slide box measurement current as the window resizes.
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

  // When a morph is active and both canvases are rendered, drive the
  // interpolation once, then clear the overlay.
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

  /** Refreshes presenter state from the backend. */
  async function refresh(): Promise<void> {
    presenterState = await invoke<PresenterState>('get_presenter_state')
    if (presenterState) {
      activeBuildStep = Infinity
      activeCodeStep = 0
      laserOn = presenterState.presenterSettings.laserPointer
      highlighterOn = presenterState.presenterSettings.highlighter
    }
  }

  /** Current laser color from presenter settings. */
  function laserColor(): string {
    return presenterState?.presenterSettings.laserColor ?? '#ff0000'
  }

  /** Current highlighter color from presenter settings. */
  function highlighterColor(): string {
    return presenterState?.presenterSettings.highlighterColor ?? '#ffff00'
  }

  /** Broadcasts the full presenter state, build step, and code step. */
  function broadcastState(): void {
    if (!presenterState) return
    void emit(PRESENTER_EVENTS.state, presenterState)
    void emit(PRESENTER_EVENTS.buildStep, { step: activeBuildStep })
    void emit(PRESENTER_EVENTS.codeStep, { step: activeCodeStep })
  }

  /** Broadcasts only the build-step index. */
  function broadcastBuildStep(): void {
    void emit(PRESENTER_EVENTS.buildStep, { step: activeBuildStep })
  }

  /** Broadcasts only the active code-step index. */
  function broadcastCodeStep(): void {
    void emit(PRESENTER_EVENTS.codeStep, { step: activeCodeStep })
  }

  /** Number of stepped code steps on the current slide (0 if none). */
  function currentCodeStepCount(): number {
    return codeStepCount(presenterState?.currentSlide)
  }

  /** Clears highlighter strokes locally and on the audience window. */
  function clearStrokes(): void {
    strokes = []
    emitHighlighter?.([])
  }

  /** Advances the build timeline or moves to the next slide. */
  async function next(): Promise<void> {
    if (!presenterState) return
    // Stepped code highlighting advances first (like build steps, but for
    // code). Each click moves to the next code step before builds or the next
    // slide.
    const codeCount = currentCodeStepCount()
    if (codeCount > 0 && activeCodeStep < codeCount - 1) {
      activeCodeStep += 1
      broadcastCodeStep()
      return
    }
    const steps = presenterState.currentSlide.animation?.steps ?? []
    const maxStep = steps.length - 1
    if (activeBuildStep < maxStep) {
      activeBuildStep += 1
      broadcastBuildStep()
      return
    }
    // A Magic Move transition belongs to the slide being entered. Compute the
    // interpolation frames before advancing so the morph plays as soon as the
    // incoming slide renders (no flash of the settled next slide).
    const upcoming = presenterState.nextSlide
    const isMorph = upcoming?.transition?.kind === 'morph'
    const prevSlide = presenterState.currentSlide
    let frames: MorphFrameDto[] = []
    if (isMorph && upcoming) {
      try {
        frames = await invoke<MorphFrameDto[]>('compute_morph', {
          prevSlideId: prevSlide.id,
          nextSlideId: upcoming.id,
        })
      } catch {
        frames = []
      }
    }
    const result = await invoke<PresenterState>('presenter_next')
    if (result.slideNumber === presenterState.slideNumber) return
    presenterState = result
    clearStrokes()
    if (isMorph && frames.length > 0 && result.currentSlide) {
      // Show the incoming slide fully built so every shape can interpolate.
      activeBuildStep = Infinity
      activeCodeStep = 0
      broadcastState()
      const payload: MorphPayload = {
        prev: prevSlide,
        frames,
        durationMs: result.currentSlide.transition?.durationMs ?? 500,
      }
      morph = payload
      // Mirror the morph to the audience window, which renders the same overlay.
      void emit(PRESENTER_EVENTS.morph, payload)
    } else {
      activeBuildStep = -1
      activeCodeStep = 0
      broadcastState()
    }
  }

  /** Goes to the previous slide, showing it fully built. */
  async function previous(): Promise<void> {
    if (!presenterState) return
    const result = await invoke<PresenterState>('presenter_previous')
    if (result.slideNumber !== presenterState.slideNumber) {
      presenterState = result
      activeBuildStep = Infinity
      activeCodeStep = 0
      clearStrokes()
      broadcastState()
    }
  }

  /** Jumps to the first slide. */
  async function first(): Promise<void> {
    while (presenterState && presenterState.slideNumber > 1) {
      const result = await invoke<PresenterState>('presenter_previous')
      if (result.slideNumber === presenterState.slideNumber) break
      presenterState = result
    }
    if (presenterState) {
      activeBuildStep = Infinity
      activeCodeStep = 0
      clearStrokes()
      broadcastState()
    }
  }

  /** Jumps to the last slide. */
  async function last(): Promise<void> {
    while (presenterState && presenterState.slideNumber < presenterState.total) {
      const result = await invoke<PresenterState>('presenter_next')
      if (result.slideNumber === presenterState.slideNumber) break
      presenterState = result
    }
    if (presenterState) {
      activeBuildStep = Infinity
      activeCodeStep = 0
      clearStrokes()
      broadcastState()
    }
  }

  /** Closes both presenter windows. */
  function close(): void {
    void emit(PRESENTER_EVENTS.exit)
    window.close()
  }

  /** Toggles the laser pointer and persists the new default to the deck. */
  function toggleLaser(): void {
    laserOn = !laserOn
    if (!laserOn) {
      laserPos = null
      emitLaser?.({ x: 0, y: 0, visible: false, color: laserColor() })
    }
    persistSettings({ ...settings(), laserPointer: laserOn })
  }

  /** Toggles the highlighter and persists the new default to the deck. */
  function toggleHighlighter(): void {
    highlighterOn = !highlighterOn
    if (!highlighterOn) drawing = false
    persistSettings({ ...settings(), highlighter: highlighterOn })
  }

  /** Toggles the audience blank screen for the given color. */
  function toggleBlank(mode: BlankMode): void {
    blankMode = blankMode === mode ? 'none' : mode
    void emit(PRESENTER_EVENTS.blank, { mode: blankMode })
  }

  /** Returns the current presenter settings DTO. */
  function settings(): PresenterSettingsDto {
    return (
      presenterState?.presenterSettings ?? {
        laserPointer: false,
        laserColor: '#ff0000',
        highlighter: false,
        highlighterColor: '#ffff00',
        projectorFilters: defaultProjectorFilters(),
      }
    )
  }

  /** Returns the current projector filters (neutral when no deck is open). */
  function currentFilters(): ProjectorFiltersDto {
    return presenterState?.presenterSettings.projectorFilters ?? defaultProjectorFilters()
  }

  /**
   * Applies a new projector filter set: updates the local source of truth,
   * broadcasts it to the audience window for an immediate CSS update, and
   * schedules a debounced persistence write. The presenter's own view is never
   * filtered — only the audience (projector) window is.
   */
  function updateFilters(next: ProjectorFiltersDto): void {
    if (!presenterState) return
    const updated: PresenterSettingsDto = {
      ...presenterState.presenterSettings,
      projectorFilters: next,
    }
    presenterState.presenterSettings = updated
    void emit(PRESENTER_EVENTS.filters, next)
    persistFilters?.(updated)
  }

  /** Persists presenter settings to the deck (colors and tool defaults). */
  async function persistSettings(next: PresenterSettingsDto): Promise<void> {
    if (presenterState) presenterState.presenterSettings = next
    try {
      await invoke('set_presenter_settings', { settings: next })
    } catch {
      // Presenter settings are a convenience; ignore persistence errors.
    }
  }

  /** Converts a pointer event to normalized slide coordinates. */
  function pointerNormalized(event: PointerEvent): Vec2 | null {
    const stage = stageEl
    if (!stage) return null
    const canvas = stage.querySelector<HTMLElement>('.canvas')
    if (!canvas) return null
    const rect = canvas.getBoundingClientRect()
    if (rect.width === 0 || rect.height === 0) return null
    return {
      x: clamp01((event.clientX - rect.left) / rect.width),
      y: clamp01((event.clientY - rect.top) / rect.height),
    }
  }

  /** Begins a highlighter stroke. */
  function onStagePointerDown(event: PointerEvent): void {
    if (!highlighterOn) return
    event.preventDefault()
    event.stopPropagation()
    const point = pointerNormalized(event)
    if (!point) return
    drawing = true
    strokes = [...strokes, { points: [point], color: highlighterColor() }]
    emitHighlighter?.(strokes)
  }

  /** Tracks the cursor for the laser or extends the active highlighter stroke. */
  function onStagePointerMove(event: PointerEvent): void {
    if (highlighterOn && drawing) {
      const point = pointerNormalized(event)
      if (!point || strokes.length === 0) return
      strokes[strokes.length - 1].points.push(point)
      strokes = [...strokes]
      emitHighlighter?.(strokes)
      return
    }
    if (!laserOn) return
    const point = pointerNormalized(event)
    if (!point) return
    laserPos = point
    emitLaser?.({ x: point.x, y: point.y, visible: true, color: laserColor() })
  }

  /** Hides the laser when the cursor leaves the slide. */
  function onStagePointerLeave(): void {
    if (laserOn) {
      laserPos = null
      emitLaser?.({ x: 0, y: 0, visible: false, color: laserColor() })
    }
  }

  /** Ends the active highlighter stroke. */
  function onWindowPointerUp(): void {
    if (drawing) drawing = false
  }

  /** Closes the filter popover on any outside click, then advances when not drawing. */
  function onWindowClick(event: MouseEvent): void {
    if (filterPanelEl && filterPanelEl.contains(event.target as Node)) return
    filterPanelOpen = false
    if (highlighterOn) return
    next()
  }

  /** Keyboard control for presenter navigation and tools. */
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
    } else if (event.key === 'b' || event.key === 'B') {
      event.preventDefault()
      toggleBlank('black')
    } else if (event.key === 'w' || event.key === 'W') {
      event.preventDefault()
      toggleBlank('white')
    } else if (event.key === 'l' || event.key === 'L') {
      event.preventDefault()
      toggleLaser()
    } else if (event.key === 'h' || event.key === 'H') {
      event.preventDefault()
      toggleHighlighter()
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
    return { r: 255, g: 255, b: 255, a: 255 }
  }

  /** CSS class for the transition kind of the current slide. */
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

<svelte:window onclick={onWindowClick} onpointerup={onWindowPointerUp} />

<div class="presenter" role="application" aria-label="Presenter view" tabindex="-1">
  {#if presenterState}
    <div
      class="stage"
      class:tool-active={laserOn || highlighterOn}
      bind:this={stageEl}
      role="group"
      aria-label="Slide stage"
      onpointerdown={onStagePointerDown}
      onpointermove={onStagePointerMove}
      onpointerleave={onStagePointerLeave}
    >
      {#key presenterState.currentSlide.id}
        <div
          class="stage-content {transitionClass()}"
          style:--transition-duration="{transitionDurationMs()}ms"
        >
          <SlideCanvas
            slide={presenterState.currentSlide}
            background={backgroundColor()}
            media={presenterState.media}
            slideSize={presenterState.slideSize}
            highContrast={presenterState.highContrast}
            readonly
            activeBuildStep={activeBuildStep}
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

      {#if slideRect && laserOn && laserPos}
        <div
          class="overlay laser-dot"
          style:left="{slideRect.x + laserPos.x * slideRect.w}px"
          style:top="{slideRect.y + laserPos.y * slideRect.h}px"
          style:background={laserColor()}
          aria-hidden="true"
        ></div>
      {/if}
    </div>

    <div class="hud">
      <div class="tools">
        <button class="tool" class:on={laserOn} onclick={toggleLaser} type="button" title="Laser pointer (L)">
          Laser
        </button>
        <label class="color" title="Laser color">
          <input
            type="color"
            value={laserColor()}
            onchange={(e) => persistSettings({ ...settings(), laserColor: e.currentTarget.value })}
          />
        </label>
        <button
          class="tool"
          class:on={highlighterOn}
          onclick={toggleHighlighter}
          type="button"
          title="Highlighter (H)"
        >
          Highlighter
        </button>
        <label class="color" title="Highlighter color">
          <input
            type="color"
            value={highlighterColor()}
            onchange={(e) =>
              persistSettings({ ...settings(), highlighterColor: e.currentTarget.value })}
          />
        </label>
        <span class="filter-tool">
          <button
            class="tool"
            class:on={filterPanelOpen}
            onclick={(e) => {
              e.stopPropagation()
              filterPanelOpen = !filterPanelOpen
            }}
            type="button"
            title="Projector filters"
          >
            Filters
          </button>
          {#if filterPanelOpen}
            <div
              class="filter-panel"
              bind:this={filterPanelEl}
            >
              <label class="filter-row check">
                <input
                  type="checkbox"
                  checked={currentFilters().invert}
                  onchange={(e) =>
                    updateFilters({ ...currentFilters(), invert: e.currentTarget.checked })}
                />
                Invert
              </label>
              <label class="filter-row">
                <span class="filter-label">Brightness <em>{currentFilters().brightness.toFixed(2)}</em></span>
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.05"
                  value={currentFilters().brightness}
                  oninput={(e) =>
                    updateFilters({ ...currentFilters(), brightness: parseFloat(e.currentTarget.value) })}
                />
              </label>
              <label class="filter-row">
                <span class="filter-label">Contrast <em>{currentFilters().contrast.toFixed(2)}</em></span>
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.05"
                  value={currentFilters().contrast}
                  oninput={(e) =>
                    updateFilters({ ...currentFilters(), contrast: parseFloat(e.currentTarget.value) })}
                />
              </label>
              <label class="filter-row">
                <span class="filter-label">Saturation <em>{currentFilters().saturation.toFixed(2)}</em></span>
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.05"
                  value={currentFilters().saturation}
                  oninput={(e) =>
                    updateFilters({ ...currentFilters(), saturation: parseFloat(e.currentTarget.value) })}
                />
              </label>
              <label class="filter-row">
                <span class="filter-label">Sepia <em>{currentFilters().sepia.toFixed(2)}</em></span>
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.05"
                  value={currentFilters().sepia}
                  oninput={(e) =>
                    updateFilters({ ...currentFilters(), sepia: parseFloat(e.currentTarget.value) })}
                />
              </label>
              <label class="filter-row">
                <span class="filter-label">Hue rotate <em>{currentFilters().hueRotate.toFixed(0)}&deg;</em></span>
                <input
                  type="range"
                  min="0"
                  max="360"
                  step="1"
                  value={currentFilters().hueRotate}
                  oninput={(e) =>
                    updateFilters({ ...currentFilters(), hueRotate: parseFloat(e.currentTarget.value) })}
                />
              </label>
              <button class="tool reset" type="button" onclick={() => updateFilters(defaultProjectorFilters())}>
                Reset
              </button>
            </div>
          {/if}
        </span>
        {#if blankMode !== 'none'}
          <span class="badge" class:black={blankMode === 'black'} class:white={blankMode === 'white'}>
            {blankMode === 'black' ? 'B' : 'W'}
          </span>
        {/if}
      </div>

      <div class="next">
        {#if presenterState.nextSlide}
          <SlideCanvas
            slide={presenterState.nextSlide}
            background={backgroundColor()}
            media={presenterState.media}
            slideSize={presenterState.slideSize}
            highContrast={presenterState.highContrast}
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

      {#if currentCodeStepCount() > 0}
        <div class="code-step-indicator" aria-label="Active code step">
          Code {activeCodeStep + 1} / {currentCodeStepCount()}
        </div>
      {/if}

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
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    user-select: none;
    touch-action: none;
  }
  .stage.tool-active {
    cursor: crosshair;
  }
  .hud {
    width: 320px;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .tools {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .tool {
    flex: 0 0 auto;
  }
  .tool.on {
    background: #1f6feb;
    border-color: #1f6feb;
  }
  .color {
    display: inline-flex;
    width: 28px;
    height: 28px;
    border: 1px solid #444;
    border-radius: 4px;
    overflow: hidden;
  }
  .color input {
    width: 200%;
    height: 200%;
    margin: -25% 0 0 -25%;
    border: none;
    padding: 0;
    cursor: pointer;
    background: none;
  }
  .badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.6rem;
    height: 1.6rem;
    border-radius: 4px;
    font-weight: 700;
    font-size: 0.9rem;
    border: 1px solid #444;
  }
  .badge.black {
    background: #000;
    color: #fff;
  }
  .badge.white {
    background: #fff;
    color: #000;
  }
  .filter-tool {
    position: relative;
    display: inline-flex;
  }
  .filter-panel {
    position: absolute;
    top: calc(100% + 0.4rem);
    left: 0;
    z-index: 50;
    width: 240px;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.75rem;
    background: #1c1c1c;
    border: 1px solid #444;
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  }
  .filter-row {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85rem;
  }
  .filter-row.check {
    flex-direction: row;
    align-items: center;
    gap: 0.4rem;
  }
  .filter-label {
    display: flex;
    justify-content: space-between;
    font-variant-numeric: tabular-nums;
  }
  .filter-label em {
    font-style: normal;
    color: #9bc1ff;
  }
  .filter-row input[type='range'] {
    width: 100%;
    cursor: pointer;
  }
  .filter-panel .reset {
    align-self: flex-end;
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
  .code-step-indicator {
    align-self: flex-start;
    padding: 0.2rem 0.5rem;
    background: #3a2f00;
    border: 1px solid #997a00;
    border-radius: 4px;
    color: #ffd95e;
    font-size: 0.85rem;
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
  .stage-content {
    width: 100%;
    height: 100%;
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
</style>
