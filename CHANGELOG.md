# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

This section tracks work toward v0.2.0 (editor completeness). See
[`docs/ROADMAP.md`](docs/ROADMAP.md),
[`docs/sprint-records/wave-1.md`](docs/sprint-records/wave-1.md),
[`docs/sprint-records/wave-2.md`](docs/sprint-records/wave-2.md),
[`docs/sprint-records/wave-3.md`](docs/sprint-records/wave-3.md),
[`docs/sprint-records/wave-4.md`](docs/sprint-records/wave-4.md), and
[`docs/sprint-records/wave-5.md`](docs/sprint-records/wave-5.md), and
[`docs/sprint-records/wave-6.md`](docs/sprint-records/wave-6.md), and
[`docs/sprint-records/wave-7.md`](docs/sprint-records/wave-7.md), and
[`docs/sprint-records/wave-8.md`](docs/sprint-records/wave-8.md), and
[`docs/sprint-records/wave-9.md`](docs/sprint-records/wave-9.md),
[`docs/sprint-records/wave-10.md`](docs/sprint-records/wave-10.md), and
[`docs/sprint-records/wave-11.md`](docs/sprint-records/wave-11.md), and
[`docs/sprint-records/wave-12.md`](docs/sprint-records/wave-12.md), and
[`docs/sprint-records/wave-13.md`](docs/sprint-records/wave-13.md), and
[`docs/sprint-records/wave-14.md`](docs/sprint-records/wave-14.md).

## [v0.4.0 — in progress]

### Added — Wave 17 (local comments)

- **Local comments**: threaded annotations anchored to slides, specific
  shapes, or text ranges within a text box. Support reply, resolve, and
  assignment. All stored locally.
- New model types: `Comment`, `CommentThread`, `CommentAnchor` (slide /
  shape / text-range variants). Additive `Deck.comments` field.
- Five reversible commands: `AddComment`, `ReplyToComment`,
  `SetCommentResolved`, `AssignComment`, `DeleteCommentThread`.
- Comments persist in the 900Slides custom XML manifest and survive PPTX
  round-trip. The CDATA section is split on `]]>` to prevent data
  corruption from comment bodies containing that sequence.
- Desktop **comments sidebar** (toggle `C`): lists threads grouped by slide,
  reply, resolve, assign, delete. Context menu: "Add comment" on shapes,
  "Comment on selection" on text ranges.
- `docs/sprint-records/wave-17.md` documenting the wave scope.

### Added — Wave 16 (local version history)

- Every save now creates a **content-addressed snapshot** (SHA-256 of the
  serialized deck). Identical saves are deduplicated. Snapshots are stored
  locally under the app data directory — no cloud, no sync.
- The **Version History** panel (File → Version History) lists all snapshots
  chronologically with timestamps and optional names.
- **Restore** a version — replaces the deck, reversible via undo.
- **Name** a version (e.g. "Before client review").
- **Diff** two versions — shows structural changes (slides/sections/shapes
  added, removed, modified).
- `docs/sprint-records/wave-16.md` documenting the wave scope.

## [0.3.0] — 2026-07-29

### Added — Wave 14 (ODP import / export)

- The `slides-odp` crate (no longer a stub) supports **opening `.odp`
  files** (OpenDocument Presentation) into the `slides-core` model and
  **exporting decks to `.odp`**. ODP is a ZIP+XML format using ODF
  namespaces (`draw:page`, `draw:frame`, `text:p`).
- **Reader** (`load`): unzips the archive, parses `content.xml`, maps
  `draw:frame` elements to model shapes (TextBox, Image, Geometric, Table).
  Converts ODP units (cm) to EMU. Unrecognized elements fall back to
  Passthrough (no panic).
- **Writer** (`save`): builds a valid ODP archive with `mimetype`,
  `content.xml`, `META-INF/manifest.xml`, `styles.xml`, `meta.xml`, and
  `Pictures/`. Maps model shapes to `draw:frame` elements. Deterministic
  output (fixed metadata date, stable ZIP entry order).
- Round-trip test (save → load → structural equality) verifies text, image,
  geometric, and table shapes survive the conversion.
- 17 unit tests (8 reader, 9 writer) including determinism and passthrough
  safety.
- `docs/sprint-records/wave-14.md` documenting the wave scope.

### Added — Wave 13 (projector CSS filter panel)

- The presenter gains a **projector compensation filter panel**: invert,
  brightness, contrast, saturation, sepia, and hue-rotate controls applied
  to the audience window via CSS `filter`. A toggle button in the presenter
  toolbar opens a popover with sliders + a reset button.
- New `ProjectorFilters` type on `PresenterSettings` (additive,
  `#[serde(default)]`) persists the settings on the deck. Slider changes
  are debounced to coalesce into a single `SetPresenterSettings` call.
- The CSS filter string only includes non-default properties (e.g.
  `brightness(1)` is omitted) for clean output.
- `docs/sprint-records/wave-13.md` documenting the wave scope.

### Added — Wave 12 (bundled fonts + stepped code highlighting)

- **Bundled open-licensed fonts**: Inter (sans), Source Serif 4 (serif),
  and JetBrains Mono (mono) are embedded in the binary via
  `include_bytes!`. When exporting (SVG, PNG, PDF), the fonts are embedded
  as base64 `@font-face` declarations so files render identically on any
  platform. The live editor preview keeps fonts unembedded for performance.
  Font aliases map common names (Calibri, Helvetica, Georgia, Courier New,
  etc.) to the bundled equivalents.
- **Stepped code highlighting**: code blocks support `1-3|4|5,7` style step
  ranges. Each pipe-separated segment is one click step; the active step's
  lines are highlighted and others dimmed. `ParagraphStyle.code_step_ranges`
  stores the range string (additive, `#[serde(default)]`). The presenter
  advances through code steps on click.
- Desktop: code step range editor in the toolbar (for code-block
  paragraphs), presenter code-step slider, and font-enabled export.
- `docs/sprint-records/wave-12.md` documenting the wave scope and acceptance
  criteria.

### Added — Wave 11 (export formats)

- Decks can now be exported to **SVG**, **PNG**, and **PDF**. A new
  `slides-pdf` crate (no longer a stub) provides:
  - **SVG export**: per-slide standalone SVG (thin wrapper over the renderer).
  - **PNG export**: rasterizes the rendered SVG via `resvg`/`usvg`/`tiny-skia`
    (pure-Rust SVG renderer, no system deps). Configurable scale (1x/2x).
  - **PDF export**: multi-page deck PDF. Each slide is rasterized at retina
    resolution and embedded as a full-page image. All exports are
    deterministic — identical inputs yield byte-identical output.
- Desktop **Export** menu (File → Export) with SVG (current slide), PNG
  (current slide, 2x), and PDF (entire deck) options. Uses the system save
  dialog with file-type filters. Shows a busy indicator and surfaces errors.
- `docs/sprint-records/wave-11.md` documenting the wave scope and acceptance
  criteria.

### Added — Wave 10 (Magic Move / Morph)

- **Magic Move**: when a slide uses the Morph transition, the presenter
  automatically interpolates position, size, rotation, and opacity for
  shapes that share a stable id with the preceding slide. Non-matching
  shapes fade in/out. This is the v0.3.0 headline feature.
- Every editable shape variant (`TextBox`, `ImageShape`, `GeometricShape`,
  `TableShape`, `ChartShape`) now carries a stable `id: String`
  (`#[serde(default)]` — old decks deserialize with empty ids and behave
  identically). The PPTX loader assigns the OOXML `p:cNvPr` id; the saver
  emits it back. Shape ids round-trip through PPTX.
- `TransitionKind::Morph` variant added.
- New `slides-animation::morph_timeline` computes a deterministic cross-slide
  interpolation frame sequence (matched by id, sorted alphabetically for
  stable output). Three frame types: interpolate (from+to), fade-in (to
  only), fade-out (from only).
- The renderer tags shapes with `data-shape-id` attributes for presenter
  matching.
- PPTX saver emits `<p:morph>` transition; loader parses it.
- Desktop: "Morph" added to the transition picker; presenter computes
  morph frames via a `compute_morph` command and applies CSS transform/
  opacity transitions on matching shape elements.
- `docs/sprint-records/wave-10.md` documenting the wave scope and acceptance
  criteria.

### Added — Wave 9 (templates and slide masters)

- A **template system** with six built-in templates: Default, Educator,
  Pitch, Conference Talk, Community Update, and Photo Essay. Each has a
  distinct theme (colors, fonts), a slide master (background layers), and
  3–5 named layouts. New types: `Master`, `BackgroundShape`,
  `PlaceholderDef`, `Layout`, `TemplateDefinition`, and `TemplateRegistry`.
  All additive; old decks default to an empty master and no layouts.
- `Deck` gains `template`, `layouts`, and `master` fields (`#[serde(default)]`).
  `Slide` gains `layout_ref`. `RenderOptions` carries the master and layouts
  for background painting.
- The renderer paints master background shapes behind slide content and
  renders dashed placeholder guides for the slide's active layout (editor
  only, not in presenter output).
- Two reversible commands: `SetTemplate` (applies a built-in template's
  theme, master, and layouts) and `SetSlideLayout` (sets a slide's layout).
- Desktop: a **template picker** (grid with theme previews) shown when
  creating a new deck, and a **layout picker** in the slide context menu.
  Three Tauri commands (`list_templates`, `set_template`, `set_slide_layout`).
- `docs/sprint-records/wave-9.md` documenting the wave scope and acceptance
  criteria.

### Added — Wave 8 (dual-display presenter)

- The presenter now runs in **dual-display mode**: a presenter window
  (controls, notes, timer, next-slide preview, tool toggles) and a separate
  fullscreen **audience window** showing only the slide. The two windows
  synchronize in real time via Tauri events (slide advances, build-step
  reveals, transitions).
- **Laser pointer**: when enabled (toggle button or `L` key), a colored dot
  follows the cursor on both windows. Throttled to ~30 fps to avoid event
  flooding. Transient — not saved to the deck.
- **Highlighter**: freehand strokes drawn over the current slide on both
  windows via an SVG overlay. Strokes clear on slide advance. Toggle button
  or `H` key.
- **Black/white slide**: `B` blanks the audience window to solid black,
  `W` to solid white — for Q&A. Toggling restores the slide.
- Presenter settings (`laser_pointer`, `laser_color`, `highlighter`,
  `highlighter_color`) persist on the deck via a new
  `SetPresenterSettings` command (verified inverse). Old decks default to
  all-off.
- `docs/sprint-records/wave-8.md` documenting the wave scope and acceptance
  criteria.

### Added — Wave 7 (deck structure & editor completeness)

- **Aspect ratios**: decks can now be 4:3, 16:10, or custom. `Deck` gains a
  `slide_size: Option<SlideSize>` field (additive, defaults to 16:9). The
  canvas and renderer respect the chosen dimensions. An aspect-ratio picker
  in the toolbar switches between presets.
- **Slide sections**: `Deck.sections` groups slides into named, collapsible
  sections. A new `SetSections` command replaces the section list (with a
  verified inverse). The thumbnail sidebar renders collapsible section
  headers.
- **Rich-text speaker notes**: `Slide.rich_notes: Option<Vec<Paragraph>>`
  augments the existing plain-text `notes` field. When set, the editor shows
  a rich-text notes panel reusing the existing formatting toolbar. A
  `SetRichNotes` command sets or clears it.
- **Find and replace**: a dialog (Cmd/Ctrl+F, Cmd/Ctrl+H) searches across
  all slides' text boxes and replaces matches through the command bus.
- **Shortcuts dialog**: a modal (`?` or menu) listing keyboard shortcuts.
- **High-contrast theme**: `Theme.high_contrast: bool` toggles a
  high-contrast palette for accessibility. A `SetHighContrast` command
  flips it with a verified inverse.
- Four reversible commands (`SetSlideSize`, `SetSections`, `SetRichNotes`,
  `SetHighContrast`). All additive; old decks deserialize unchanged.
- `docs/sprint-records/wave-7.md` documenting the wave scope and acceptance
  criteria.

### Added — Wave 6 (spell-check)

- Offline en-US spell-check is now first-class. A new `slides-spell` crate
  (no longer a stub) bundles a public-domain en-US word list (~233,000 words
  plus contractions) via `include_str!`, so checking works fully offline with
  no network dependency. Membership uses an exact `HashSet` (no false
  positives that silently accept errors).
- `SpellChecker` tokenizes text into word spans, flags misspellings with
  byte offsets, and returns correction suggestions ranked by edit distance
  then alphabetical order. Suggestions use the Norvig generate-and-test
  method (edits at distance 1–2, filtered against the dictionary set) — never
  scanning all 233k words per call, so there is no perceptible latency. A
  user dictionary (learned words) augments the bundled list
  case-insensitively.
- Three Tauri commands (`spell_check`, `spell_suggest`, `spell_add_word`)
  hold a single `SpellChecker` in shared state. The Svelte text editor
  renders red squiggles under misspelled words (debounced, non-blocking),
  a context menu offers suggestions and an "Add to dictionary" action, and
  learned words persist to a file in the app data directory and reload on
  startup.
- `docs/sprint-records/wave-6.md` documenting the wave scope and acceptance
  criteria.

### Added — Wave 5 (animations and transitions)

- Animations and transitions are now first-class. The reserved
  `Option<Animation>` and `Option<Transition>` fields on `Slide` are filled
  with real types: `Transition` (kind + duration), `TransitionKind` (none,
  fade, slide, push, wipe), `Animation` (ordered build step sequence),
  `BuildStep` (shape index + effect + duration), and `BuildEffect` (fade,
  slide-in left/right/top/bottom, appear, disappear). All additive; old
  decks deserialize unchanged (`None` by default).
- Five reversible commands: `SetTransition`, `SetSlideAnimation`,
  `AddBuildStep`, `RemoveBuildStepAt`, `MoveBuildStep`. Each produces a
  verified inverse; shape indices are validated at apply time.
- New `slides-animation` crate (no longer a stub) computes a deterministic
  timeline from the model (`build_timeline`) and exposes CSS class/keyframe
  helpers. Same input always yields identical output — no `HashMap`
  iteration. This satisfies the ROADMAP risk-register determinism
  requirement.
- `slides-render` wraps animated shapes in tagged `<g>` groups and emits a
  `<style>` block of `@keyframes`; the hooks are inert in the static editor
  SVG and activated by the presenter via CSS.
- PPTX loader parses `p:transition` (fade, push, wipe, slide, cut) and the
  simple `p:timing` build-in subset (`p:animEffect` with `filter`), mapping
  OOXML shape ids back to model indices. Complex or unrecognized timing
  falls back to passthrough with a loss warning (no panic). The saver
  patches `p:transition` and regenerates `p:timing` for dirty slides;
  untouched slides and all non-slide parts stay byte-for-byte identical.
- Five new Tauri commands (`set_transition`, `set_slide_animation`,
  `add_build_step`, `remove_build_step`, `move_build_step`). The Svelte
  editor has a transition picker, per-shape build-in menu, and a build-order
  list. The presenter advances build steps on click and plays transitions
  between slides.
- `docs/sprint-records/wave-5.md` documenting the wave scope, the shape
  identity decision (index-based for v0.2.0; stable IDs deferred to v0.3.0),
  and acceptance criteria.

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
