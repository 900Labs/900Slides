/**
 * Coalesces typing without letting an older asynchronous commit erase a newer
 * draft. A flush waits for every edit queued while it is running. Rejections
 * retain the latest draft so explicit save/navigation can retry safely.
 *
 * @template T
 * @param {{commit: (value: T) => void | Promise<void>, onChange?: (dirty: boolean) => void, delay?: number, maxWait?: number}} options
 */
export function createDraftQueue({ commit, onChange = () => {}, delay = 600, maxWait = 2000 }) {
  /** @type {Map<string, {value: T}>} */
  const pending = new Map()
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let debounce
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let deadline
  /** @type {Promise<void> | undefined} */
  let active

  function clearTimers() {
    clearTimeout(debounce)
    clearTimeout(deadline)
    debounce = undefined
    deadline = undefined
  }

  /** @returns {Promise<void>} */
  function flush() {
    clearTimers()
    if (active) return active
    active = Promise.resolve().then(async () => {
      while (pending.size > 0) {
        const next = pending.entries().next().value
        if (!next) break
        const [key, entry] = next
        await commit(entry.value)
        if (pending.get(key) === entry) pending.delete(key)
        onChange(pending.size > 0)
      }
    }).finally(() => { active = undefined })
    return active
  }

  function backgroundFlush() {
    // The caller's command handler reports failures. Keep dirty state and do
    // not retry endlessly on an unavailable backend; new input or Save retries.
    void flush().catch(() => {})
  }

  return {
    /** @param {string} key @param {T} value */
    set(key, value) {
      pending.set(key, { value })
      onChange(true)
      if (active) return
      clearTimeout(debounce)
      debounce = setTimeout(backgroundFlush, delay)
      deadline ??= setTimeout(backgroundFlush, maxWait)
    },
    /** @param {string} key @returns {T | undefined} */
    peek(key) { return pending.get(key)?.value },
    isDirty() { return pending.size > 0 },
    flush,
    // Hosts flush before unmounting; destruction only cancels stale timers.
    dispose: clearTimers,
  }
}
