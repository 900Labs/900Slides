# Wave 5 — v0.2.0 animations and transitions

Status: Proposed
Owner: 900 Labs
Scope target: `PRODUCT_SPEC.md` §5.2 (animations, transitions) and §6.5 (determinism)
Last updated: 2026-07-28

Wave 5 makes **animations and transitions first-class**: per-slide
transitions (none, fade, slide, push, wipe) and per-shape build-in effects
(none, fade, slide-in left/right/top/bottom, appear, disappear) with
**deterministic timing**. The ROADMAP flags animation determinism as a risk
("must be proven with a hash test in CI"), so the `slides-animation` crate
computes a deterministic timeline from the model and the wave ships a hash
test that proves two runs produce identical frame sequences.

## Shape identity decision

Build steps reference shapes by **positional index** (`shape_index` into
`slide.shapes`). Shapes do not yet carry stable IDs. This matches the spec's
constrained v0.2.0 build-in subset and keeps the model change small. Stable
shape IDs are a **v0.3.0 prerequisite** (Magic Move needs them); they are
explicitly out of scope here.

Known limitation: reordering shapes (insert/delete) after setting build-ins
can misalign build steps. The build-step setters validate the index against
the current slide; on undo, the inverse restores both the prior animation
state and the prior shape order. Documented; acceptable for v0.2.0.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | Fill the reserved `Animation`/`Transition` fields; add commands |
| 2 | Animation crate | `crates/slides-animation/` | New (was stub): deterministic timeline computation |
| 3 | Renderer | `crates/slides-render/src/lib.rs` | Emit CSS animation hooks on shape groups for build-ins |
| 4 | Loader | `crates/slides-pptx/src/load.rs` | Parse `p:transition` and simple `p:timing` build-ins |
| 5 | Saver | `crates/slides-pptx/src/save.rs` | Emit `p:transition` / `p:timing` for edited slides |
| 6 | Commands | `crates/slides-core` + `apps/desktop` | Animation/transition commands + presenter playback |

## Explicitly out of this wave

- Stable shape IDs and object-identity tracking (v0.3.0 Magic Move).
- Motion paths, trigger models (click/hover/after-previous/with-previous),
  the Animation Pane (all v0.4.0).
- Animation sub-types beyond the §5.2 list (no spin, no zoom, no 3D).
- Simultaneous/overlapping builds with interleaved timing — the v0.2.0 model
  is a simple ordered sequence (one click reveals the next build step).
- Transitions beyond none/fade/slide/push/wipe (no morph, no cube, no ripple).
- Animations on table cells or chart series (whole-shape build-ins only).

## The shared contract — model changes (component 1, lands first)

The reserved fields already exist on `Slide`:

```rust
pub struct Slide {
    pub id: String,
    pub notes: String,
    pub shapes: Vec<Shape>,
    pub animation: Option<Animation>,   // reserved -> filled
    pub transition: Option<Transition>, // reserved -> filled
}
```

**Filling a reserved `Option<None>` field is additive.** Old decks that never
set these fields deserialize with `None` unchanged (`Option` already
`#[serde(default)]`). `SCHEMA_VERSION` stays `1`. Add a test:
`old_deck_without_animation_deserializes` (round-trips a Wave-4 snapshot).

### Transition model

```rust
/// A transition played when advancing TO this slide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    /// Kind of transition.
    pub kind: TransitionKind,
    /// Duration in milliseconds. Deterministic. Clamped to 0..=5000.
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    None,
    Fade,
    Slide,
    Push,
    Wipe,
}
```

### Animation (build sequence) model

```rust
/// A build-in animation sequence for a slide: an ordered list of steps.
/// Each step reveals one shape with an effect. Steps fire in order (one
/// presenter click per step), per the v0.2.0 constrained model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    /// Ordered build steps. Step 0 fires on the first build click.
    pub steps: Vec<BuildStep>,
}

/// One build-in step targeting a single shape by index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildStep {
    /// Index into `slide.shapes` of the shape this step reveals.
    pub shape_index: usize,
    /// The reveal effect.
    pub effect: BuildEffect,
    /// Duration of the effect in milliseconds. Deterministic. Clamped 0..=3000.
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildEffect {
    Fade,
    SlideInLeft,
    SlideInRight,
    SlideInTop,
    SlideInBottom,
    Appear,
    Disappear,
}
```

### New commands (all reversible)

- `SetTransition { slide_id, transition: Option<Transition> }` — sets or
  clears the slide transition. Inverse snapshots the prior transition
  (`Option`).
- `SetSlideAnimation { slide_id, animation: Option<Animation> }` — replaces
  or clears the entire build sequence. Inverse snapshots the prior animation.
  (The editor sends the full edited step list, like `SetChartData`.)
- `AddBuildStep { slide_id, step: BuildStep }` — appends a step. Inverse is
  `RemoveBuildStepAt { slide_id, index }`.
- `RemoveBuildStepAt { slide_id, index }` — removes a step by position.
  Inverse is `InsertBuildStepAt { slide_id, index, step }`.
- `MoveBuildStep { slide_id, from, to }` — reorders a step. Inverse swaps
  back.

### Validation invariants

- `duration_ms` clamped to the documented ranges on construction
  (`Transition` 0..=5000, `BuildStep` 0..=3000).
- `shape_index` validated against the slide's shape count at apply time
  (in `validate`). A step pointing past the end rejects the command rather
  than panicking.
- `BuildEffect::Disappear` is allowed (it's in the spec list) but note it
  hides a shape that was already visible — the presenter timeline must treat
  it as a hide step, not a reveal.
- No duplicate `shape_index` enforcement: the same shape CAN have multiple
  steps (e.g. appear then disappear). This is intentional and valid.

## Component 2 — Animation crate (`slides-animation`, no longer a stub)

Computes a **deterministic timeline** from the model — the §6.5 guarantee.
Does NOT depend on `slides-render`; `slides-render` depends on it only for
CSS-class names (see component 3). The core deliverable is a pure function
that, given a slide's `Animation`, produces a stable frame sequence.

```rust
/// A single keyframe moment in a build sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildFrame {
    /// Which build step this frame belongs to (index into steps).
    pub step_index: usize,
    /// The shape index this frame affects.
    pub shape_index: usize,
    /// Start time relative to the step's click, in milliseconds.
    pub start_ms: u32,
    /// End time, in milliseconds.
    pub end_ms: u32,
    /// The effect and its progress direction.
    pub effect: slides_core::BuildEffect,
}

/// Computes the deterministic frame sequence for a slide's build animation.
/// Same input -> identical output (stable order, no HashMap iteration).
pub fn build_timeline(animation: &slides_core::Animation) -> Vec<BuildFrame>;

/// The CSS class name for a build effect, used by the renderer (component 3).
/// Deterministic: same effect -> same class name.
pub fn css_class(effect: slides_core::BuildEffect) -> &'static str;

/// The CSS @keyframes definition for a build effect, as a deterministic
/// string the renderer can drop into a <style> block.
pub fn keyframes(effect: slides_core::BuildEffect) -> &'static str;
```

### Determinism rules

- `build_timeline` iterates `steps` in order (Vec, stable). No HashMap.
- Times are derived purely from `duration_ms` (start=0, end=duration per
  step). Two runs with the same `Animation` produce byte-identical
  `Vec<BuildFrame>`.
- Test: `deterministic_timeline_same_input_same_output` — build a timeline
  twice, assert equality. This is the ROADMAP risk-register hash test (here
  expressed as a structural equality; the render hash covers the visual side).

### Effect CSS

Each `BuildEffect` maps to a CSS class + keyframes:
- Fade: `opacity 0 -> 1`.
- SlideInLeft/Right/Top/Bottom: `translate` from off-screen edge to 0.
- Appear: `visibility hidden -> visible` (instant).
- Disappear: `opacity 1 -> 0`.

All keyframes are static `&'static str` constants — deterministic by
construction.

## Component 3 — Renderer (`slides-render`)

Small change to `render_slide`: when a slide has an `Animation`, wrap each
shape that has a build step in a `<g class="build-<step_index>">` group and
emit a `<style>` block with the keyframes for the effects used. This lets
the presenter apply the timeline via CSS without the renderer knowing about
time.

- Import `slides-animation` (sibling path dep, like `slides-chart`).
- The static single-frame SVG for editing is unchanged (the classes are
  inert until the presenter activates them).
- Add a test: a slide with a build step produces a `<style>` block and a
  `class="build-0"` group; a slide without animation produces neither.

## Component 4 — Loader (`slides-pptx/src/load.rs`)

Currently `transition` is hardcoded to `None`. Map the OOXML:

1. `p:transition` (child of `p:sld/p:cSld`, NOT inside `spTree`):
   - `p:fade` -> `Fade`, `p:push` -> `Push`, `p:wipe` -> `Wipe`,
     `p:pull`/slide variants -> `Slide`, absent or `p:cut` -> `None`.
   - `spd` attribute (`slow`/`med`/`fast` or `ms` numeric) -> `duration_ms`
     (slow=1000, med=500, fast=250 default).
2. `p:timing` (child of `p:sld`, NOT inside `spTree`) — parse ONLY the
   simple build-in subset:
   - Walk `p:tnLst`/`p:par`/`p:cTn` for `p:animEffect transition="in"` with
     `filter="fade"` etc., targeting a shape by its `p:cNvPr id`.
   - Resolve the OOXML shape id back to a **model shape index** (the loader
     assigns shapes in document order; map cNvPr id -> index).
   - Unrecognized timing structures: fall back to `None` + loss warning
     (do NOT panic; complex animations are out of scope and stay as raw
     bytes in the slide XML, preserved byte-for-byte).
3. Set `slide.transition` and `slide.animation` from the parsed values.

## Component 5 — Saver (`slides-pptx/src/save.rs`)

`p:transition` and `p:timing` live outside `spTree`, so the existing save
path copies them verbatim (byte-for-byte) when the slide is untouched. For
**edited** slides whose transition/animation changed:

1. If `slide.transition` is `Some` and differs from the original, patch the
   `p:transition` element (or insert one if absent) inside `p:cSld`. Use
   `p:fade`/`p:push`/`p:wipe`/slide elements with a `spd` or `advTm`.
2. If `slide.animation` is `Some` and differs, regenerate the `p:timing`
   tree from the build steps (the simple subset only). Map model shape
   index back to the slide's cNvPr id for targeting.
3. **Lossless guarantee (§4.9):** unedited slides and all non-slide parts
   stay byte-for-byte identical. A slide whose transition/animation was
   NOT changed must round-trip byte-identical (add a test).
4. Clearing transition/animation (`None`) removes the element if the
   original had one.

Session tracking: a slide becomes `dirty` when a transition/animation
command touches it (the command's `affected_slide_ids` already drives this;
the saver already regenerates dirty slides).

## Component 6 — Commands (desktop)

- `set_transition(slide_id, kind, duration_ms)` — sets or clears (`None`)
  the slide transition. Returns snapshot.
- `set_slide_animation(slide_id, steps)` — replaces the full build sequence.
- `add_build_step(slide_id, shape_index, effect, duration_ms)` — append.
- `remove_build_step(slide_id, step_index)` — remove.
- `move_build_step(slide_id, from, to)` — reorder.

Frontend:
- Slide-level **transition picker** (dropdown: none/fade/slide/push/wipe +
  a duration slider) in the slide-thumbnail context menu or a side panel.
- Per-shape **build-in menu**: select a shape, choose an effect
  (none/fade/slide-in directions/appear/disappear). Appends a build step.
- A **build-order list** (mini Animation Pane) showing the ordered steps,
  with reorder (up/down) and remove. This is the lightweight v0.2.0 version
  of the full Animation Pane (which is v0.4.0).
- **Presenter playback:** on each click, advance the build timeline; apply
  the CSS class to reveal the next shape. Transitions play between slides.

## Dependency ordering

1. **Model** (component 1) — single worktree, merged first.
2. **Parallel fan-out:**
   - `slides-animation` crate (needs model)
   - `slides-pptx` loader + saver (needs model)
3. **Renderer** (component 3) — after the animation crate (small).
4. **Desktop** — after all of the above.

## Acceptance criteria

1. A PPTX with a fade transition loads into a `Slide` with
   `transition: Some(Transition{kind: Fade, ..})`, and saving without edits
   produces a byte-for-byte-identical package. Editing the transition and
   saving patches only `p:transition`; everything else unchanged.
2. A PPTX with simple build-in animations loads into a `Slide` with a
   populated `Animation`. Complex/unrecognized timing falls back to
   passthrough + loss warning (no panic).
3. `build_timeline` is deterministic: same input -> identical output (hash/
   equality test). This satisfies the ROADMAP risk-register determinism
   requirement.
4. The renderer emits CSS build-in hooks only when a slide has animation;
   inert for static slides.
5. Every new command round-trips through undo correctly.
6. The presenter advances build steps on click and plays transitions.
7. Quality gate green. Privacy gate passes. No telemetry.

## Test fixtures

- A hand-built PPTX with a fade transition on slide 2.
- A hand-built PPTX with one simple fade build-in on a shape.
- A hand-built PPTX with a complex/unrecognized `p:timing` (falls back).
- Round-trip: load -> edit transition/animation -> save -> assert only the
  target element changed; all other parts byte-identical.
- Timeline determinism for a multi-step animation.
- Every command's apply/undo.
