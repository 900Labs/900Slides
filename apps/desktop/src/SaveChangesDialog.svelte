<script lang="ts">
  import { onMount } from 'svelte'

  let { name, action, onChoose }: {
    name: string
    action: string
    onChoose: (choice: 'save' | 'discard' | 'cancel') => void
  } = $props()
  let dialog: HTMLDialogElement

  onMount(() => {
    dialog.showModal()
    dialog.querySelector<HTMLButtonElement>('.primary')?.focus()
    return () => dialog.close()
  })
</script>

<dialog bind:this={dialog} aria-labelledby="save-changes-title" aria-describedby="save-changes-detail"
  oncancel={(event) => { event.preventDefault(); onChoose('cancel') }}>
  <span class="eyebrow">Unsaved changes</span>
  <h2 id="save-changes-title">Save “{name}”?</h2>
  <p id="save-changes-detail">Save your changes before {action}. Discarding removes changes since your last save.</p>
  <div class="actions">
    <button class="discard" type="button" onclick={() => onChoose('discard')}>Discard changes</button>
    <span></span>
    <button type="button" onclick={() => onChoose('cancel')}>Cancel</button>
    <button class="primary" type="button" onclick={() => onChoose('save')}>Save</button>
  </div>
</dialog>

<style>
  dialog { width: min(460px, calc(100vw - 3rem)); padding: 1.5rem; border: 1px solid #d3dbea; border-radius: 14px; color: #243147; box-shadow: 0 24px 80px #172a4540; }
  dialog::backdrop { background: #14213d66; }
  .eyebrow { font-size: .72rem; font-weight: 700; text-transform: uppercase; letter-spacing: .08em; color: #65758c; }
  h2 { font-size: 1.25rem; margin: .7rem 0; overflow-wrap: anywhere; }
  p { color: #5a687b; font-size: .9rem; line-height: 1.6; }
  .actions { display: flex; flex-wrap: wrap; align-items: center; gap: .5rem; margin-top: 1.4rem; }
  .actions span { flex: 1; }
  button { background: #fff; border: 1px solid #cbd4e2; border-radius: 6px; padding: .65rem .85rem; color: #25344d; cursor: pointer; }
  button:hover { background: #f0f4fa; }
  button:focus-visible { outline: 3px solid #4b91e2; outline-offset: 3px; }
  .primary { background: #245eb3; border-color: #245eb3; color: white; }
  .primary:hover { background: #1c4a8e; }
  .discard { color: #9a3541; }
</style>
