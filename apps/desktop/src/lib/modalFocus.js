/**
 * Keep keyboard focus inside a modal and return it to the invoking control.
 * The backdrop remains responsible for pointer interception.
 * @param {HTMLElement} node
 * @param {() => void} onEscape
 */
export function modalFocus(node, onEscape) {
  const previous = document.activeElement
  let active = true
  const controls = () => Array.from(node.querySelectorAll(
    'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex="0"]',
  )).filter((element) => element instanceof HTMLElement && element.getClientRects().length > 0)
  queueMicrotask(() => {
    if (!active) return
    const first = controls()[0]
    if (first instanceof HTMLElement) first.focus()
    else node.focus()
  })
  /** @param {KeyboardEvent} event */
  const handleKey = (event) => {
    // Editor shortcuts must never act on the slide behind a modal.
    event.stopPropagation()
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onEscape()
    } else if (event.key === 'Tab') {
      const items = controls()
      const first = items[0]
      const last = items[items.length - 1]
      if (!first) {
        event.preventDefault()
        node.focus()
      } else if (event.shiftKey && (document.activeElement === first || document.activeElement === node)) {
        event.preventDefault()
        if (last instanceof HTMLElement) last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        if (first instanceof HTMLElement) first.focus()
      }
    }
  }
  node.addEventListener('keydown', handleKey)
  return {
    destroy() {
      active = false
      node.removeEventListener('keydown', handleKey)
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus()
    },
  }
}
