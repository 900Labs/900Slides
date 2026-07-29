<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import type {
    CommentAnchorDto,
    CommentThreadDto,
    DeckSnapshot,
    TextBoxSnapshot,
  } from './lib/types'

  interface Props {
    /** Current deck snapshot (the source of truth, owned by App). */
    deck: DeckSnapshot
    /** Id of the slide currently shown in the editor. */
    activeSlideId: string
    /** A pending anchor the user invoked from a context menu, or null. */
    draft: CommentAnchorDto | null
    /** Adopts the deck snapshot returned by each comment command. */
    onApplied: (deck: DeckSnapshot) => void
    /** Clears the pending draft anchor after it has been used. */
    onClearDraft: () => void
    /** Closes the comments sidebar. */
    onClose: () => void
  }

  let {
    deck,
    activeSlideId,
    draft,
    onApplied,
    onClearDraft,
    onClose,
  }: Props = $props()

  /** Author name applied to new comments and replies (free text). */
  let author = $state('You')
  /** Body of the new-thread form. */
  let newBody = $state('')
  /** Per-thread reply bodies, keyed by thread id. */
  let replyBody = $state<Record<string, string>>({})
  /** Per-thread assignee inputs, keyed by thread id. */
  let assignInput = $state<Record<string, string>>({})
  /** Unresolved threads the user explicitly collapsed. */
  let collapsed = $state<Set<string>>(new Set())
  /** Resolved threads the user explicitly expanded. */
  let forceOpen = $state<Set<string>>(new Set())
  /** Last error message, shown until dismissed. */
  let error = $state('')
  /** True while any command is in flight. */
  let busy = $state(false)

  /** Concatenated text of a text box (paragraphs joined by newline), matching
   *  the editor textarea so byte offsets line up. */
  function textBoxText(shape: TextBoxSnapshot): string {
    return shape.paragraphs
      .map((paragraph) => paragraph.runs.map((run) => run.text).join(''))
      .join('\n')
  }

  /** Finds a shape by id on the given slide, or null. */
  function findShape(slideId: string, shapeId: string): TextBoxSnapshot | null {
    const slide = deck.slides.find((s) => s.id === slideId)
    if (!slide) return null
    const shape = slide.shapes.find((sh) => {
      if (sh.kind !== 'text_box') return false
      return (sh.value as TextBoxSnapshot).id === shapeId
    })
    return shape ? (shape.value as TextBoxSnapshot) : null
  }

  /** Returns the byte slice of a text box between [start, end). */
  function sliceBytes(text: string, start: number, end: number): string {
    const bytes = new TextEncoder().encode(text)
    const s = Math.max(0, Math.min(start, bytes.length))
    const e = Math.max(s, Math.min(end, bytes.length))
    return new TextDecoder().decode(bytes.slice(s, e))
  }

  /** One-based slide number for an id, or 0 when the slide is gone. */
  function slideNumber(slideId: string): number {
    const idx = deck.slides.findIndex((s) => s.id === slideId)
    return idx >= 0 ? idx + 1 : 0
  }

  /** Human-readable anchor context for a thread (slide number + shape/excerpt). */
  function anchorLabel(thread: CommentThreadDto): string {
    const a = thread.anchor
    const num = slideNumber(a.slideId)
    const slideText = num > 0 ? `Slide ${num}` : 'Unknown slide'
    if (a.kind === 'slide') return slideText
    if (a.kind === 'shape') return `${slideText} · shape`
    // text_range
    const box = findShape(a.slideId, a.shapeId)
    let excerpt = ''
    if (box) {
      excerpt = sliceBytes(textBoxText(box), a.start, a.end).replace(/\s+/g, ' ').trim()
    }
    return excerpt ? `${slideText} · “${excerpt.slice(0, 40)}”` : `${slideText} · text`
  }

  /** The anchor used by the new-comment form: the pending draft, or a
   *  whole-slide anchor on the active slide. */
  const newAnchor = $derived<CommentAnchorDto>(
    draft ?? { kind: 'slide', slideId: activeSlideId },
  )

  /** Label for the new-comment form's target. */
  const newAnchorLabel = $derived.by(() => {
    if (draft) {
      const dummy: CommentThreadDto = {
        id: '',
        anchor: draft,
        comments: [],
        resolved: false,
      }
      return anchorLabel(dummy)
    }
    return `Slide ${slideNumber(activeSlideId) || 'current'}`
  })

  /** Threads grouped by slide, in slide order, with orphaned threads last. */
  const groups = $derived.by<{ label: string; threads: CommentThreadDto[] }[]>(() => {
    const all = deck.comments
    const result: { label: string; threads: CommentThreadDto[] }[] = []
    const seen = new Set<string>()
    for (let i = 0; i < deck.slides.length; i += 1) {
      const slide = deck.slides[i]
      const threads = all.filter((t) => t.anchor.slideId === slide.id)
      if (threads.length > 0) {
        seen.add(slide.id)
        result.push({ label: `Slide ${i + 1}`, threads })
      }
    }
    const orphans = all.filter((t) => !seen.has(t.anchor.slideId))
    if (orphans.length > 0) {
      result.push({ label: 'Other', threads: orphans })
    }
    return result
  })

  /** Whether a thread should render expanded. */
  function isExpanded(thread: CommentThreadDto): boolean {
    return thread.resolved ? forceOpen.has(thread.id) : !collapsed.has(thread.id)
  }

  function toggleExpand(thread: CommentThreadDto): void {
    if (thread.resolved) {
      const next = new Set(forceOpen)
      if (next.has(thread.id)) next.delete(thread.id)
      else next.add(thread.id)
      forceOpen = next
    } else {
      const next = new Set(collapsed)
      if (next.has(thread.id)) next.delete(thread.id)
      else next.add(thread.id)
      collapsed = next
    }
  }

  function formatTime(timestamp: string): string {
    const date = new Date(timestamp)
    return Number.isNaN(date.getTime()) ? timestamp : date.toLocaleString()
  }

  /** Runs a command, surfacing errors and adopting the returned snapshot. */
  async function run(task: () => Promise<DeckSnapshot>): Promise<void> {
    busy = true
    error = ''
    try {
      const snapshot = await task()
      onApplied(snapshot)
    } catch (e) {
      error = String(e)
    } finally {
      busy = false
    }
  }

  /** Creates a new thread from the new-comment form. */
  async function doAdd(): Promise<void> {
    const body = newBody.trim()
    const name = author.trim() || 'You'
    if (!body) return
    await run(() =>
      invoke<DeckSnapshot>('add_comment', {
        anchor: newAnchor,
        author: name,
        body,
      }),
    )
    newBody = ''
    if (draft) onClearDraft()
  }

  /** Appends a reply to a thread. */
  async function doReply(thread: CommentThreadDto): Promise<void> {
    const body = (replyBody[thread.id] ?? '').trim()
    const name = author.trim() || 'You'
    if (!body) return
    await run(() =>
      invoke<DeckSnapshot>('reply_to_comment', {
        threadId: thread.id,
        slideId: thread.anchor.slideId,
        author: name,
        body,
      }),
    )
    replyBody = { ...replyBody, [thread.id]: '' }
  }

  /** Toggles a thread's resolved flag. */
  async function doResolve(thread: CommentThreadDto): Promise<void> {
    await run(() =>
      invoke<DeckSnapshot>('set_comment_resolved', {
        threadId: thread.id,
        slideId: thread.anchor.slideId,
        resolved: !thread.resolved,
      }),
    )
  }

  /** Commits the assignee input (empty clears the assignee). */
  async function commitAssign(thread: CommentThreadDto): Promise<void> {
    const value = (assignInput[thread.id] ?? '').trim()
    await run(() =>
      invoke<DeckSnapshot>('assign_comment', {
        threadId: thread.id,
        slideId: thread.anchor.slideId,
        assignee: value === '' ? null : value,
      }),
    )
  }

  /** Removes a thread (undoable via Undo). */
  async function doDelete(thread: CommentThreadDto): Promise<void> {
    await run(() =>
      invoke<DeckSnapshot>('delete_comment_thread', {
        threadId: thread.id,
        slideId: thread.anchor.slideId,
      }),
    )
  }

  function onKey(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<aside class="comments" aria-label="Comments">
  <header class="comments-header">
    <h2>Comments</h2>
    <button type="button" class="close-btn" onclick={onClose} aria-label="Close comments">✕</button>
  </header>

  {#if error}
    <div class="banner" role="alert">
      {error}
      <button type="button" onclick={() => (error = '')}>Dismiss</button>
    </div>
  {/if}

  <div class="comments-body">
    <div class="new-comment">
      <label class="author-field">
        Author
        <input
          type="text"
          bind:value={author}
          placeholder="Your name"
          aria-label="Comment author"
        />
      </label>
      <div class="new-target">On: {newAnchorLabel}{draft ? ' (from selection)' : ''}</div>
      <textarea
        bind:value={newBody}
        placeholder="Write a comment…"
        aria-label="New comment body"
        rows="2"
      ></textarea>
      <div class="new-actions">
        {#if draft}
          <button type="button" class="ghost" onclick={onClearDraft}>Clear target</button>
        {/if}
        <button type="button" class="action" onclick={() => void doAdd()} disabled={busy || !newBody.trim()}>
          Add comment
        </button>
      </div>
      <p class="hint">Tip: right-click a shape or selected text to anchor a comment.</p>
    </div>

    {#if groups.length === 0}
      <p class="empty">No comments yet. Add one above or right-click the canvas.</p>
    {:else}
      {#each groups as group (group.label)}
        {#if group.threads.length > 0}
          <section class="slide-group">
            <h3>{group.label}</h3>
            {#each group.threads as thread (thread.id)}
              {@const root = thread.comments[0]}
              {@const replies = thread.comments.slice(1)}
              {@const expanded = isExpanded(thread)}
              <article class="thread" class:resolved={thread.resolved} class:dimmed={thread.resolved && !expanded}>
                <div class="thread-head">
                  <button
                    type="button"
                    class="expand"
                    onclick={() => toggleExpand(thread)}
                    aria-expanded={expanded}
                    title={expanded ? 'Collapse' : 'Expand'}
                  >
                    {expanded ? '▼' : '▶'}
                  </button>
                  <span class="anchor" title={anchorLabel(thread)}>{anchorLabel(thread)}</span>
                  <span class="head-spacer"></span>
                  <button
                    type="button"
                    class="resolve"
                    class:on={thread.resolved}
                    onclick={() => void doResolve(thread)}
                    disabled={busy}
                    title={thread.resolved ? 'Mark unresolved' : 'Resolve'}
                  >
                    {thread.resolved ? '✓ Resolved' : 'Resolve'}
                  </button>
                </div>

                {#if expanded}
                  <div class="root-comment">
                    <div class="meta">
                      <strong>{root?.author ?? 'Unknown'}</strong>
                      <span class="time">{root ? formatTime(root.timestamp) : ''}</span>
                    </div>
                    <p class="body">{root?.body ?? ''}</p>
                    {#if thread.assignedTo}
                      <p class="assigned">Assigned to {thread.assignedTo}</p>
                    {/if}

                    {#if replies.length > 0}
                      <ul class="replies">
                        {#each replies as reply (reply.id)}
                          <li>
                            <div class="meta">
                              <strong>{reply.author}</strong>
                              <span class="time">{formatTime(reply.timestamp)}</span>
                            </div>
                            <p class="body">{reply.body}</p>
                          </li>
                        {/each}
                      </ul>
                    {/if}

                    <div class="thread-tools">
                      <input
                        type="text"
                        class="assign-input"
                        value={assignInput[thread.id] ?? thread.assignedTo ?? ''}
                        placeholder="Assign to…"
                        aria-label="Assignee"
                        onchange={(e) => {
                          assignInput = { ...assignInput, [thread.id]: (e.target as HTMLInputElement).value }
                          void commitAssign(thread)
                        }}
                      />
                      <button type="button" class="danger" onclick={() => void doDelete(thread)} disabled={busy}>
                        Delete
                      </button>
                    </div>

                    <div class="reply-row">
                      <input
                        type="text"
                        placeholder="Reply…"
                        value={replyBody[thread.id] ?? ''}
                        aria-label="Reply body"
                        oninput={(e) => {
                          replyBody = { ...replyBody, [thread.id]: (e.target as HTMLInputElement).value }
                        }}
                        onkeydown={(e) => {
                          if (e.key === 'Enter') {
                            e.preventDefault()
                            void doReply(thread)
                          }
                        }}
                      />
                      <button
                        type="button"
                        class="action"
                        onclick={() => void doReply(thread)}
                        disabled={busy || !(replyBody[thread.id] ?? '').trim()}
                      >
                        Reply
                      </button>
                    </div>
                  </div>
                {:else}
                  <div class="collapsed-summary">
                    <span class="meta"><strong>{root?.author ?? 'Unknown'}</strong></span>
                    <span class="summary-body">{root?.body ?? ''}</span>
                  </div>
                {/if}
              </article>
            {/each}
          </section>
        {/if}
      {/each}
    {/if}
  </div>
</aside>

<style>
  .comments {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: 360px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    background: #fafafa;
    border-left: 1px solid #ccc;
    box-shadow: -4px 0 16px rgba(0, 0, 0, 0.12);
  }
  .comments-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.6rem 0.75rem;
    border-bottom: 1px solid #e0e0e0;
    background: #f0f0f0;
  }
  .comments-header h2 {
    margin: 0;
    font-size: 1.05rem;
  }
  .close-btn {
    background: none;
    border: none;
    font-size: 1rem;
    cursor: pointer;
    color: #555;
  }
  .banner {
    margin: 0.5rem 0.75rem 0;
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
  .comments-body {
    flex: 1;
    overflow-y: auto;
    padding: 0.6rem 0.75rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .new-comment {
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    background: #fff;
    padding: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .author-field {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.78rem;
    color: #555;
  }
  .author-field input,
  .new-comment textarea,
  .assign-input,
  .reply-row input {
    padding: 0.3rem 0.4rem;
    border: 1px solid #ccc;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.85rem;
  }
  .new-comment textarea {
    resize: vertical;
  }
  .new-target {
    font-size: 0.78rem;
    color: #666;
  }
  .new-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.4rem;
  }
  .hint {
    margin: 0;
    font-size: 0.72rem;
    color: #999;
  }
  .empty {
    font-size: 0.85rem;
    color: #777;
  }
  .slide-group {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .slide-group h3 {
    margin: 0.25rem 0 0.1rem;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #888;
  }
  .thread {
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    background: #fff;
    overflow: hidden;
  }
  .thread.resolved {
    background: #f6f6f6;
  }
  .thread.dimmed {
    opacity: 0.7;
  }
  .thread-head {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.35rem 0.45rem;
    border-bottom: 1px solid #eee;
  }
  .expand {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 0.7rem;
    color: #555;
    padding: 0 0.15rem;
  }
  .anchor {
    font-size: 0.78rem;
    color: #0070c0;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .head-spacer {
    flex: 1;
  }
  .resolve {
    border: 1px solid #ccc;
    background: #fff;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0.15rem 0.4rem;
  }
  .resolve.on {
    background: #e7f4e4;
    border-color: #9ccf93;
    color: #2f6b25;
  }
  .resolve:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .root-comment {
    padding: 0.4rem 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .meta {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    font-size: 0.78rem;
  }
  .time {
    color: #999;
    font-size: 0.72rem;
  }
  .body {
    margin: 0;
    font-size: 0.85rem;
    color: #222;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .assigned {
    margin: 0;
    font-size: 0.75rem;
    color: #6a4ba5;
  }
  .replies {
    list-style: none;
    margin: 0.2rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .replies li {
    border-left: 2px solid #e0e0e0;
    padding-left: 0.45rem;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }
  .thread-tools {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    margin-top: 0.15rem;
  }
  .assign-input {
    flex: 1;
  }
  .danger {
    border: 1px solid #e5b7b3;
    background: #fdecea;
    color: #8a3b2e;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.78rem;
    padding: 0.2rem 0.5rem;
  }
  .danger:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .reply-row {
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }
  .reply-row input {
    flex: 1;
  }
  .action {
    border: 1px solid #0070c0;
    background: #0070c0;
    color: #fff;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0.25rem 0.6rem;
    white-space: nowrap;
  }
  .action:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .ghost {
    border: 1px solid #ccc;
    background: #f7f7f7;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0.25rem 0.6rem;
  }
  .collapsed-summary {
    padding: 0.3rem 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }
  .summary-body {
    font-size: 0.8rem;
    color: #777;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
