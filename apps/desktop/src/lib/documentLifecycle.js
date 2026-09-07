/**
 * Resolve a destructive transition only after editor drafts reach the model.
 * Cancelled saves and rejected flushes never authorize replacement or close.
 * @param {{ flush: () => Promise<void>, isDirty: () => Promise<boolean>, recover: () => Promise<void>, decide: () => Promise<'save'|'discard'|'cancel'>, save: () => Promise<boolean> }} actions
 * @returns {Promise<'continue'|'discard'|'cancel'>}
 */
export async function confirmDocumentTransition(actions) {
  await actions.flush()
  if (!(await actions.isDirty())) return 'continue'
  await actions.recover()
  const choice = await actions.decide()
  if (choice === 'discard') return 'discard'
  if (choice === 'cancel') return 'cancel'
  return (await actions.save()) ? 'continue' : 'cancel'
}

/** @param {string | null | undefined} path */
export function documentFileName(path) {
  return path?.split(/[\\/]/).pop() || 'Untitled presentation'
}
