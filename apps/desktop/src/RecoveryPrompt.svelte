<script lang="ts">
  import type { RecoverySnapshot } from './lib/types'

  /** Props for the recovery prompt dialog. */
  interface Props {
    /** Available recovery snapshots, newest first. */
    snapshots: RecoverySnapshot[]
    /** Restores the selected snapshot. */
    onRestore: (id: string) => void
    /** Discards the selected snapshot. */
    onDiscard: (id: string) => void
    /** Skips recovery and starts a new deck. */
    onSkip: () => void
  }

  let { snapshots, onRestore, onDiscard, onSkip }: Props = $props()

  /** Formats a filename timestamp as a local date/time string. */
  function formatTimestamp(timestamp: string): string {
    const ms = parseInt(timestamp, 10)
    if (Number.isNaN(ms)) return timestamp
    return new Date(ms).toLocaleString()
  }
</script>

<div class="overlay">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="recovery-title">
    <h2 id="recovery-title">Recovery snapshots found</h2>
    <p>900Slides found unsaved recovery snapshots. Choose one to restore or skip.</p>

    <ul>
      {#each snapshots as snapshot}
        <li>
          <span class="meta">{snapshot.deckId} — {formatTimestamp(snapshot.timestamp)}</span>
          <div class="actions">
            <button onclick={() => onRestore(snapshot.id)} type="button">Restore</button>
            <button onclick={() => onDiscard(snapshot.id)} type="button">Discard</button>
          </div>
        </li>
      {/each}
    </ul>

    <button class="skip" onclick={onSkip} type="button">Skip and start new deck</button>
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
    max-width: 480px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  }
  .dialog h2 {
    margin-top: 0;
  }
  .dialog ul {
    list-style: none;
    padding: 0;
  }
  .dialog li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0;
    border-bottom: 1px solid #eee;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
  }
  .skip {
    margin-top: 1rem;
    width: 100%;
  }
</style>
