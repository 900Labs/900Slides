<script lang="ts">
  /** Modal listing the application's keyboard shortcuts. Static content. */
  interface Props {
    /** Closes the dialog. */
    onClose: () => void
  }

  let { onClose }: Props = $props()

  /** Shortcut groups shown in the dialog. */
  const groups: { title: string; shortcuts: { keys: string; label: string }[] }[] = [
    {
      title: 'Global',
      shortcuts: [
        { keys: 'Ctrl/Cmd+F', label: 'Find' },
        { keys: 'Ctrl/Cmd+H', label: 'Find and replace' },
        { keys: '?', label: 'Open this shortcuts dialog' },
        { keys: 'Esc', label: 'Close dialog' },
      ],
    },
    {
      title: 'Deck',
      shortcuts: [
        { keys: 'Ctrl/Cmd+Z', label: 'Undo' },
        { keys: 'Ctrl/Cmd+S', label: 'Save (use the Save button)' },
      ],
    },
    {
      title: 'Editor',
      shortcuts: [
        { keys: 'Click', label: 'Select a slide in the thumbnail sidebar' },
        { keys: 'B / I / U / S', label: 'Bold / Italic / Underline / Strikethrough (text toolbar)' },
        { keys: 'Double-click chart', label: 'Open the chart data editor' },
      ],
    },
    {
      title: 'Presenter',
      shortcuts: [
        { keys: '→ / ↓ / Space / PgDn', label: 'Next slide or build step' },
        { keys: '← / ↑ / PgUp', label: 'Previous slide' },
        { keys: 'Home', label: 'First slide' },
        { keys: 'End', label: 'Last slide' },
        { keys: 'Esc', label: 'Exit presenter' },
      ],
    },
  ]

  function onKey(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation">
  <button
    type="button"
    class="backdrop-close"
    aria-label="Close shortcuts dialog"
    onclick={onClose}
  ></button>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
    <div class="dialog-header">
      <h2>Keyboard shortcuts</h2>
      <button type="button" class="close-btn" onclick={onClose} aria-label="Close">✕</button>
    </div>
    <div class="dialog-body">
      {#each groups as group}
        <section>
          <h3>{group.title}</h3>
          <dl>
            {#each group.shortcuts as sc}
              <dt><kbd>{sc.keys}</kbd></dt>
              <dd>{sc.label}</dd>
            {/each}
          </dl>
        </section>
      {/each}
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
    width: min(560px, 92vw);
    max-height: 84vh;
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
  .close-btn {
    background: none;
    border: none;
    font-size: 1rem;
    cursor: pointer;
    color: #555;
  }
  .dialog-body {
    padding: 0.5rem 1rem 1rem;
    overflow-y: auto;
  }
  .dialog-body h3 {
    margin: 0.75rem 0 0.25rem;
    font-size: 0.85rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.2rem 0.75rem;
  }
  dt {
    margin: 0;
  }
  dd {
    margin: 0;
    font-size: 0.9rem;
  }
  kbd {
    display: inline-block;
    padding: 0.05rem 0.4rem;
    font-family: 'Courier New', monospace;
    font-size: 0.8rem;
    background: #f3f3f3;
    border: 1px solid #ccc;
    border-bottom-width: 2px;
    border-radius: 4px;
    white-space: nowrap;
  }
</style>
