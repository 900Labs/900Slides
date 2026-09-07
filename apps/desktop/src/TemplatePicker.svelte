<script lang="ts">
  import { onMount } from 'svelte'
  import type { TemplateInfoDto } from './lib/types'

  interface Props {
    templates: TemplateInfoDto[]
    onSelect: (templateName: string | null) => void
    onCancel: () => void
  }

  let { templates, onSelect, onCancel }: Props = $props()
  let selected = $state<string | null>('default')
  let appearance = $state<'all' | 'light' | 'dark'>('all')
  let dialogEl: HTMLDivElement

  const descriptions: Record<string, { category: string; description: string; sample: string }> = {
    default: { category: 'Everyday', description: 'A clear, versatile starting point for any idea.', sample: 'Make your point.' },
    educator: { category: 'Education', description: 'Warm colors and serif headings for lessons and workshops.', sample: 'A new way to learn.' },
    pitch: { category: 'Business', description: 'A dark theme with a bright accent for plans and proposals.', sample: 'Ideas worth backing.' },
    conference_talk: { category: 'Talks', description: 'Monospace headings and a dark stage for technical talks.', sample: 'Share what you know.' },
    community_update: { category: 'Community', description: 'Friendly green tones for local projects and team updates.', sample: 'Better, together.' },
    photo_essay: { category: 'Visual stories', description: 'A quiet, dark theme to accompany your own photographs.', sample: 'Every image, a story.' },
  }

  function isLight(backgroundHex: string): boolean {
    const hex = backgroundHex.replace('#', '')
    const r = parseInt(hex.slice(0, 2), 16) || 0
    const g = parseInt(hex.slice(2, 4), 16) || 0
    const b = parseInt(hex.slice(4, 6), 16) || 0
    return (0.299 * r + 0.587 * g + 0.114 * b) / 255 > 0.55
  }

  const visibleTemplates = $derived(templates.filter((template) =>
    appearance === 'all' || (isLight(template.backgroundHex) ? 'light' : 'dark') === appearance,
  ))
  const selectedTemplate = $derived(templates.find((template) => template.name === selected))

  $effect(() => {
    if (!visibleTemplates.some((template) => template.name === selected)) {
      selected = visibleTemplates[0]?.name ?? null
    }
  })

  onMount(() => {
    const previousFocus = document.activeElement
    dialogEl.querySelector<HTMLElement>('[role="radio"][aria-checked="true"]')?.focus()
    if (!templates.length) dialogEl.querySelector<HTMLElement>('.primary')?.focus()
    return () => {
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus()
    }
  })

  /** Keep keyboard navigation within the modal and away from editor shortcuts. */
  function onDialogKeydown(event: KeyboardEvent): void {
    event.stopPropagation()
    if (event.key === 'Escape') {
      event.preventDefault()
      onCancel()
      return
    }
    if (event.key === 'Tab') {
      const focusable = Array.from(dialogEl.querySelectorAll<HTMLElement>('button:not([disabled]):not([tabindex="-1"])'))
      const first = focusable[0]
      const last = focusable.at(-1)
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last?.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first?.focus()
      }
      return
    }
    if (!(event.target instanceof HTMLElement) || event.target.getAttribute('role') !== 'radio') return
    const index = visibleTemplates.findIndex((template) => template.name === selected)
    let next = index
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') next = (index + 1) % visibleTemplates.length
    else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') next = (index - 1 + visibleTemplates.length) % visibleTemplates.length
    else if (event.key === 'Home') next = 0
    else if (event.key === 'End') next = visibleTemplates.length - 1
    else return
    event.preventDefault()
    selected = visibleTemplates[next].name
    dialogEl.querySelectorAll<HTMLElement>('[role="radio"]')[next]?.focus()
  }
</script>

<div class="overlay">
  <div bind:this={dialogEl} class="dialog" role="dialog" aria-modal="true" aria-labelledby="template-title" aria-describedby="template-description" tabindex="-1" onkeydown={onDialogKeydown}>
    <header>
      <div>
        <p class="eyebrow">NEW PRESENTATION</p>
        <h2 id="template-title">Start with a little direction.</h2>
        <p id="template-description">Choose a theme for your first slide. Make it your own as you go.</p>
      </div>
      <button type="button" class="close" aria-label="Cancel new presentation" title="Close (Esc)" onclick={onCancel}>×</button>
    </header>

    <div class="filter-row">
      <div class="filters" role="group" aria-label="Theme appearance">
        {#each ['all', 'light', 'dark'] as filter}
          <button type="button" class:active={appearance === filter} aria-pressed={appearance === filter} onclick={() => { appearance = filter as typeof appearance }}>
            {filter === 'all' ? 'All themes' : filter === 'light' ? 'Light' : 'Dark'}
          </button>
        {/each}
      </div>
      <span class="local-label">Included · Available offline</span>
    </div>

    <div class="grid" role="radiogroup" aria-label="Presentation theme">
      {#each visibleTemplates as template (template.name)}
        {@const details = descriptions[template.name]}
        <button type="button" role="radio" aria-checked={selected === template.name} aria-label={`${template.displayName}. ${details?.description ?? 'Presentation theme'}`} tabindex={selected === template.name ? 0 : -1} class="card" class:selected={selected === template.name} onclick={() => { selected = template.name }}>
          <span class="preview" style:background={template.backgroundHex} style:color={isLight(template.backgroundHex) ? '#202637' : '#f5f7fa'} style:font-family={`"${template.headingFont}", ${template.name === 'conference_talk' ? 'monospace' : template.headingFont === 'Georgia' ? 'serif' : 'sans-serif'}`}>
            <span class="preview-rule" style:background={template.accentHex}></span>
            <span class="preview-kicker" style:color={template.accentHex}>YOUR NEXT CHAPTER</span>
            <span class="preview-title">{details?.sample ?? 'A new perspective.'}</span>
            <span class="preview-lines" aria-hidden="true"><span></span><span></span></span>
            {#if selected === template.name}<span class="check" aria-hidden="true">✓</span>{/if}
          </span>
          <span class="card-caption">
            <span class="display-name">{template.displayName}</span>
            <span class="category">{details?.category ?? 'General'}</span>
          </span>
        </button>
      {:else}
        <p class="empty">{templates.length ? 'No themes match this appearance.' : 'Start with a blank presentation. You can choose a theme later.'}</p>
      {/each}
    </div>

    <footer>
      <div class="selection" aria-live="polite">
        <strong>{selectedTemplate?.displayName ?? 'Blank presentation'}</strong>
        <span>{selectedTemplate ? descriptions[selectedTemplate.name]?.description ?? 'A starting theme for your presentation.' : 'A fresh slide, ready for your ideas.'}</span>
        {#if selectedTemplate}<small>Theme preview · {selectedTemplate.headingFont} headings · Uses installed fonts</small>{/if}
      </div>
      <div class="actions">
        <button type="button" class="cancel" onclick={onCancel}>Cancel</button>
        <button type="button" class="primary" onclick={() => onSelect(selected)}>Create presentation <span aria-hidden="true">→</span></button>
      </div>
    </footer>
  </div>
</div>

<style>
  .overlay { position: fixed; inset: 0; padding: 24px; box-sizing: border-box; background: rgba(17, 25, 42, 0.55); display: flex; align-items: center; justify-content: center; z-index: 100; }
  .dialog { width: min(820px, 100%); max-height: 100%; display: flex; flex-direction: column; background: #fbfcfe; color: #20293a; border: 1px solid #dce3ec; border-radius: 16px; box-shadow: 0 20px 70px rgba(12, 25, 46, 0.25); overflow: hidden; outline: none; }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: 20px; padding: 24px 26px 16px; }
  .eyebrow { margin: 0 0 7px; color: #3268a5; font-size: 10px; font-weight: 750; letter-spacing: 0.12em; }
  h2 { margin: 0; font-size: 25px; line-height: 1.2; font-weight: 650; letter-spacing: -0.035em; }
  #template-description { margin: 9px 0 0; font-size: 13px; color: #5a6578; line-height: 1.5; }
  button { font: inherit; cursor: pointer; }
  button:focus-visible { outline: 3px solid #2475c7; outline-offset: 3px; }
  .close { flex-shrink: 0; width: 30px; height: 30px; display: grid; place-content: center; padding: 0; color: #526074; border: 1px solid #d9e0e9; background: #fff; border-radius: 8px; font-size: 22px; line-height: 1; }
  .close:hover, .cancel:hover { background: #eef2f7; }
  .filter-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 0 26px 15px; }
  .filters { display: flex; gap: 3px; padding: 3px; border: 1px solid #e0e6ee; border-radius: 9px; background: #f0f3f8; }
  .filters button { border: 0; background: transparent; border-radius: 6px; padding: 6px 12px; color: #536177; font-size: 12px; }
  .filters button.active { background: #fff; color: #175895; box-shadow: 0 1px 3px #172a4620; font-weight: 650; }
  .local-label { color: #5b6a7a; font-size: 11px; }
  .grid { min-height: 0; display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 16px; padding: 5px 26px 22px; overflow-y: auto; }
  .card { min-width: 0; padding: 0; border: 1px solid #d6dee9; border-radius: 9px; text-align: left; background: #fff; overflow: hidden; color: inherit; }
  .card:hover { border-color: #8096b4; }
  .card.selected { border-color: #2878c4; box-shadow: 0 0 0 2px #2878c4; }
  .preview { position: relative; box-sizing: border-box; display: flex; flex-direction: column; justify-content: center; aspect-ratio: 16 / 9; padding: 18px; overflow: hidden; }
  .preview-rule { position: absolute; left: 0; top: 0; bottom: 0; width: 5px; }
  .preview-kicker { margin-bottom: 8px; font-family: sans-serif; font-size: 7px; font-weight: 700; letter-spacing: 0.07em; }
  .preview-title { display: block; max-width: 160px; font-size: 19px; line-height: 1.15; font-weight: 650; letter-spacing: -0.02em; }
  .preview-lines { display: flex; flex-direction: column; gap: 4px; margin-top: 12px; opacity: 0.22; }
  .preview-lines span { height: 3px; width: 65%; background: currentColor; border-radius: 2px; }
  .preview-lines span:last-child { width: 42%; }
  .check { position: absolute; right: 9px; top: 9px; display: grid; place-content: center; width: 22px; height: 22px; border-radius: 50%; background: #1769b7; color: #fff; box-shadow: 0 0 0 2px #fff; font: 700 12px sans-serif; }
  .card-caption { display: flex; flex-direction: column; gap: 3px; padding: 10px 12px; border-top: 1px solid #e4e9f0; }
  .display-name { font-size: 12px; font-weight: 650; }
  .category { color: #677386; font-size: 10px; }
  .empty { grid-column: 1 / -1; color: #59687b; font-size: 13px; padding: 35px 0; }
  footer { display: flex; justify-content: space-between; align-items: center; gap: 18px; padding: 17px 26px; border-top: 1px solid #e0e6ee; background: #fff; }
  .selection { display: flex; flex-direction: column; gap: 4px; max-width: 340px; }
  .selection strong { font-size: 12px; }
  .selection span { font-size: 11px; color: #596679; line-height: 1.35; }
  .selection small { font-size: 10px; color: #677386; }
  .actions { display: flex; gap: 8px; flex-shrink: 0; }
  .actions button { padding: 10px 13px; border-radius: 7px; font-size: 12px; font-weight: 600; }
  .cancel { border: 1px solid #d5dde8; background: #fff; color: #48586c; }
  .primary { border: 1px solid #145f9f; background: #176ab2; color: #fff; }
  .primary:hover { background: #135990; }
  .primary span { margin-left: 7px; }
  @media (max-width: 700px) {
    .overlay { padding: 16px; }
    header, footer { padding: 18px; }
    .filter-row { padding: 0 18px 12px; }
    .grid { padding: 5px 18px 18px; gap: 12px; grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .local-label { display: none; }
    footer { align-items: stretch; flex-direction: column; gap: 12px; }
    .actions { justify-content: flex-end; }
    .selection { max-width: none; }
    h2 { font-size: 22px; }
  }
  @media (max-height: 640px) {
    .overlay { padding: 12px; }
    header { padding-top: 16px; padding-bottom: 12px; }
    .selection span, .selection small { display: none; }
    footer { padding-top: 12px; padding-bottom: 12px; }
  }
</style>
