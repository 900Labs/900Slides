<script lang="ts">
  import type {
    AccessibilityIssueDto,
    AccessibilityReportDto,
    DeckSnapshot,
    IssueCategoryDto,
    IssueSeverityDto,
  } from './lib/types'

  interface Props {
    /** Latest accessibility report, or null while loading/none. */
    report: AccessibilityReportDto | null
    /** Current deck snapshot, used to resolve slide numbers from ids. */
    deck: DeckSnapshot
    /** True while a check is running. */
    checking: boolean
    /** Re-runs the accessibility check. */
    onRecheck: () => void
    /** Navigates to an issue's slide and selects its shape, when applicable. */
    onNavigate: (slideId: string | null, shapeIndex: number | null) => void
    /** Closes the panel. */
    onClose: () => void
  }

  let { report, deck, checking, onRecheck, onNavigate, onClose }: Props = $props()

  /** Display order and labels for issue categories. */
  const CATEGORY_LABELS: { category: IssueCategoryDto; label: string }[] = [
    { category: 'missing_alt_text', label: 'Missing alt text' },
    { category: 'low_contrast', label: 'Low contrast' },
    { category: 'missing_title', label: 'Missing title' },
    { category: 'small_text', label: 'Small text' },
    { category: 'reading_order', label: 'Reading order' },
    { category: 'empty_slide', label: 'Empty slides' },
  ]

  /** Issues grouped by category, in a stable display order, each group sorted
   *  by slide number. Empty groups are omitted. */
  const groups = $derived.by<{ category: IssueCategoryDto; label: string; issues: AccessibilityIssueDto[] }[]>(() => {
    const issues = report?.issues ?? []
    return CATEGORY_LABELS.map(({ category, label }) => {
      const items = issues
        .filter((issue) => issue.category === category)
        .slice()
        .sort((a, b) => slideNumber(a) - slideNumber(b))
      return { category, label, issues: items }
    }).filter((group) => group.issues.length > 0)
  })

  /** Score tier for the colored badge: green >= 90, yellow >= 70, red < 70. */
  const tier = $derived.by<'green' | 'yellow' | 'red'>(() => {
    const score = report?.score ?? 0
    if (score >= 90) return 'green'
    if (score >= 70) return 'yellow'
    return 'red'
  })

  /** One-based slide number for an issue, or 0 when the slide is unknown. */
  function slideNumber(issue: AccessibilityIssueDto): number {
    if (!issue.slideId) return 0
    const idx = deck.slides.findIndex((s) => s.id === issue.slideId)
    return idx >= 0 ? idx + 1 : 0
  }

  /** Short, human-readable severity glyph. */
  function severityGlyph(severity: IssueSeverityDto): string {
    if (severity === 'error') return '✕'
    if (severity === 'warning') return '!'
    return '·'
  }

  /** Navigates to the issue's slide and selects its shape. */
  function goto(issue: AccessibilityIssueDto): void {
    onNavigate(issue.slideId ?? null, issue.shapeIndex ?? null)
  }

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
    aria-label="Close accessibility panel"
    onclick={onClose}
  ></button>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Accessibility checker">
    <div class="dialog-header">
      <h2>Accessibility</h2>
      <div class="header-actions">
        <button type="button" class="ghost" onclick={onRecheck} disabled={checking}>
          {checking ? 'Checking…' : 'Re-check'}
        </button>
        <button type="button" class="close-btn" onclick={onClose} aria-label="Close">✕</button>
      </div>
    </div>

    <div class="body">
      <section class="score-card">
        <div class="score" class:tier-green={tier === 'green'} class:tier-yellow={tier === 'yellow'} class:tier-red={tier === 'red'}>
          {report ? report.score : '–'}
        </div>
        <div class="score-meta">
          <div class="score-title">WCAG 2.2 AA score</div>
          {#if report}
            {#if report.issues.length === 0}
              <p class="score-summary good">No issues found — this deck meets WCAG 2.2 AA.</p>
            {:else}
              <p class="score-summary">
                {report.issues.length} issue{report.issues.length === 1 ? '' : 's'} across
                {report.slidesWithIssues} of {report.totalSlides}
                slide{report.totalSlides === 1 ? '' : 's'}.
              </p>
            {/if}
          {:else if checking}
            <p class="score-summary">Checking the deck…</p>
          {:else}
            <p class="score-summary">Run a check to see the score.</p>
          {/if}
        </div>
      </section>

      {#if report && report.issues.length === 0}
        <p class="empty">Nothing to fix. 🎉</p>
      {:else if groups.length === 0}
        <p class="empty">No issues to show. Click “Re-check” to audit the deck.</p>
      {:else}
        <div class="issue-groups">
          {#each groups as group (group.category)}
            <section class="issue-group">
              <h3>{group.label} <span class="count">{group.issues.length}</span></h3>
              <ul class="issue-list">
                {#each group.issues as issue, i (`${group.category}-${i}`)}
                  <li>
                    <button
                      type="button"
                      class="issue"
                      class:sev-error={issue.severity === 'error'}
                      class:sev-warning={issue.severity === 'warning'}
                      class:sev-suggestion={issue.severity === 'suggestion'}
                      onclick={() => goto(issue)}
                      disabled={!issue.slideId}
                      title={issue.slideId ? 'Go to this slide' : 'Not tied to a slide'}
                    >
                      <span class="sev-badge" aria-hidden="true">{severityGlyph(issue.severity)}</span>
                      <span class="issue-main">
                        <span class="issue-top">
                          {#if slideNumber(issue) > 0}
                            <span class="slide-no">Slide {slideNumber(issue)}</span>
                          {:else}
                            <span class="slide-no dim">Deck-wide</span>
                          {/if}
                          <span class="sev-label">{issue.severity}</span>
                        </span>
                        <span class="issue-msg">{issue.message}</span>
                        {#if issue.fixHint}
                          <span class="issue-hint">Fix: {issue.fixHint}</span>
                        {/if}
                      </span>
                    </button>
                  </li>
                {/each}
              </ul>
            </section>
          {/each}
        </div>
      {/if}
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
    width: min(640px, 94vw);
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
  .body {
    padding: 0.75rem 1rem 1rem;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .score-card {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    background: #fafafa;
    padding: 0.6rem 0.8rem;
  }
  .score {
    width: 3.6rem;
    height: 3.6rem;
    flex: 0 0 3.6rem;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
    font-weight: 700;
    color: #fff;
    background: #999;
  }
  .tier-green {
    background: #2e7d32;
  }
  .tier-yellow {
    background: #c77700;
  }
  .tier-red {
    background: #c62828;
  }
  .score-title {
    font-weight: 600;
    font-size: 0.9rem;
  }
  .score-summary {
    margin: 0.15rem 0 0;
    font-size: 0.82rem;
    color: #555;
  }
  .score-summary.good {
    color: #2e7d32;
  }
  .empty {
    font-size: 0.85rem;
    color: #777;
    margin: 0.5rem 0;
    text-align: center;
  }
  .issue-groups {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .issue-group h3 {
    margin: 0 0 0.3rem;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #666;
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .count {
    background: #e5e5e5;
    color: #444;
    border-radius: 999px;
    padding: 0 0.4rem;
    font-size: 0.7rem;
  }
  .issue-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .issue {
    width: 100%;
    text-align: left;
    background: #fff;
    border: 1px solid #e0e0e0;
    border-left-width: 4px;
    border-radius: 6px;
    padding: 0.4rem 0.5rem;
    cursor: pointer;
    display: flex;
    gap: 0.5rem;
    align-items: flex-start;
  }
  .issue:disabled {
    cursor: default;
    opacity: 0.85;
  }
  .issue:not(:disabled):hover {
    background: #f5f9ff;
  }
  .sev-error {
    border-left-color: #c62828;
  }
  .sev-warning {
    border-left-color: #c77700;
  }
  .sev-suggestion {
    border-left-color: #1f6feb;
  }
  .sev-badge {
    flex: 0 0 1.1rem;
    height: 1.1rem;
    border-radius: 50%;
    color: #fff;
    font-size: 0.7rem;
    font-weight: 700;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-top: 0.1rem;
  }
  .sev-error .sev-badge {
    background: #c62828;
  }
  .sev-warning .sev-badge {
    background: #c77700;
  }
  .sev-suggestion .sev-badge {
    background: #1f6feb;
  }
  .issue-main {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }
  .issue-top {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }
  .slide-no {
    font-size: 0.75rem;
    font-weight: 600;
    color: #0070c0;
  }
  .slide-no.dim {
    color: #999;
  }
  .sev-label {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #888;
  }
  .issue-msg {
    font-size: 0.85rem;
    color: #222;
    word-break: break-word;
  }
  .issue-hint {
    font-size: 0.78rem;
    color: #6a4ba5;
  }
</style>
