# Wave 19 — v0.4.0 animation enhancements

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.4.0 (motion paths, trigger model,
per-slide reduce-motion override, Animation Pane)
Last updated: 2026-07-30

Wave 19 extends the animation system with triggers, delays, motion paths,
a per-slide reduce-motion override, and a dedicated Animation Pane in the
desktop UI. This is the largest remaining v0.4.0 chunk.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | Trigger + delay + motion path fields on BuildStep; reduce-motion on Slide |
| 2 | Animation | `crates/slides-animation/src/lib.rs` | Motion path timeline; trigger-aware timeline |
| 3 | Desktop | `apps/desktop/` | Animation Pane UI + trigger/delay controls + motion path editor |

## The shared contract — model changes (component 1)

All additive with `#[serde(default)]`. Old decks unaffected.

### BuildStep extensions

```rust
pub struct BuildStep {
    pub shape_index: usize,
    pub effect: BuildEffect,
    pub duration_ms: u32,
    // NEW:
    /// When this step fires. Defaults to OnClick.
    #[serde(default)]
    pub trigger: Trigger,
    /// Delay before the effect starts, in ms (after the trigger fires).
    #[serde(default)]
    pub delay_ms: u32,
    /// Optional motion path (waypoints in EMU). Only for BuildEffect::MotionPath.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_path: Option<Vec<Rect>>,
}

/// When a build step fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    #[default]
    OnClick,
    WithPrevious,
    AfterPrevious,
}
```

### New BuildEffect variant

Add `MotionPath` to `BuildEffect`. When this effect is used, the step's
`motion_path` field provides the waypoints.

### Per-slide reduce-motion override

```rust
// On Slide:
#[serde(default, skip_serializing_if = "Option::is_none")]
pub reduce_motion: Option<bool>,
```

When `Some(true)`, the presenter renders all build-ins instantly (no
animation), overriding any system-level preference for this slide only.
When `None`, the system preference is used.

### New commands

- `SetBuildStepTrigger { slide_id, step_index, trigger }` — sets the trigger.
  Inverse snapshots prior.
- `SetBuildStepDelay { slide_id, step_index, delay_ms }` — sets the delay.
  Inverse snapshots prior.
- `SetBuildStepMotionPath { slide_id, step_index, path: Option<Vec<Rect>> }`
  — sets or clears a motion path. Inverse snapshots prior.
- `SetSlideReduceMotion { slide_id, reduce_motion: Option<bool> }` — sets or
  clears the per-slide override. Inverse snapshots prior.

## Component 2 — Animation crate (`slides-animation`)

### Trigger-aware timeline

`build_timeline` currently assumes all steps fire on separate clicks. With
triggers:
- `OnClick`: starts a new click group (as before).
- `WithPrevious`: fires simultaneously with the previous step.
- `AfterPrevious`: fires immediately after the previous step's duration
  elapses (no click needed).

The timeline output gains a `click_group` field (which click triggers this
step) and an `offset_ms` field (delay after the click or after the previous
step).

### Motion path timeline

When a step's effect is `MotionPath`, the timeline produces interpolation
frames along the waypoints (linear interpolation between each pair of
waypoints, subdivided by the duration).

### Reduce-motion

When reduce-motion is active, `build_timeline` returns instant frames
(duration_ms = 0) so the presenter shows shapes immediately without
animation.

## Component 3 — Desktop (`apps/desktop/`)

### Animation Pane (`AnimationPane.svelte`)

A dedicated panel (replaces or extends the current build-order list):
- Ordered list of all build steps with: shape thumbnail/label, effect name,
  trigger dropdown (On Click / With Previous / After Previous), duration
  slider, delay slider.
- Drag-to-reorder steps.
- Per-step: edit motion path (a small canvas to draw waypoints).
- Reduce-motion toggle per slide.

### Motion path editor

When a step's effect is MotionPath, a small overlay canvas lets the user
click to add waypoints. The waypoints are stored as EMU coordinates relative
to the shape's position.

## Dependency ordering

1. **Model** (component 1) — additive fields + 4 commands.
2. **Animation crate** (component 2) — trigger-aware timeline.
3. **Desktop** (component 3) — Animation Pane + motion path editor.

## Acceptance criteria

1. A step with `WithPrevious` fires simultaneously with the previous step.
2. A step with `AfterPrevious` fires immediately after the previous step
   completes (no click).
3. A MotionPath step interpolates along its waypoints.
4. Reduce-motion renders all steps instantly.
5. The Animation Pane shows all steps with trigger/delay/duration controls.
6. Old decks (no trigger/delay/motion_path fields) behave as before (all
   OnClick, no delay, no motion path).
7. Quality gate green. Privacy gate passes. No telemetry.
