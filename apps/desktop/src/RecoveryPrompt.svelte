<script lang="ts">
  import { modalFocus } from './lib/modalFocus.js'
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
  <div class="dialog" use:modalFocus={onSkip} tabindex="-1" role="dialog" aria-modal="true" aria-labelledby="recovery-title">
    <h2 id="recovery-title">Recover your work</h2>
    <p>Your original files are unchanged. Restore a recovery copy, or start a new deck and keep these copies for later.</p>

    <ul>
      {#each snapshots as snapshot, i}
        <li>
          <span class="meta">Recovery {i + 1}<br /><time>{formatTimestamp(snapshot.timestamp)}</time></span>
          <div class="actions">
            <button onclick={() => onRestore(snapshot.id)} type="button">Restore</button>
            <button onclick={() => onDiscard(snapshot.id)} type="button">Discard</button>
          </div>
        </li>
      {/each}
    </ul>

    <button class="skip" onclick={onSkip} type="button">Keep copies and start new deck</button>
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
    max-width: 560px;
    max-height: calc(100vh - 40px);
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  }
  .dialog h2 {
    margin-top: 0;
  }
  .dialog ul {
    overflow-y: auto;
    min-height: 0;
    list-style: none;
    padding: 0;
  }
  .dialog li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem 0;
    border-bottom: 1px solid #eee;
  }
  time { font-size: 0.82rem; color: #576577; }
  button:focus-visible { outline: 2px solid #2457a7; outline-offset: 3px; }
  .actions {
    display: flex;
    gap: 0.5rem;
  }
  .skip {
    margin-top: 1rem;
    width: 100%;
  }
</style>
