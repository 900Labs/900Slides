# Wave 12 — v0.3.0 bundled fonts + stepped code highlighting

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.3.0 ("Bundled open-licensed font set
(one sans, one serif, one mono) embedded in the binary" and "Stepped code
highlighting for code blocks with 1-3|4|5,7 style ranges")
Last updated: 2026-07-29

Wave 12 bundles three open-licensed fonts (sans, serif, mono) into the
binary so decks render identically across platforms, and adds stepped code
highlighting (`1-3|4|5,7` ranges) for code blocks.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Renderer | `crates/slides-render/` | Embed fonts via `@font-face` in SVG; stepped code ranges |
| 2 | Desktop | `apps/desktop/` | Code step editor + highlight preview |

## Explicitly out of this wave

- Font selection UI (the bundled fonts are the defaults; user font choices
  remain as fallbacks).
- Syntax highlighting (tokenization/coloring of code keywords). Stepped
  highlighting = highlighting specific line ranges, not syntax coloring.
- Variable font axes or font subsetting at build time.

## Component 1 — Renderer (`slides-render`)

### Bundled fonts

Bundle three SIL Open Font License (OFL) fonts:
- **Sans**: Inter (or Source Sans 3)
- **Serif**: Source Serif 4 (or DejaVu Serif)
- **Mono**: JetBrains Mono (or Source Code Pro)

These are embedded via `include_bytes!` in the renderer crate and emitted
as base64 `@font-face` declarations in the SVG `<defs>` block. The renderer
maps theme font names to the bundled fonts:
- Any sans-serif family name → bundled sans
- Any serif family name → bundled serif
- Any monospace family name → bundled mono

This ensures the SVG renders identically whether viewed in the desktop app,
a browser, or as a standalone SVG file.

The font files live at `crates/slides-render/fonts/` and are committed to
the repo (they're small: ~300KB each for regular+bold weight subsets).

Font embedding approach:
1. Add font files to `crates/slides-render/fonts/` (`.ttf` format).
2. Embed via `include_bytes!("../fonts/Inter-Regular.ttf")` etc.
3. In `render_slide`, when emitting `<defs>`, add `<style>` with
   `@font-face` rules containing the base64-encoded font data.
4. The CSS font-family names in the SVG match the `@font-face` declarations.

**Determinism concern**: embedding base64 font data in every SVG makes
output large but deterministic. To keep the editor fast, only embed fonts
when exporting (SVG/PDF), not in the live preview. The renderer gains a
`embed_fonts: bool` flag on `RenderOptions`.

### Stepped code highlighting

Code blocks already exist (`paragraph.style.code_block: bool`). Add:
- A new field on `Paragraph`: `code_step_ranges: Option<String>` — the
  step range string (e.g. `"1-3|4|5,7"`). Additive with `#[serde(default)]`.
- The renderer highlights the lines in the current step range with a
  background color (e.g. light yellow) when the range is set.
- A `code_active_step: usize` on `RenderOptions` (or `Slide`) — which step
  is currently active (0-indexed). Lines in the active step's range get
  the highlight; others are dimmed.

Range parsing: `"1-3|4|5,7"` means step 0 = lines 1-3, step 1 = line 4,
step 2 = lines 5 and 7. Parse into `Vec<Vec<RangeInclusive<usize>>>`.

## Component 2 — Desktop (`apps/desktop/`)

- When editing a code block, a small **step editor** lets the user define
  ranges (e.g. typing "1-3|4|5,7" in a text field).
- A **step slider** in the presenter advances through the steps, updating
  the active step and re-rendering the highlight.
- The code block preview shows the current step's highlighted lines.

## New model fields (additive)

- `Paragraph.code_step_ranges: Option<String>` with `#[serde(default)]`.
- `RenderOptions.embed_fonts: bool` (defaults false).
- `RenderOptions.code_active_step: usize` (defaults 0).

## Dependency ordering

1. **Renderer** (component 1) — fonts + code ranges. Single change.
2. **Desktop** (component 2) — code step editor + presenter.

## Acceptance criteria

1. A slide rendered with `embed_fonts: true` contains `@font-face`
   declarations with base64 font data.
2. A slide rendered with `embed_fonts: false` (the default) has no embedded
   font data — identical to current behavior.
3. A code block with `code_step_ranges: Some("1-3")` and
   `code_active_step: 0` highlights lines 1-3.
4. Old decks (no `code_step_ranges`) render identically.
5. Quality gate green. Privacy gate passes. No telemetry.
