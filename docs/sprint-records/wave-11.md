# Wave 11 — v0.3.0 export formats (PDF, PNG, SVG)

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.3.0 ("Secondary formats: ... PDF export
with selectable text and embedded fonts, PNG and SVG per-slide export")
Last updated: 2026-07-29

Wave 11 adds per-slide **SVG export**, **PNG export**, and **PDF export**.
The renderer already produces deterministic SVG; SVG export is a thin
wrapper. PNG rasterizes the SVG via the `image` crate (already a workspace
dep). PDF uses the `printpdf` crate to produce a multi-page PDF with
selectable text (text extracted from the model, not rasterized).

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Export crate | `crates/slides-pdf/` | New (was stub): SVG, PNG, PDF export |
| 2 | Desktop | `apps/desktop/` | Export menu/dialog, file save dialogs |

## Explicitly out of this wave

- ODP import/export (separate wave — different format family).
- PDF image-per-page import (rasterize PDF → image shapes).
- PDF form fields, annotations, or interactive elements.
- SVG export with embedded fonts (v0.3.0 fonts wave may add this).

## Component 1 — Export crate (`slides-pdf`, no longer a stub)

Depends on `slides-core` (model), `slides-render` (SVG rendering), `image`
(rasterization, already a workspace dep). Add `printpdf` for PDF generation.

### Public API

```rust
/// Exports a single slide as a standalone SVG document.
/// Deterministic: same input -> identical output.
pub fn export_slide_svg(
    slide: &slides_core::Slide,
    theme: &slides_core::Theme,
    media: &slides_core::MediaStore,
    opts: &slides_render::RenderOptions,
) -> String;

/// Exports a single slide as a PNG image at the given scale.
/// 1x = render at native EMU resolution; 2x = retina.
pub fn export_slide_png(
    slide: &slides_core::Slide,
    theme: &slides_core::Theme,
    media: &slides_core::MediaStore,
    opts: &slides_render::RenderOptions,
    scale: f64,
) -> Result<Vec<u8>>;

/// Exports an entire deck as a multi-page PDF with selectable text.
/// Text from text boxes is placed as actual PDF text objects (not
/// rasterized), so it's selectable and searchable. Images and geometric
/// shapes are rasterized into the PDF page.
pub fn export_deck_pdf(
    deck: &slides_core::Deck,
    opts: &slides_render::RenderOptions,
) -> Result<Vec<u8>>;
```

### SVG export

Trivial: `render_slide` already returns `RenderedSlide { svg, hash }`. The
export function calls it and returns the standalone SVG string. Add a test
asserting the output is valid XML and contains expected content.

### PNG export

Rasterize the rendered SVG. The cleanest approach for a pure-Rust no-system-
deps path: use `resvg`/`usvg`/`tiny-skia` to rasterize the SVG to PNG.
However, those are heavy deps. Alternative: render the SVG via the `image`
crate — but `image` doesn't rasterize SVG. So we need a SVG rasterizer.

Add `resvg` (pure Rust SVG renderer) to the workspace deps. It's the standard
choice for server-side SVG rasterization without a browser. It depends on
`usvg` (SVG parser) and `tiny-skia` (2D rasterizer).

```
resvg = "0.42"
usvg = "0.42"
tiny-skia = "0.11"
```

Rasterize: parse the SVG string with `usvg::Tree::from_str`, render with
`resvg::render` to a `tiny_skia::Pixmap`, encode as PNG.

### PDF export

Use `printpdf` to generate a multi-page PDF:
1. For each slide, create a PDF page at the slide dimensions (convert EMU to
   points: 1 inch = 914400 EMU = 72 pt).
2. Rasterize the slide's non-text shapes (geometric, images, charts, tables)
   as a background image (via the PNG rasterizer above).
3. Place text from text boxes as selectable PDF text objects at their
   positions, using the theme's font sizes.
4. Embed a basic font (printpdf ships with Helvetica).

This produces a PDF where text is selectable and the visual fidelity of
non-text elements is preserved via rasterization.

### Tests

- `export_slide_svg_produces_valid_svg`: output starts with `<svg` and
  contains expected text.
- `export_slide_png_produces_valid_png`: output starts with PNG magic bytes.
- `export_slide_png_deterministic`: same input -> identical bytes.
- `export_deck_pdf_produces_valid_pdf`: output starts with `%PDF` and has
  multiple pages.
- `export_deck_pdf_text_is_selectable`: the PDF contains the text content
  (search for a known string in the raw PDF bytes — PDF text is stored in
  content streams, so check for the font/text operators).
- All exports are deterministic (no timestamps, no random IDs in output).

## Component 2 — Desktop (`apps/desktop/`)

- **Export menu**: File → Export → submenu with SVG, PNG, PDF options.
- **File save dialog**: Tauri's `dialog.save()` to pick the output path.
- **Progress indicator**: for multi-page PDF export on large decks.
- Tauri commands:
  - `export_svg(slide_id) -> Result<String, String>` — returns SVG content.
  - `export_png(slide_id, scale) -> Result<Vec<u8>, String>` — returns bytes.
  - `export_pdf() -> Result<Vec<u8>, String>` — returns full deck PDF bytes.
- The frontend writes the returned bytes to the user-chosen file path via
  Tauri's `fs` plugin.

## Dependency ordering

1. **Export crate** (component 1) — single worktree, merged first.
2. **Desktop** (component 2) — after the crate lands.

## Acceptance criteria

1. A slide exports to standalone SVG with correct content.
2. A slide exports to PNG (valid PNG, deterministic).
3. A deck exports to PDF with selectable text and visual fidelity.
4. All exports are deterministic (same input -> identical output).
5. The export menu offers SVG/PNG/PDF with file save dialogs.
6. Quality gate green. Privacy gate passes. No telemetry.
