// @ts-nocheck

/**
 * Combines a shape's persisted rotation with any presenter build transform.
 * Keeping the rotation on the container lets DOM hit-testing and selection
 * geometry follow the same rendered shape, while the model still owns the
 * canonical rotation value for SVG/PPTX output.
 */
export function composeShapeTransform(rotation, buildTransform) {
  const parts = []
  if (Number.isFinite(rotation) && rotation !== 0) parts.push(`rotate(${rotation}deg)`)
  if (buildTransform && buildTransform !== 'none') parts.push(buildTransform)
  return parts.length > 0 ? parts.join(' ') : undefined
}

/**
 * Rotated line surfaces support direct selection and movement, but their local
 * resize axes need inverse-rotated pointer math. Hide their handles until that
 * interaction exists rather than presenting a resize operation that changes
 * the wrong axis.
 */
export function supportsDirectResize(shapeKind, geometry) {
  return !(shapeKind === 'geometric' && geometry === 'line')
}
