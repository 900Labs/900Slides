<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import type {
    DeckSnapshot,
    SlideSnapshot,
    TextBoxSnapshot,
    VersionDiffDto,
    VersionInfoDto,
  } from './lib/types'

  interface Props {
    /** Closes the panel. */
    onClose: () => void
    /** Called after a restore; the parent adopts the returned deck snapshot. */
    onRestore: (deck: DeckSnapshot) => void
  }

  let { onClose, onRestore }: Props = $props()

  /** All saved versions of the current deck, newest first. */
  let versions = $state<VersionInfoDto[]>([])
  /** True while the version list is loading or refreshing. */
  let loading = $state(false)
  /** Last error message, shown until dismissed. */
  let error = $state('')
  /** Hash of the version currently selected for preview. */
  let selectedHash = $state<string | null>(null)
  /** Preview deck for the selected version, or null while loading/none. */
  let previewDeck = $state<DeckSnapshot | null>(null)
  let previewLoading = $state(false)
  /** Hash whose inline name editor is open. */
  let namingHash = $state<string | null>(null)
  /** Current value of the inline name editor. */
  let nameInput = $state('')
  /** True while a restore is in flight. */
  let restoring = $state(false)
  /** First version selected for a diff. */
  let diffA = $state<string>('')
  /** Second version selected for a diff. */
  let diffB = $state<string>('')
  let diffLoading = $state(false)
  let diffResult = $state<VersionDiffDto | null>(null)

  /** Loads the version list from the backend. */
  async function refresh(): Promise<void> {
    loading = true
    error = ''
    try {
      versions = await invoke<VersionInfoDto[]>('list_versions')
      const first = versions[0]?.hash ?? ''
      diffA = diffA && versions.some((v) => v.hash === diffA) ? diffA : first
      diffB =
        diffB && versions.some((v) => v.hash === diffB) ? diffB : (versions[1]?.hash ?? first)
      // Auto-select the newest version for preview on first load.
      if (selectedHash === null && versions.length > 0) {
        await selectVersion(first)
      } else if (versions.length === 0) {
        selectedHash = null
        previewDeck = null
      }
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  /** Selects a version and loads its read-only preview. */
  async function selectVersion(hash: string): Promise<void> {
    selectedHash = hash
    previewLoading = true
    previewDeck = null
    try {
      previewDeck = await invoke<DeckSnapshot>('get_version', { hash })
    } catch (e) {
      error = String(e)
    } finally {
      previewLoading = false
    }
  }

  /** Opens the inline name editor for a version. */
  function startName(v: VersionInfoDto): void {
    namingHash = v.hash
    nameInput = v.name ?? ''
  }

  /** Saves the edited name and refreshes the list. */
  async function commitName(): Promise<void> {
    const hash = namingHash
    const name = nameInput.trim()
    if (hash === null) return
    namingHash = null
    try {
      await invoke('name_version', { hash, name })
      await refresh()
    } catch (e) {
      error = String(e)
    }
  }

  /** Restores the selected version into the session (undoable). */
  async function doRestore(hash: string): Promise<void> {
    restoring = true
    error = ''
    try {
      const snapshot = await invoke<DeckSnapshot>('restore_version', { hash })
      onRestore(snapshot)
      await refresh()
    } catch (e) {
      error = String(e)
    } finally {
      restoring = false
    }
  }

  /** Computes the structural diff between the two selected versions. */
  async function runDiff(): Promise<void> {
    if (!diffA || !diffB) return
    diffLoading = true
    error = ''
    diffResult = null
    try {
      diffResult = await invoke<VersionDiffDto>('diff_versions', {
        hashA: diffA,
        hashB: diffB,
      })
    } catch (e) {
      error = String(e)
    } finally {
      diffLoading = false
    }
  }

  /** Builds a short text preview of a slide (fallback rendering). */
  function previewText(slide: SlideSnapshot): string {
    return slide.shapes
      .map((shape) => {
        if (shape.kind === 'text_box') {
          const textBox = shape.value as TextBoxSnapshot
          return textBox.paragraphs
            .map((paragraph) => paragraph.runs.map((run) => run.text).join(''))
            .join(' ')
        }
        return ''
      })
      .join(' ')
      .trim()
  }

  /** Formats an ISO timestamp as a localized date-time string. */
  function formatTime(timestamp: string): string {
    const date = new Date(timestamp)
    return Number.isNaN(date.getTime()) ? timestamp : date.toLocaleString()
  }

  function onKey(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
    }
  }

  // Load versions when the panel opens.
  $effect(() => {
    void refresh()
  })
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation">
  <button
    type="button"
    class="backdrop-close"
    aria-label="Close version history"
    onclick={onClose}
  ></button>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Version history">
    <div class="dialog-header">
      <h2>Version history</h2>
      <div class="header-actions">
        <button type="button" class="ghost" onclick={() => void refresh()} disabled={loading}>
          Refresh
        </button>
        <button type="button" class="close-btn" onclick={onClose} aria-label="Close">✕</button>
      </div>
    </div>

    {#if error}
      <div class="banner" role="alert">
        {error}
        <button type="button" onclick={() => (error = '')}>Dismiss</button>
      </div>
    {/if}

    <div class="body">
      <section class="list-col">
        {#if loading && versions.length === 0}
          <p class="empty">Loading versions…</p>
        {:else if versions.length === 0}
          <p class="empty">
            No saved versions yet. Save the deck to create a content-addressed snapshot.
          </p>
        {:else}
          <ul class="version-list">
            {#each versions as v (v.hash)}
              <li class="version-row" class:selected={selectedHash === v.hash}>
                {#if namingHash === v.hash}
                  <div class="row-edit">
                    <input
                      type="text"
                      class="name-input"
                      bind:value={nameInput}
                      aria-label="Version name"
                    />
                    <button type="button" class="mini" onclick={commitName}>Save</button>
                    <button type="button" class="mini" onclick={() => (namingHash = null)}>
                      Cancel
                    </button>
                  </div>
                {:else}
                  <button type="button" class="row-main" onclick={() => void selectVersion(v.hash)}>
                    <span class="row-time">{formatTime(v.timestamp)}</span>
                    <span class="row-name">{v.name ?? 'Untitled'}</span>
                    <span class="row-hash">{v.hash.slice(0, 8)}</span>
                  </button>
                  <div class="row-actions">
                    <button type="button" class="mini" onclick={() => startName(v)}>Name</button>
                    <button
                      type="button"
                      class="mini"
                      onclick={() => void doRestore(v.hash)}
                      disabled={restoring}
                    >
                      Restore
                    </button>
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      <section class="detail-col">
        <h3>Preview</h3>
        {#if previewLoading}
          <p class="empty">Loading preview…</p>
        {:else if previewDeck}
          <div class="preview-card">
            <div class="preview-meta">{previewDeck.slides.length} slide{previewDeck.slides.length === 1 ? '' : 's'}</div>
            {#if previewDeck.slides.length > 0}
              <div class="preview-thumb">
                {previewText(previewDeck.slides[0]) || '(blank first slide)'}
              </div>
            {/if}
          </div>
        {:else}
          <p class="empty">Select a version to preview it.</p>
        {/if}

        <h3>Compare two versions</h3>
        <div class="diff-controls">
          <select bind:value={diffA} aria-label="First version">
            {#each versions as v (v.hash)}
              <option value={v.hash}>{formatTime(v.timestamp)} · {v.hash.slice(0, 8)}</option>
            {/each}
          </select>
          <span class="diff-arrow">→</span>
          <select bind:value={diffB} aria-label="Second version">
            {#each versions as v (v.hash)}
              <option value={v.hash}>{formatTime(v.timestamp)} · {v.hash.slice(0, 8)}</option>
            {/each}
          </select>
          <button type="button" class="action" onclick={() => void runDiff()} disabled={diffLoading || !diffA || !diffB}>
            Compare
          </button>
        </div>

        {#if diffResult}
          <div class="diff-result">
            {#if diffResult.slidesAdded.length === 0 && diffResult.slidesRemoved.length === 0 && diffResult.slidesModified.length === 0}
              <p class="empty">No structural differences between these versions.</p>
            {:else}
              {#if diffResult.slidesAdded.length > 0}
                <p class="diff-group">
                  <strong>Slides added:</strong> {diffResult.slidesAdded.join(', ')}
                </p>
              {/if}
              {#if diffResult.slidesRemoved.length > 0}
                <p class="diff-group">
                  <strong>Slides removed:</strong> {diffResult.slidesRemoved.join(', ')}
                </p>
              {/if}
              {#if diffResult.slidesModified.length > 0}
                <p class="diff-group"><strong>Modified slides:</strong></p>
                <ul class="modified-list">
                  {#each diffResult.slidesModified as mod (mod.slideId)}
                    <li>
                      <span class="mod-id">{mod.slideId}</span>
                      <span class="mod-counts">
                        +{mod.shapesAdded} shape{mod.shapesAdded === 1 ? '' : 's'}
                        · −{mod.shapesRemoved} shape{mod.shapesRemoved === 1 ? '' : 's'}
                      </span>
                      {#if mod.textChanged.length > 0}
                        <span class="mod-text">{mod.textChanged.join(' · ')}</span>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {/if}
            {/if}
          </div>
        {/if}
      </section>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .backdrop-close {
    position: fixed;
    inset: 0;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .dialog {
    position: relative;
    z-index: 1;
    width: min(760px, 94vw);
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    background: #fff;
    border-radius: 8px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #e5e5e5;
  }
  .dialog-header h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .close-btn {
    background: none;
    border: none;
    font-size: 1rem;
    cursor: pointer;
    color: #555;
  }
  .ghost {
    padding: 0.25rem 0.6rem;
    border: 1px solid #ccc;
    background: #f7f7f7;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .ghost:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .banner {
    margin: 0.5rem 1rem 0;
    padding: 0.4rem 0.6rem;
    background: #fdecea;
    border: 1px solid #e5b7b3;
    border-radius: 4px;
    font-size: 0.85rem;
    color: #8a3b2e;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }
  .banner button {
    border: 1px solid #e5b7b3;
    background: #fff;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 0.75rem;
    padding: 0.75rem 1rem 1rem;
    overflow-y: auto;
  }
  .list-col {
    min-width: 0;
  }
  .version-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    max-height: 60vh;
    overflow-y: auto;
  }
  .version-row {
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    background: #fafafa;
    overflow: hidden;
  }
  .version-row.selected {
    border-color: #0070c0;
    box-shadow: 0 0 0 2px rgba(0, 112, 192, 0.25);
  }
  .row-main {
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid #eee;
    padding: 0.4rem 0.5rem;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  .row-time {
    font-size: 0.8rem;
    color: #333;
    font-weight: 600;
  }
  .row-name {
    font-size: 0.85rem;
    color: #444;
  }
  .row-hash {
    font-size: 0.7rem;
    color: #999;
    font-family: 'Courier New', monospace;
  }
  .row-edit {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.4rem 0.5rem;
    flex-wrap: wrap;
  }
  .name-input {
    flex: 1;
    min-width: 80px;
    padding: 0.2rem 0.35rem;
    border: 1px solid #ccc;
    border-radius: 4px;
    font-size: 0.85rem;
  }
  .row-actions {
    display: flex;
    gap: 0.25rem;
    padding: 0.25rem 0.5rem;
  }
  .mini {
    padding: 0.2rem 0.5rem;
    font-size: 0.8rem;
    border: 1px solid #ccc;
    background: #fff;
    border-radius: 4px;
    cursor: pointer;
  }
  .mini:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .detail-col {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .detail-col h3 {
    margin: 0.5rem 0 0.15rem;
    font-size: 0.8rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .preview-card {
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    background: #fafafa;
    padding: 0.5rem;
  }
  .preview-meta {
    font-size: 0.8rem;
    color: #555;
    margin-bottom: 0.35rem;
  }
  .preview-thumb {
    aspect-ratio: 16 / 9;
    border: 1px solid #ddd;
    background: #fff;
    padding: 0.35rem;
    font-size: 0.65rem;
    line-height: 1.2;
    color: #333;
    overflow: hidden;
    word-break: break-word;
  }
  .empty {
    font-size: 0.85rem;
    color: #777;
    margin: 0.25rem 0;
  }
  .diff-controls {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-wrap: wrap;
  }
  .diff-controls select {
    flex: 1;
    min-width: 120px;
    padding: 0.25rem 0.35rem;
    border: 1px solid #ccc;
    border-radius: 4px;
    font-size: 0.8rem;
  }
  .diff-arrow {
    color: #999;
  }
  .action {
    padding: 0.3rem 0.7rem;
    border: 1px solid #0070c0;
    background: #0070c0;
    color: #fff;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85rem;
    white-space: nowrap;
  }
  .action:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .diff-result {
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    background: #fafafa;
    padding: 0.5rem;
    font-size: 0.85rem;
    color: #333;
  }
  .diff-group {
    margin: 0 0 0.35rem;
  }
  .modified-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .modified-list li {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding-bottom: 0.3rem;
    border-bottom: 1px solid #eee;
  }
  .modified-list li:last-child {
    border-bottom: none;
  }
  .mod-id {
    font-family: 'Courier New', monospace;
    font-size: 0.8rem;
    color: #0070c0;
  }
  .mod-counts {
    font-size: 0.8rem;
    color: #555;
  }
  .mod-text {
    font-size: 0.78rem;
    color: #777;
    word-break: break-word;
  }
</style>
