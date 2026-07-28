# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

This section tracks work toward v0.2.0 (editor completeness). See
[`docs/ROADMAP.md`](docs/ROADMAP.md),
[`docs/sprint-records/wave-1.md`](docs/sprint-records/wave-1.md),
[`docs/sprint-records/wave-2.md`](docs/sprint-records/wave-2.md),
[`docs/sprint-records/wave-3.md`](docs/sprint-records/wave-3.md), and
[`docs/sprint-records/wave-4.md`](docs/sprint-records/wave-4.md).

### Added — Wave 4 (charts)

- Charts are now first-class in `slides-core`: `Shape` gains a `Chart`
  variant with `ChartShape`, `ChartType` (bar, column, line, area, pie,
  scatter), `ChartData` (category-based or XY), `CategorySeries`, `XYSeries`,
  and `XYPoint` types. Data is capped at 100 categories, 20 series, and
  1000 points per series (`PRODUCT_SPEC.md` §5.2), with structural invariants
  enforced (non-empty series, value/category alignment, and a data kind that
  matches the chart type). All additive; old decks deserialize unchanged.
- Four reversible commands: `AddChart`, `SetChartType`, `SetChartData`, and
  `SetChartTitle`. Each produces a verified inverse; deleting a chart and
  undoing restores it.
- New `slides-chart` crate (no longer a stub) renders any `ChartShape` to a
  deterministic SVG via `render_chart_svg`: bars/columns as grouped
  rectangles, lines as polylines, areas as filled polygons, pie wedges, and
  scatter points, with axes, tick labels, grid lines, an optional legend, and
  a title. `slides-render` delegates chart shapes to this crate.
- PPTX loader resolves chart `p:graphicFrame` relationships to their
  `ppt/charts/chartN.xml` parts and maps `c:chartSpace` to `ChartShape`
  (type from the plot-area chart element and `barDir`; series from `c:ser`).
  The original part path is retained for lossless save; unrecognized or
  malformed charts fall back to passthrough with a loss warning. The saver
  patches only the data sections of dirty chart parts (values, category and
  series names, title) and writes fresh parts for newly inserted charts;
  every unedited part stays byte-for-byte identical.
- Four new Tauri commands (`add_chart`, `set_chart_type`, `set_chart_data`,
  `set_chart_title`) wired through the transactional command bus. The Svelte
  canvas renders charts through the deterministic slide SVG, and a data-table
  editor (double-click a chart) edits categories, series, and values.
- `docs/sprint-records/wave-4.md` documenting the wave scope and acceptance
  criteria.

### Added — Wave 3 (tables)

- Tables are now first-class in `slides-core`: `Shape` gains a `Table`
  variant with `TableShape`, `TableRow`, `TableCell`, `TableBorders`,
  `BorderEdge`, and `CellAlign` types. Tables cap at 50 rows × 50 columns
  (`PRODUCT_SPEC.md` §5.2) with structural invariants enforced (non-empty,
  uniform column count, matched `column_widths`). All additive; old decks
  deserialize unchanged.
- Eight reversible commands: `AddTable`, `SetCellText`, `SetCellStyle`
  (double-Option merge-patch over fill/borders/align), `ResizeTable`,
  `InsertRow`, `InsertColumn`, `DeleteRow`, `DeleteColumn`. Each produces a
  verified inverse; deleting a whole table and undoing restores every cell.
- `slides-render` renders tables as a deterministic SVG grid: cell
  backgrounds, border edges (with dash styles), header row (bold + distinct
  fill), cell alignment (left/center/right), vertical centering, and rotation.
  Empty tables render a frame rect without panicking.
- PPTX loader maps `p:graphicFrame`/`a:tbl` to `TableShape`, parsing
  `tblGrid` (column widths), `tr`/`tc` (rows/cells), `tcPr` (fills, borders
  via `lnL`/`lnR`/`lnT`/`lnB`, alignment), and `tblPr` (header row). Rich
  text inside cells collapses to plain text with a loss warning. Tables
  exceeding the 50×50 cap clamp and warn. Graphic frames without `a:tbl`
  (charts, SmartArt) stay passthrough. The saver emits a full
  `p:graphicFrame`/`a:tbl` for dirty slides only; untouched parts stay
  byte-for-byte identical.
- Seven new Tauri commands (`add_table`, `set_cell_text`, `set_cell_style`,
  `insert_row`, `insert_column`, `delete_row`, `delete_column`) wired through
  the transactional command bus. The Svelte canvas renders tables with cell
  backgrounds, borders, alignment, and editable cell text (click to edit,
  blur to commit). The toolbar has a table button with a size-picker grid
  and insert/delete row/column buttons.
- `docs/sprint-records/wave-3.md` documenting the wave scope and acceptance
  criteria.

### Added — Wave 2 (rich text)

- Rich-text runs in `slides-core`: `Run` gains `strikethrough`,
  `vertical_align` (baseline / superscript / subscript), `link`, `code`
  (inline monospace), and `font_family`. `Paragraph` gains a `ParagraphStyle`
  block carrying `heading` (H1-H6), `blockquote`, `code_block`, and
  `indent_level`. All additive with `#[serde(default)]`; v0.1.0 / Wave 1
  decks still load unchanged.
- `Link::new` validates the URL against a scheme allowlist (mailto, tel,
  fragment, relative paths) and rejects `javascript:`, `vbscript:`, `mocha:`,
  `livescript:`, `http:`, `https:`, `file:`, `data:`, any unknown scheme, and
  control characters. `Link::new_unchecked` preserves an existing link for
  the loader (used with a loss warning when a target fails the allowlist).
- Two new reversible commands: `SetRunStyle` (merge-patch over a single run's
  style flags) and `SetParagraphStyle` (replace a paragraph's style block).
  Each produces a verified inverse.
- `slides-render` renders every new property to deterministic SVG:
  strikethrough (combined with underline), super/subscript (`baseline-shift`
  tspan), links (`<a href>` with escaped URL), inline code (monospace),
  headings (scaled font sizes), blockquote (vertical bar + italic), code
  blocks (monospace + background rect), and indent levels.
- PPTX loader maps OOXML run/paragraph properties onto the new model
  (`strike`, `baseline` for super/sub, `a:hlinkClick` for links, `a:latin`
  typeface for code/fonts, `pStyle` for headings, `lvl` for indent) and the
  saver emits them. Hyperlinks reuse the existing relationship infrastructure
  with `External` target mode. Disallowed link schemes are preserved
  unchecked with a per-slide loss warning (transparent fidelity). The
  lossless-passthrough guarantee is unchanged (untouched parts stay
  byte-for-byte identical).
- Two new Tauri commands: `set_run_style` and `set_paragraph_style`, wired
  through the transactional command bus. The Svelte canvas renders all new
  rich-text properties via CSS classes; the toolbar exposes strikethrough,
  superscript, subscript, inline-code toggles, a heading dropdown (H1-H6),
  and blockquote / code-block paragraph toggles.
- `docs/sprint-records/wave-2.md` documenting the wave scope, the shared
  model contract, component specs, and acceptance criteria.

### Added — Wave 1 (images and geometric shapes, first-class)

- Deck model in `slides-core`: images and geometric shapes are now first-class.
  `Shape` gains `Image` and `Geometric` variants; new types `Transform`,
  `Geometry` (rectangle, rounded rectangle, ellipse, triangle, line, arrow,
  right-arrow callout, five-point star), `Style`, `Fill`, `Outline`, `Shadow`,
  `Crop`, and a key-addressed `MediaStore` on the deck. All additions are
  additive with no schema break: v0.1.0 decks and recovery snapshots still
  load (the `media` field defaults to empty when absent).
- Five reversible commands: `AddShape`, `DeleteShape`, `MoveShape`,
  `SetShapeStyle`, `InsertImage`. Each produces a verified inverse; deleting
  the sole reference to an image also removes its media entry, and undo
  restores it.
- `slides-media` image ingest boundary (no longer a stub): sniffs MIME by
  magic bytes (PNG/JPEG/GIF/WebP/SVG), enforces a size cap and a maximum
  dimension, strips EXIF and other metadata from rasters by default, keeps
  only the first frame of multi-frame formats, and sanitizes SVG against
  `<script>` elements (including namespaced ones), event-handler attributes,
  and unsafe URL references (`javascript:`, `vbscript:`, `http(s):`, `file:`,
  `data:`). Dimension caps are checked from the image header before full
  decode.
- `slides-render` deterministic SVG renderer (no longer a stub): renders text
  boxes, geometric shapes, images, and passthrough placeholders to a single
  `<svg>` document, with a stable xxHash64 content hash. SVG user units are
  EMU directly. Missing image references render a labelled placeholder instead
  of panicking.
- PPTX loader now maps `p:pic` to editable `Image` shapes (media ingested
  through `slides-media` for EXIF stripping, MIME allowlist, and content-
  addressed deduplication) and `p:sp` with a recognized `a:prstGeom` to
  `Geometric` shapes. Fill, outline, shadow, rotation, and crop round-trip
  through load and save. Unrecognized presets and disallowed images fall back
  to opaque passthrough with a per-slide loss warning.
- PPTX saver regenerates only dirty slides; all untouched parts stay
  byte-for-byte identical. Deleting a shape is persisted: when the model has
  fewer shapes than the original slide XML, the whole shape tree is
  regenerated from the model so deleted shapes are removed and remaining
  shapes stay aligned.
- Six new Tauri commands: `insert_image`, `add_shape`,
  `update_shape_transform`, `update_shape_style`, `delete_shape`, and
  `render_slide_svg`. The deck snapshot carries a base64 media store so the
  frontend can render images directly; the snapshot's media DTO is cached by a
  content fingerprint so non-media commands do not re-encode every image on
  each keystroke.
- Svelte canvas renders images (base64 data URI, with crop) and geometric
  shapes (per-geometry SVG) alongside the existing editable text boxes.
  Thumbnail panel and presenter use the renderer's SVG output.
- `docs/sprint-records/wave-1.md` documenting the wave scope, the shared model
  contract, component specs, and acceptance criteria.

### Fixed — Wave 1 review pass

- SVG sanitizer no longer accepts `javascript:` URIs (and any scheme, not just
  `://`-bearing ones) or namespaced `<prefix:script>` elements.
- Image dimensions are read from the header before full decode, preventing a
  memory-exhaustion vector via a malicious huge-dimension image.
- `DeleteShape` is now persisted on save (previously left deleted shapes in the
  file and misaligned the following shapes).
- `insert_image` uses the same content-addressed media key as the loader, so
  duplicate images deduplicate instead of producing duplicate package parts.
- A text-bearing shape of any geometry is modeled as a text box so its text
  stays editable.
- `serialized_size` for image commands accounts media bytes directly instead
  of JSON-serializing `Vec<u8>` as a ~4x-larger integer array.
- The saver surfaces an explicit error instead of silently dropping an image
  whose MIME type is not in the allowlist (which previously produced a
  `<a:blip>` pointing at a part that was never written).

## [0.1.0] - 2026-07-24

### Added

- Deck model in `slides-core`: Deck, Slide, Shape (TextBox | Passthrough),
  Theme, Color, Rect (EMU geometry), Paragraph, Run, ListStyle.
- Transactional command bus with bounded undo (100 transactions, 64 MiB
  aggregate, 32 MiB per transaction). Each command produces a verified
  inverse.
- `EditText` command with correct inverse and dirty-slide tracking.
- PPTX load boundary in `slides-pptx`: parses `[Content_Types].xml`,
  relationships, `presentation.xml`, slide XML, and theme. Maps text boxes to
  editable shapes; everything else to opaque passthrough objects.
- PPTX save boundary: regenerates only edited slides; copies all other parts
  byte-for-byte from the original package. Round-trip test asserts
  byte-identity of untouched parts.
- Loss ledger: per-slide warnings for content that is preserved but not
  editable.
- 900Slides custom XML manifest under `/customXml/` with content-type and
  relationship wiring so PowerPoint opens authored decks without repair
  warnings.
- Tauri v2 desktop shell with 14 commands: new, open, save, snapshot, edit
  text, undo, loss ledger, presenter start/state/next/previous, and recovery
  list/restore/discard.
- Svelte 5 editor: slide thumbnails, slide canvas with editable text boxes
  and passthrough placeholders, notes panel, toolbar (New/Open/Save/Undo).
- Fullscreen presenter mode: current slide, next-slide preview, slide
  counter, elapsed timer, speaker notes, keyboard navigation.
- Recovery: debounced (750 ms) autosave snapshots after each edit, startup
  recovery prompt with restore/discard/skip.
- Quality gate: `scripts/verify-local.sh` (fmt, clippy, test, svelte-check).
- Workspace of 11 library crates, Apache-2.0 license, rust-toolchain pin.

### Known limitations

- No image, shape, table, or chart editing (preserved on save, shown as
  placeholders in the editor).
- No animations or transitions.
- No PDF, PNG, SVG, or ODP export.
- No spell-check.
- 16:9 aspect ratio only.
- No signed or notarized installer for any platform.
