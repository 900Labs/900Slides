# Wave 14 — v0.3.0 ODP import / export

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.3.0 ("ODP import/export") and
`PRODUCT_SPEC.md` §6.4/§7.3 ("ODP round-trip is a priority").
Last updated: 2026-07-29

Wave 14 fills the `slides-odp` stub: an ODP (OpenDocument Presentation)
reader that opens `.odp` files into the `slides-core` model, and a writer
that exports a deck to `.odp`. ODP-specific structures without a clean model
equivalent fall back to passthrough with a per-slide warning.

## ODP format overview

An ODP file is a ZIP archive with:
- `mimetype` — `application/vnd.oasis.opendocument.presentation`
- `content.xml` — the main document: styles + body (slides)
- `styles.xml` — named styles
- `meta.xml` — metadata
- `Pictures/` — embedded images
- `META-INF/manifest.xml` — file manifest

Key ODF namespaces:
- `office:document` — root element
- `office:body/office:presentation/draw:page` — slides
- `draw:frame` — shapes (text boxes, images, geometric)
- `draw:text-box/text:p/text:span` — text content
- `draw:image` — images
- `draw:rect`, `draw:ellipse`, `draw:line` — geometric shapes
- `style:style`, `style:properties` — styles (colors, fonts, sizes)
- `table:table` — tables
- `draw:page-settings` — slide dimensions

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | ODP crate | `crates/slides-odp/` | New (was stub): reader + writer |

This is a single-crate wave. The reader and writer share the same crate
(mirroring `slides-pptx`'s structure with `load.rs` + `save.rs`).

## Component 1 — ODP crate (`slides-odp`)

Depends on `slides-core` (model), `zip` (already a workspace dep),
`quick-xml` (already a workspace dep). No new external deps needed.

### Public API

```rust
/// Opens an ODP file and converts it to the slides-core Deck model.
/// ODP-specific structures without a clean model equivalent are mapped
/// with a per-slide warning (logged, not panicking).
pub fn load(odp_bytes: &[u8]) -> Result<slides_core::Deck>;

/// Exports a slides-core Deck to an ODP file.
/// Shapes and animations are mapped to the nearest ODP equivalent;
/// lossy mappings produce a warning.
pub fn save(deck: &slides_core::Deck) -> Result<Vec<u8>>;
```

### Reader (`load`)

1. Unzip the archive. Read `content.xml`.
2. Parse `office:document-content/office:body/office:presentation`:
   - Each `draw:page` → a `Slide`. The `draw:name` attribute → slide id.
   - `draw:page-settings` (or `style:page-layout`) → slide dimensions.
3. For each `draw:page`, parse child `draw:frame` elements:
   - `draw:frame` with a `draw:text-box` → `Shape::TextBox`. Parse
     `text:p`/`text:span` for paragraphs and runs (text, bold, italic,
     underline, font size, font family).
   - `draw:frame` with `draw:image` → `Shape::Image`. Resolve the xlink:href
     to the image file in `Pictures/`, store bytes in the MediaStore.
   - `draw:frame` with `draw:rect`/`draw:ellipse`/`draw:line` →
     `Shape::Geometric`. Map geometry and style.
   - Unrecognized elements → `Shape::Passthrough` with raw XML.
4. Parse `style:style` declarations for theme (background, fonts, accent).
5. Map `draw:page-transition` to `Transition` when present.
6. Convert ODP units: 1cm = 360000 EMU. ODP uses cm/mm/in for positioning.

### Writer (`save`)

1. Build a valid ODP archive:
   - `mimetype` (uncompressed, first entry — ODF requirement)
   - `META-INF/manifest.xml`
   - `content.xml` — the deck: styles + body
   - `styles.xml`
   - `meta.xml`
   - `Pictures/` — images from the MediaStore
2. For each slide, emit a `draw:page` with `draw:frame` children:
   - `Shape::TextBox` → `draw:frame/draw:text-box/text:p/text:span`
   - `Shape::Image` → `draw:frame/draw:image` with embedded file
   - `Shape::Geometric` → `draw:rect`/`draw:ellipse`/`draw:line`
   - `Shape::Table` → `table:table`
   - `Shape::Chart` → passthrough (ODP charts are complex; emit a placeholder
     image or passthrough)
   - `Shape::Passthrough` → emit raw XML
3. Theme → `style:style` declarations.
4. Transitions → `draw:page-transition` (when supported by ODP).
5. Animations: ODP has a different animation model — map build-in effects to
   the nearest ODP equivalent with a warning when lossy.

### Units conversion

ODP uses centimeters by default: `1cm = 360000 EMU`. Convert model EMU
values to cm strings for positioning attributes (`svg:x`, `svg:y`,
`svg:width`, `svg:height`).

### Tests

- `load_simple_text_slide`: a hand-built ODP with one text box loads into
  a Deck with a TextBox shape.
- `load_simple_image_slide`: a hand-built ODP with one image loads into a
  Deck with an ImageShape.
- `load_extracts_slide_dimensions`: slide width/height mapped correctly.
- `load_unknown_element_passes_through`: an unrecognized element becomes
  Passthrough (no panic).
- `save_simple_deck_produces_valid_odp`: save a Deck → valid ODP (starts
  with the mimetype, is a valid ZIP).
- `save_load_round_trip`: load → save → load → assert structural equality
  (shape types, text content, positions).
- `save_deterministic`: same Deck → identical bytes.
- `load_empty_deck`: an ODP with zero slides loads into an empty Deck.

Build hand-crafted ODP files in-memory (ZIP + XML strings) for tests — do
NOT download real ODP files.

## Acceptance criteria

1. A simple ODP file with text and images loads into the slides-core model.
2. A deck exports to a valid ODP file (mimetype, valid ZIP, valid XML).
3. Round-trip preserves shape types, text content, and approximate positions.
4. Unrecognized ODP elements fall back to passthrough (no panic).
5. Output is deterministic.
6. Quality gate green. Privacy gate passes. No telemetry.
