# Wave 20 — v0.4.0 custom layouts + rehearse timings

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.4.0 ("Custom layouts per template" and
"Rehearse timings. Per-slide duration recording.")
Last updated: 2026-07-30

Wave 20 closes out v0.4.0 with two remaining items: user-selectable custom
layouts per slide (building on the Wave 9 template system) and rehearse
timings (per-slide duration recording for self-paced presentations).

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | `Slide.rehearsed_duration_ms` + command |
| 2 | Desktop | `apps/desktop/` | Layout picker UI + rehearse timings recording + playback |

## The shared contract — model changes (component 1)

### Rehearsed timings

Add to `Slide`:

```rust
/// Per-slide rehearsed duration in milliseconds. `None` means no timing
/// recorded. Used by the presenter's auto-advance mode.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub rehearsed_duration_ms: Option<u32>,
```

Additive with `#[serde(default)]`. Old decks load with `None` (no timing).

New command:
- `SetSlideRehearsedDuration { slide_id: String, duration_ms: Option<u32> }`
  — sets or clears the rehearsed duration. Inverse snapshots prior.
  Validate: slide_id must exist.

No other model changes needed — the layout system already exists from Wave 9
(`Deck.layouts: Vec<Layout>`, `Slide.layout_ref: Option<String>`,
`SetSlideLayout` command, `TemplateRegistry` with 6 templates each having
3-4 layouts). This wave is mostly desktop UI.

## Component 2 — Desktop (`apps/desktop/`)

### Layout picker (building on Wave 9)

The `SetSlideLayout` command already exists. Wire a UI:
- In the slide thumbnail context menu (or a side panel), a **layout
  dropdown** showing the current template's available layouts (from
  `deck.layouts`). Each layout name is selectable.
- Selecting a layout calls `set_slide_layout` → the slide's `layout_ref`
  updates and the canvas re-renders with the placeholder guides.
- When the template changes, the layout list updates.

### Rehearse timings

- A **"Rehearse"** button in the presenter (or a menu item) starts timed
  mode: the presenter records the time spent on each slide.
- On each slide advance, the per-slide duration is recorded.
- When rehearse mode ends (Esc or a "Done" button), the timings are
  saved to the deck via `SetSlideRehearsedDuration` (one command per slide,
  or a batch).
- A **"Use timings"** toggle in the presenter enables auto-advance: the
  presenter advances to the next slide automatically after the rehearsed
  duration elapses (instead of waiting for a click).
- The presenter shows the rehearsed duration per slide alongside the live
  timer.

## Dependency ordering

1. **Model** (component 1) — small: one field + one command.
2. **Desktop** (component 2) — both features.

## Acceptance criteria

1. The layout picker shows the current template's layouts and changes the
   slide's layout_ref on selection.
2. Rehearse mode records per-slide durations.
3. Auto-advance uses the rehearsed durations.
4. Old decks (no rehearsed_duration_ms) load unchanged.
5. Quality gate green. Privacy gate passes. No telemetry.
