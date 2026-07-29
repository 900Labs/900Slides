# Wave 10 — v0.3.0 Magic Move / Morph

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.3.0 ("Magic Move / Morph equivalent for
identity-matched object morphing between adjacent slides... Object identity
is tracked by stable IDs.")
Last updated: 2026-07-29

Wave 10 adds **Magic Move** — the headline v0.3.0 feature. When a slide
uses the `Morph` transition, the presenter automatically interpolates
position, size, rotation, and opacity for shapes that share a stable ID
with the preceding slide. Non-matching shapes fade in/out. This requires
adding stable shape IDs (deferred from v0.2.0 Wave 5) and a morph timeline
computer.

## Shape identity — the model change

Each editable shape variant gains `id: String` with `#[serde(default)]`.
Old decks deserialize with empty-string ids (no match → no morph, so they
behave exactly as before). New shapes get a UUID. The loader assigns the
OOXML `p:cNvPr` id (it already extracts these — they're used for build-step
resolution and then discarded).

This is a large but mechanical change. The `id` field is added to:
`TextBox`, `ImageShape`, `GeometricShape`, `TableShape`, `ChartShape`.
`PassthroughObject` already has `id`.

Build steps (Wave 5) continue to reference shapes by index — the id field
is additive and doesn't change existing index-based logic. A future wave
may migrate build steps to id-based referencing.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | `id` on every shape variant + `TransitionKind::Morph` |
| 2 | Animation | `crates/slides-animation/` | `morph_timeline` — cross-slide interpolation frames |
| 3 | PPTX | `crates/slides-pptx/` | Assign cNvPr ids on load; emit morph transition on save |
| 4 | Renderer | `crates/slides-render/src/lib.rs` | CSS morph hooks (interpolation keyframes) |
| 5 | Desktop | `apps/desktop/` | Morph in transition picker + presenter playback |

## The shared contract — model changes (component 1)

### Shape id fields

Add to each editable shape variant:

```rust
pub struct TextBox {
    /// Stable identifier for cross-slide matching (Magic Move). Defaults to
    /// empty string so old decks deserialize unchanged.
    #[serde(default)]
    pub id: String,
    // ... existing fields unchanged
}
```

Same for `ImageShape`, `GeometricShape`, `TableShape`, `ChartShape`.

Add a `Shape::id()` accessor method that returns `&str` from any variant
(excluding Passthrough, which has its own `id`).

Add a `Shape::set_id(&mut self, id: String)` mutator.

### TransitionKind::Morph

```rust
pub enum TransitionKind {
    None,
    Fade,
    Slide,
    Push,
    Wipe,
    Morph,  // NEW
}
```

No new command needed — `SetTransition` already takes a `TransitionKind`,
so the user just picks "Morph" from the transition picker (Wave 5 desktop).

### Shape::generate_id() helper

```rust
impl Shape {
    /// Generates a new unique shape id (UUID without hyphens).
    pub fn generate_id() -> String { /* uuid short form */ }
}
```

## Component 2 — Morph timeline (`slides-animation`)

The morph is a transition-level effect, not a per-shape animation. It
compares two adjacent slides and produces interpolation frames for matching
shapes.

```rust
/// A morph frame: one shape's interpolation from the previous slide to the
/// next. Deterministic: same slide pair always produces identical frames.
#[derive(Debug, Clone, PartialEq)]
pub struct MorphFrame {
    /// The shape id being morphed (matches on both slides).
    pub shape_id: String,
    /// Source transform (on the previous slide). None if the shape is new
    /// on the next slide (fade-in).
    pub from: Option<MorphTransform>,
    /// Target transform (on the next slide). None if the shape is being
    /// removed (fade-out).
    pub to: Option<MorphTransform>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MorphTransform {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
}

/// Computes the morph frame sequence for a transition from prev to next.
/// Matches shapes by stable id. Deterministic: iterate shapes in order,
/// no HashMap in output ordering (use BTreeMap or sorted iteration for the
/// matching pass).
pub fn morph_timeline(prev: &Slide, next: &Slide) -> Vec<MorphFrame>;
```

### Matching algorithm

1. Collect `(id, MorphTransform)` from `prev.shapes` and `next.shapes`
   (skip Passthrough and shapes with empty id).
2. For each id present on BOTH slides: emit a `MorphFrame { from: Some,
   to: Some }` (interpolate position/size/rotation).
3. For each id on `next` only: `MorphFrame { from: None, to: Some }`
   (fade-in).
4. For each id on `prev` only: `MorphFrame { from: Some, to: None }`
   (fade-out).
5. Sort by id for deterministic output.

### Determinism

The output order must be stable: sort by shape_id alphabetically. No
HashMap iteration in the output. Test: `morph_timeline_deterministic`.

## Component 3 — PPTX loader/saver

### Loader

The loader already extracts `p:cNvPr` ids into `shape_ids: Vec<String>`.
Currently these are used for build-step animation resolution and then
discarded. Change: assign each extracted id to the corresponding shape's
`id` field (the shapes are in document order, same as `shape_ids`).

### Saver

When emitting shapes on save, use the shape's `id` field as the `p:cNvPr`
id (falling back to the index-based `shape_id_for_index` when empty).

When `slide.transition` is `Some(Transition{kind: Morph, ..})`, emit a
`p:transition` with `p:morph` element (OOXML's morph transition).

## Component 4 — Renderer

When a slide has a Morph transition, emit CSS `@keyframes` and data
attributes on shape groups that carry the from/to transforms for the
interpolation. The presenter applies a CSS transition on the transform
property. Add morph keyframes to `slides-animation::keyframes` or a new
`morph_keyframes` function.

## Component 5 — Desktop

- Transition picker (Wave 5) already exists — add "Morph" to the dropdown.
- Presenter: when the transition to the next slide is Morph, apply the
  `morph_timeline` frames via CSS transitions on shape elements. Each
  morphed shape gets `transition: transform <duration>ms ease` and the
  new transform applied. Fading shapes get opacity transitions.

## Dependency ordering

1. **Model** (component 1) — the `id` field + `TransitionKind::Morph`.
   Single worktree, merged first.
2. **Parallel:** Animation (component 2) + PPTX (component 3).
3. **Renderer** (component 4) — after animation.
4. **Desktop** (component 5) — after all of the above.

## Acceptance criteria

1. Two adjacent slides with a shape that shares an id (same position
   different) and a Morph transition on slide 2 — the presenter interpolates
   the shape's position during the transition.
2. Shapes without matching ids fade in/out.
3. Old decks (empty shape ids) load and render identically — no morph
   occurs because no ids match.
4. The morph timeline is deterministic (same slide pair → identical frames).
5. PPTX round-trip preserves shape ids (cNvPr).
6. Quality gate green. Privacy gate passes. No telemetry.
