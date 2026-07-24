<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'
  import SlideThumbnail from './SlideThumbnail.svelte'
  import SlideCanvas from './SlideCanvas.svelte'
  import Presenter from './Presenter.svelte'
  import RecoveryPrompt from './RecoveryPrompt.svelte'
  import type {
    DeckSnapshot,
    RecoverySnapshot,
    RunDto,
    SlideSnapshot,
    WarningDto,
  } from './lib/types'

  /** True if this window is the presenter view. */
  const isPresenter = window.location.hash === '#/presenter'

  let deck = $state<DeckSnapshot | null>(null)
  let activeIndex = $state(0)
  let warnings = $state<WarningDto[]>([])
  let showWarnings = $state(true)
  let recoverySnapshots = $state<RecoverySnapshot[]>([])
  let showRecovery = $state(false)

  const activeSlide = $derived<SlideSnapshot | null>(deck?.slides[activeIndex] ?? null)
  const notes = $derived(activeSlide?.notes ?? '')

  $effect(() => {
    if (!isPresenter) {
      loadInitial()
    }
  })

  /** On startup, list recovery snapshots or create a new blank deck. */
  async function loadInitial(): Promise<void> {
    try {
      const snapshots = await invoke<RecoverySnapshot[]>('list_recovery_snapshots')
      if (snapshots.length > 0) {
        recoverySnapshots = snapshots
        showRecovery = true
        return
      }
      await newDeck()
    } catch (err) {
      console.error('Failed to load initial state:', err)
    }
  }

  /** Creates a new blank deck from the Rust model. */
  async function newDeck(): Promise<void> {
    deck = await invoke<DeckSnapshot>('new_deck')
    activeIndex = 0
    warnings = deck?.warnings ?? []
    showWarnings = true
    showRecovery = false
  }

  /** Opens an existing .pptx file via the system dialog. */
  async function onOpen(): Promise<void> {
    const path = await open({
      multiple: false,
      filters: [{ name: 'Presentation', extensions: ['pptx'] }],
    })
    if (typeof path !== 'string') return
    deck = await invoke<DeckSnapshot>('open_deck', { path })
    activeIndex = 0
    warnings = deck?.warnings ?? []
    showWarnings = true
  }

  /** Saves the current deck to a .pptx file via the system dialog. */
  async function onSave(): Promise<void> {
    const path = await save({
      filters: [{ name: 'Presentation', extensions: ['pptx'] }],
    })
    if (typeof path !== 'string') return
    await invoke('save_deck', { path })
  }

  /** Undoes the last edit and refreshes from the returned snapshot. */
  async function onUndo(): Promise<void> {
    deck = await invoke<DeckSnapshot>('undo')
    activeIndex = Math.min(activeIndex, (deck?.slides.length ?? 1) - 1)
  }

  /** Opens the presenter window. */
  async function onStartPresenter(): Promise<void> {
    await invoke('start_presenter')
  }

  /** Sends a text edit command to Rust and re-renders from the snapshot. */
  async function handleTextEdit(detail: {
    slideId: string
    shapeIndex: number
    paragraphIndex: number
    runs: RunDto[]
  }): Promise<void> {
    deck = await invoke<DeckSnapshot>('edit_text', {
      slide_id: detail.slideId,
      shape_index: detail.shapeIndex,
      paragraph_index: detail.paragraphIndex,
      runs: detail.runs,
    })
  }

  /** Selects a different slide in the thumbnail panel. */
  function selectSlide(index: number): void {
    activeIndex = index
  }

  /** Restores a recovery snapshot as the current deck. */
  async function handleRestore(id: string): Promise<void> {
    deck = await invoke<DeckSnapshot>('restore_recovery', { id })
    activeIndex = 0
    warnings = deck?.warnings ?? []
    showWarnings = true
    showRecovery = false
  }

  /** Discards a recovery snapshot and falls back to a new deck if none remain. */
  async function handleDiscard(id: string): Promise<void> {
    await invoke('discard_recovery', { id })
    recoverySnapshots = recoverySnapshots.filter((s) => s.id !== id)
    if (recoverySnapshots.length === 0) {
      showRecovery = false
      await newDeck()
    }
  }

  /** Skips recovery and creates a new deck, keeping the snapshots on disk. */
  async function handleSkip(): Promise<void> {
    showRecovery = false
    await newDeck()
  }
</script>

{#if isPresenter}
  <Presenter />
{:else}
  <div class="app">
    <header class="toolbar">
      <button onclick={newDeck} type="button">New</button>
      <button onclick={onOpen} type="button">Open</button>
      <button onclick={onSave} type="button">Save</button>
      <button onclick={onUndo} type="button">Undo</button>
      <button onclick={onStartPresenter} type="button">Present</button>
    </header>

    {#if showWarnings && warnings.length > 0}
      <div class="banner" role="alert">
        <strong>Warnings:</strong>
        <ul>
          {#each warnings as warning}
            <li>{warning.slideId}: {warning.message}</li>
          {/each}
        </ul>
        <button onclick={() => (showWarnings = false)} type="button">Dismiss</button>
      </div>
    {/if}

    <div class="workspace">
      <aside class="sidebar" aria-label="Slide thumbnails">
        {#if deck}
          {#each deck.slides as slide, index}
            <SlideThumbnail
              {slide}
              selected={index === activeIndex}
              onClick={() => selectSlide(index)}
            />
          {/each}
        {/if}
      </aside>

      <main class="canvas-area" aria-label="Editor canvas">
        {#if activeSlide && deck}
          <SlideCanvas
            slide={activeSlide}
            background={deck.theme.background}
            onEdit={handleTextEdit}
          />
        {:else}
          <div class="empty-canvas">Open or create a deck to start editing.</div>
        {/if}
      </main>

      <aside class="notes" aria-label="Speaker notes">
        <h2>Notes</h2>
        {#if notes}
          <p>{notes}</p>
        {:else}
          <p class="placeholder">No notes for this slide.</p>
        {/if}
      </aside>
    </div>

    {#if showRecovery}
      <RecoveryPrompt
        snapshots={recoverySnapshots}
        onRestore={handleRestore}
        onDiscard={handleDiscard}
        onSkip={handleSkip}
      />
    {/if}
  </div>
{/if}

<style>
  :global(body) {
    margin: 0;
    font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  }
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .toolbar {
    display: flex;
    gap: 0.5rem;
    padding: 0.5rem;
    border-bottom: 1px solid #ccc;
    background: #f4f4f4;
  }
  .toolbar button {
    padding: 0.4rem 0.8rem;
  }
  .banner {
    padding: 0.5rem 1rem;
    background: #fff3cd;
    border-bottom: 1px solid #e0c36e;
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .banner ul {
    margin: 0;
    padding-left: 1rem;
    flex: 1;
  }
  .workspace {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
  .sidebar {
    width: 180px;
    overflow-y: auto;
    border-right: 1px solid #ccc;
    background: #fafafa;
    padding: 0.5rem;
  }
  .canvas-area {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #e0e0e0;
    overflow: auto;
  }
  .empty-canvas {
    color: #666;
  }
  .notes {
    width: 220px;
    border-left: 1px solid #ccc;
    padding: 0.5rem;
    background: #fafafa;
    overflow-y: auto;
  }
  .notes h2 {
    font-size: 1rem;
    margin-top: 0;
  }
  .placeholder {
    color: #888;
  }
</style>
