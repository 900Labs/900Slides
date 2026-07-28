# Wave 7 — v0.2.0 deck structure & editor completeness

Status: Proposed
Owner: 900 Labs
Scope target: `PRODUCT_SPEC.md` §5.2 (aspect ratios, slide sections, rich-text
speaker notes, find & replace, shortcuts dialog, high-contrast theme)
Last updated: 2026-07-28

Wave 7 completes the remaining v0.2.0 editor-completeness items that do not
require a dedicated stub crate. All changes are **additive** with no schema
break (new fields use `#[serde(default)]`). This wave has two components:
the model extensions and the desktop wiring.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | Aspect ratio, slide sections, rich-text notes |
| 2 | Desktop | `apps/desktop/` | All six features: picker, sections panel, notes editor, find/replace, shortcuts dialog, high-contrast theme |

## Feature specs

### Aspect ratios (4:3, 16:10, custom)

Add `Deck.slide_size: Option<SlideSize>` with `#[serde(default)]`. When `None`,
the deck defaults to the current 16:9 (12,192,000 × 6,858,000 EMU). Types:

```rust
pub struct SlideSize {
    pub width_emu: f64,
    pub height_emu: f64,
}
impl SlideSize {
    pub fn widescreen_16_9() -> Self;  // 12,192,000 x 6,858,000
    pub fn standard_4_3() -> Self;     // 9,144,000 x 6,858,000
    pub fn widescreen_16_10() -> Self; // 12,149,333 x 7,593,333
}
```

The renderer's `RenderOptions` already takes `width_emu`/`height_emu` — the
desktop resolves them from `deck.slide_size`. The PPTX loader reads
`p:sldSz` (cy/cx attributes) on load; the saver emits it.

### Slide sections

Add `Deck.sections: Vec<SlideSection>` with `#[serde(default)]`:

```rust
pub struct SlideSection {
    pub name: String,
    /// Slide id that starts this section.
    pub start_slide_id: String,
}
```

Sections are an ordered list; the desktop shows a collapsible section list
in the thumbnail sidebar. The PPTX format stores sections in
`ppt/presentation.xml` under `p:extLst` — carry as passthrough when
unrecognized, parse when simple.

### Rich-text speaker notes

Currently `Slide.notes: String`. Add `Slide.rich_notes: Option<Vec<Paragraph>>`
with `#[serde(default)]`. When `Some`, the editor renders rich-text notes
(reusing the existing `Paragraph`/`Run` model). When `None`, the plain
`notes: String` is used. A `SetRichNotes` command sets/clears the field
(inverse snapshots prior).

### Find and replace

Desktop-only — no model change. A dialog that searches across all slides'
text and replaces matches. Operates through the command bus (uses
`EditTextBox` or a new batch command). Keyboard: Cmd/Ctrl+F to open.

### Shortcuts dialog

Desktop-only — a modal listing keyboard shortcuts. Static content, no model
change.

### High-contrast theme

Add `Theme.high_contrast: bool` with `#[serde(default)]`. When true, the
renderer uses a high-contrast color palette (black/white/yellow). A
`SetHighContrast` command toggles it (inverse snapshots prior).

## New commands

- `SetSlideSize { slide_size: Option<SlideSize> }` — sets the deck's aspect
  ratio. Inverse snapshots prior.
- `SetRichNotes { slide_id, rich_notes: Option<Vec<Paragraph>> }` — sets
  rich-text notes. Inverse snapshots prior.
- `SetHighContrast { high_contrast: bool }` — toggles the theme.
  Inverse snapshots prior.
- (Slide sections are managed via existing slide reordering + a
  `SetSections { sections: Vec<SlideSection> }` command.)

## Dependency ordering

1. **Model** (component 1) — all additive fields + commands, single change.
2. **Desktop** (component 2) — all six features wired up.

## Acceptance criteria

1. A deck with `slide_size: Some(standard_4_3())` renders at 4:3; old decks
   without the field default to 16:9 and deserialize unchanged.
2. Slide sections render in a collapsible list and round-trip.
3. Rich-text notes render and edit; plain notes still work.
4. Find & replace works across slides through the command bus.
5. The shortcuts dialog opens and lists shortcuts.
6. High-contrast theme renders a distinct palette.
7. Quality gate green. Privacy gate passes. No telemetry.
