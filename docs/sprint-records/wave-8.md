# Wave 8 — v0.2.0 dual-display presenter

Status: Proposed
Owner: 900 Labs
Scope target: `PRODUCT_SPEC.md` §5.2 ("Dual-display presenter mode (separate
presenter + audience windows), laser pointer and highlighter overlay, and a
black/white slide for Q&A.")
Last updated: 2026-07-29

Wave 8 upgrades the presenter from a single window to a **dual-display**
mode: a presenter window (controls, notes, next-slide preview) and a
separate audience window (fullscreen slide only). It adds a laser pointer,
a highlighter overlay, and a black/white slide for Q&A.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | `Deck.presenter_settings` (additive) |
| 2 | Desktop | `apps/desktop/` | Dual-window presenter, laser, highlighter, B/W |

## Explicitly out of this wave

- Remote control from a second device (phone/tablet). Local dual-display only.
- Presenter notes on a separate display in a different aspect ratio.
- Recording or streaming the presenter.
- Annotation persistence (laser/highlighter are transient, not saved).

## The shared contract — model changes (component 1)

```rust
/// Presenter configuration stored on the deck. Additive with
/// `#[serde(default)]`; old decks get the defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresenterSettings {
    /// Whether the laser pointer is enabled.
    #[serde(default)]
    pub laser_pointer: bool,
    /// Laser pointer color (hex). Defaults to red.
    #[serde(default = "default_laser_color")]
    pub laser_color: String,
    /// Whether the highlighter tool is enabled.
    #[serde(default)]
    pub highlighter: bool,
    /// Highlighter color (hex). Defaults to yellow.
    #[serde(default = "default_highlighter_color")]
    pub highlighter_color: String,
}

impl Default for PresenterSettings { ... }

fn default_laser_color() -> String { "#ff0000".to_string() }
fn default_highlighter_color() -> String { "#ffff00".to_string() }
```

Add to `Deck`:
```rust
#[serde(default)]
pub presenter_settings: PresenterSettings,
```

No new commands needed — presenter settings are runtime preferences, not
deck edits. The frontend reads them from the snapshot; the laser/highlighter
toggles update `presenter_settings` via a `SetPresenterSettings` command
(with a verified inverse, for consistency with the command bus pattern).

### New command

- `SetPresenterSettings { settings: PresenterSettings }` — replaces
  presenter settings. Inverse snapshots prior.

## Component 2 — Desktop (`apps/desktop/`)

### Dual-window architecture

Tauri v2 supports opening a second window. The presenter flow becomes:
1. User clicks **Present** → opens TWO windows:
   - **Presenter window** (existing Presenter.svelte, enhanced): current
     slide, next-slide preview, notes, timer, controls, laser/highlighter/B-W
     toggles.
   - **Audience window** (new AudienceWindow.svelte): fullscreen slide only,
     transitions, build-step reveals. No chrome.
2. The two windows are synchronized via Tauri events: the presenter window
   advances slides/build-steps and emits events; the audience window listens
   and updates. Use the existing `tauri::Emitter` / `tauri::Listener` pattern.

### Laser pointer

When enabled, the presenter window tracks the mouse cursor and draws a
small colored circle overlay. The audience window mirrors the laser
position via events (throttled). The laser is transient — not saved to the
deck.

### Highlighter

When enabled, the presenter can draw freehand strokes over the current
slide. Strokes render as an overlay on both windows (via events). Strokes
clear on slide advance. Transient — not saved.

### Black/white slide

A toggle (key `B` or `W`) that blanks the audience window to solid black or
white for Q&A. The presenter window shows a small indicator. The slide
content is hidden, not lost; toggling back restores it.

### Keyboard navigation

The presenter window handles: arrows/space (advance), B (black), W (white),
L (laser toggle), H (highlighter toggle), Esc (exit). Events flow to the
audience window.

## Acceptance criteria

1. Presenting opens two synchronized windows: presenter (with controls) and
   audience (fullscreen, clean).
2. Advancing in the presenter window updates the audience window in real time.
3. The laser pointer renders on both windows and follows the cursor.
4. The highlighter draws transient strokes on both windows, cleared on advance.
5. B/W keys blank the audience window; content restores on toggle.
6. Presenter settings persist on the deck and deserialize from old decks.
7. Quality gate green. Privacy gate passes. No telemetry.

## Dependency ordering

1. **Model** (component 1) — single worktree, small.
2. **Desktop** (component 2) — after model merges.
