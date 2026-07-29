# Wave 13 — v0.3.0 projector CSS filter panel

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.3.0 ("Projector CSS filter panel in
presenter mode: invert, brightness, contrast, saturation, sepia, hue-rotate,
persisted per device.")
Last updated: 2026-07-29

A small, self-contained wave. The presenter gains a CSS filter panel for
projector compensation: invert, brightness, contrast, saturation, sepia,
and hue-rotate sliders applied to the audience window. Settings persist to
the app data directory (per-device, not per-deck).

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | `ProjectorFilters` on `PresenterSettings` |
| 2 | Desktop | `apps/desktop/` | Filter panel UI + CSS application + persistence |

## Model changes (additive)

Add to `PresenterSettings`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectorFilters {
    #[serde(default)]
    pub invert: bool,
    #[serde(default = "default_brightness")]
    pub brightness: f64,   // 0.0..=2.0, default 1.0
    #[serde(default = "default_contrast")]
    pub contrast: f64,     // 0.0..=2.0, default 1.0
    #[serde(default = "default_saturation")]
    pub saturation: f64,   // 0.0..=2.0, default 1.0
    #[serde(default)]
    pub sepia: f64,        // 0.0..=1.0, default 0.0
    #[serde(default)]
    pub hue_rotate: f64,   // 0.0..=360.0 degrees, default 0.0
}
```

Add `pub projector_filters: ProjectorFilters` to `PresenterSettings`
(`#[serde(default)]`).

No new commands — the existing `SetPresenterSettings` command (Wave 8)
already replaces the entire settings struct.

## Desktop

- A **filter panel** in the presenter window (toggle button → popover with
  sliders for brightness, contrast, saturation, sepia, hue-rotate + an
  invert checkbox + a reset button).
- The CSS `filter` property is applied to the audience window's slide
  container element: `filter: invert(1) brightness(1.2) contrast(1.1) ...`.
- Filters persist via `SetPresenterSettings` (stored on the deck's
  `presenter_settings.projector_filters`).
- The audience window applies the filter in real time via Tauri events.

## Acceptance criteria

1. The presenter has a filter panel with all 6 controls.
2. Adjusting filters applies CSS to the audience window in real time.
3. Filters persist on the deck via PresenterSettings.
4. Old decks (no projector_filters) default to neutral (no filtering).
5. Quality gate green. Privacy gate passes. No telemetry.
