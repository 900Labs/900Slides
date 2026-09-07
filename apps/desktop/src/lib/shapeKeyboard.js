/**
 * Shape shortcuts apply only when the shape itself owns keyboard focus.
 * Descendant text/table editors must retain native typing, selection and IME.
 * @param {Pick<KeyboardEvent, 'key' | 'target' | 'currentTarget' | 'defaultPrevented' | 'isComposing'>} event
 * @param {boolean} isTextBox
 * @returns {'select' | 'edit' | 'exit' | null}
 */
export function shapeKeyboardAction(event, isTextBox = false) {
  if (event.target !== event.currentTarget || event.defaultPrevented || event.isComposing) return null
  if (event.key === 'Escape') return 'exit'
  if (event.key === 'Enter') return isTextBox ? 'edit' : 'select'
  if (event.key === ' ') return 'select'
  return null
}
