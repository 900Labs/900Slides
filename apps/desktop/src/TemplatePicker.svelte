<script lang="ts">
  import type { TemplateInfoDto } from './lib/types'

  /** Props for the template picker dialog. */
  interface Props {
    /** Available built-in templates, in canonical order. */
    templates: TemplateInfoDto[]
    /** Creates a new deck from the chosen template (or the default blank deck). */
    onSelect: (templateName: string | null) => void
    /** Closes the picker without creating a deck. */
    onCancel: () => void
  }

  let { templates, onSelect, onCancel }: Props = $props()

  /** The currently selected template name, or null for the default blank deck. */
  let selected = $state<string | null>('default')

  /** Resolves a contrasting text color for a background hex color. */
  function textColorFor(backgroundHex: string): string {
    const hex = backgroundHex.replace('#', '')
    const r = parseInt(hex.slice(0, 2), 16) || 0
    const g = parseInt(hex.slice(2, 4), 16) || 0
    const b = parseInt(hex.slice(4, 6), 16) || 0
    // Relative luminance — dark backgrounds get light text.
    const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255
    return luminance > 0.55 ? '#222' : '#fff'
  }
</script>

<div class="overlay">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="template-title">
    <h2 id="template-title">Choose a template</h2>
    <p>Pick a starting theme for your new deck. You can switch templates later.</p>

    <div class="grid" role="radiogroup" aria-label="Template">
      {#each templates as template}
        <button
          type="button"
          role="radio"
          aria-checked={selected === template.name}
          class="card"
          class:selected={selected === template.name}
          style="background: {template.backgroundHex}; color: {textColorFor(
            template.backgroundHex,
          )}"
          onclick={() => (selected = template.name)}
        >
          <div class="card-body">
            <div class="display-name">{template.displayName}</div>
            <div class="fonts">{template.headingFont}</div>
            <div class="swatch-row">
              <span class="swatch bg" title={`Background ${template.backgroundHex}`}></span>
              <span
                class="swatch accent"
                style="background: {template.accentHex}"
                title={`Accent ${template.accentHex}`}
              ></span>
            </div>
          </div>
          {#if selected === template.name}
            <span class="check" aria-hidden="true">✓</span>
          {/if}
        </button>
      {/each}
    </div>

    <div class="actions">
      <button type="button" onclick={onCancel}>Cancel</button>
      <button type="button" class="primary" onclick={() => onSelect(selected)}>
        Create
      </button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .dialog {
    background: #fff;
    color: #222;
    padding: 1.5rem;
    border-radius: 0.5rem;
    width: 90%;
    max-width: 640px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  }
  .dialog h2 {
    margin-top: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.75rem;
    margin: 1rem 0;
  }
  .card {
    position: relative;
    padding: 0.75rem;
    border: 2px solid transparent;
    border-radius: 0.4rem;
    cursor: pointer;
    text-align: left;
    overflow: hidden;
  }
  .card.selected {
    border-color: #0070c0;
    box-shadow: 0 0 0 2px #0070c0;
  }
  .display-name {
    font-weight: 600;
    font-size: 0.95rem;
  }
  .fonts {
    font-size: 0.75rem;
    opacity: 0.85;
    margin-top: 0.15rem;
  }
  .swatch-row {
    display: flex;
    gap: 0.3rem;
    margin-top: 0.5rem;
  }
  .swatch {
    width: 1rem;
    height: 1rem;
    border-radius: 50%;
    border: 1px solid rgba(0, 0, 0, 0.2);
  }
  .swatch.bg {
    background: #fff;
  }
  .check {
    position: absolute;
    top: 0.35rem;
    right: 0.45rem;
    font-weight: bold;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
  .primary {
    background: #0070c0;
    color: #fff;
  }
</style>
