# Wave 3 — v0.2.0 tables

Status: Proposed
Owner: 900 Labs
Scope target: `PRODUCT_SPEC.md` §5.2 (tables) and §6.2 (`Shape::Table`)
Last updated: 2026-07-27

Wave 3 makes **tables first-class** across the whole stack: model, renderer,
PPTX load/save, and the desktop editor. It follows the same additive pattern
as Waves 1 and 2 — a new `Shape::Table` variant, new commands, loader/saver
mapping, SVG rendering, and desktop commands + UI. No schema break.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | Add `Table`, `TableRow`, `TableCell`, `TableBorders`; new commands |
| 2 | Renderer | `crates/slides-render/src/lib.rs` | Add table SVG rendering |
| 3 | Loader | `crates/slides-pptx/src/load.rs` | Map `p:graphicFrame`/`a:tbl` → `Table` |
| 4 | Saver | `crates/slides-pptx/src/save.rs` | Emit `p:graphicFrame`/`a:tbl` for edited slides |
| 5 | Commands | `crates/slides-core` (commands) + `apps/desktop/src-tauri/src/commands.rs` | Table mutation commands + Tauri surface |

## Explicitly out of this wave (later waves)

- Charts (`slides-chart`), animations/transitions (`slides-animation`).
- Cell-level rich text beyond plain text (cell text is a `String` this wave;
  rich runs inside cells is a follow-up).
- Merged cells (rowspan/colspan). OOXML supports them; this wave models a
  regular grid only.
- Cell-level animations or conditional formatting.
- Table styles/templates (the "banded rows" preset). A default style is applied;
  custom styles are a follow-up.
- Pivot tables or formula evaluation (900Slides is a presentation tool, not a
  spreadsheet).

## The shared contract — model changes (component 1, lands first)

All additive; `SCHEMA_VERSION` stays `1`. New fields are `#[serde(default)]`
so v0.1.0 / Wave 1 / Wave 2 decks and recovery snapshots still load.

### New `Shape::Table` variant

```rust
pub enum Shape {
    TextBox(TextBox),
    Image(ImageShape),
    Geometric(GeometricShape),
    Table(TableShape),     // NEW
    Passthrough(PassthroughObject),
}
```

Keep the existing serde tag/content attributes. `Table` serializes as
`{ "kind": "table", "value": {...} }`.

### Table model

```rust
/// A table shape: a grid of cells with per-column widths and per-row heights.
pub struct TableShape {
    /// Placement of the table on the slide.
    pub transform: Transform,
    /// Rows, top to bottom. Must be non-empty.
    pub rows: Vec<TableRow>,
    /// Per-column width in EMU. Length must equal the number of columns
    /// (all rows must have the same column count).
    pub column_widths: Vec<f64>,
    /// Default cell borders applied when a cell has no explicit border.
    #[serde(default)]
    pub default_borders: TableBorders,
    /// Whether the first row is rendered as a header (bold, distinct fill).
    #[serde(default)]
    pub header_row: bool,
}

/// A single row of cells.
pub struct TableRow {
    /// Row height in EMU.
    pub height: f64,
    /// Cells, left to right.
    pub cells: Vec<TableCell>,
}

/// A single cell.
pub struct TableCell {
    /// Plain text content of the cell (rich runs inside cells is a follow-up).
    #[serde(default)]
    pub text: String,
    /// Cell fill color, or None to inherit the table default.
    #[serde(default)]
    pub fill: Option<Fill>,
    /// Cell-level border overrides. When None, inherit `default_borders`.
    #[serde(default)]
    pub borders: Option<TableBorders>,
    /// Horizontal alignment of the cell text.
    #[serde(default)]
    pub align: CellAlign,
}

/// Horizontal alignment of cell text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CellAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// The four borders of a cell (or the table default).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TableBorders {
    #[serde(default)]
    pub top: Option<BorderEdge>,
    #[serde(default)]
    pub bottom: Option<BorderEdge>,
    #[serde(default)]
    pub left: Option<BorderEdge>,
    #[serde(default)]
    pub right: Option<BorderEdge>,
}

/// A single border edge: color, width, and dash style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BorderEdge {
    pub color: Color,
    /// Width in EMU.
    pub width_emu: f64,
    #[serde(default)]
    pub dash: DashStyle,
}
```

Reuse `DashStyle` (already in the model from Wave 1) and `Fill`/`Color`.

### Table invariants (enforced in constructors/validate)

- `rows` is non-empty.
- All rows have the same number of columns.
- `column_widths.len() == rows[0].cells.len()`.
- Hard cap: 50 rows × 50 columns (per §5.2). `TableShape::new` and the
  `AddTable` command validate this; a table exceeding the cap is rejected
  (the loader clamps + warns rather than panicking).

### New commands

Each implements `Command` with a verified `inverse`:

- `AddTable { slide_id, table: TableShape }` — appends a table shape. Inverse
  is `DeleteShape` for the resulting index. Validate checks slide exists and
  the table invariants + 50×50 cap.
- `SetCellText { slide_id, shape_index, row, col, text }` — sets a cell's text.
  Inverse snapshots the prior text.
- `SetCellStyle { slide_id, shape_index, row, col, fill, borders, align }` —
  merge-patch over a cell's style (only `Some` fields change). Inverse
  snapshots prior values for the changed fields.
- `ResizeTable { slide_id, shape_index, column_widths, row_heights }` — sets
  column widths and row heights. Inverse snapshots the prior vectors.
- `InsertRow { slide_id, shape_index, index, row }` — inserts a row at `index`.
  Inverse removes it. Validate checks the cap.
- `InsertColumn { slide_id, shape_index, index, width }` — inserts a column.
  Inverse removes it. Validate checks the cap.
- `DeleteRow { slide_id, shape_index, index }` — removes a row. Inverse
  re-inserts it at the same index with the captured cells.
- `DeleteColumn { slide_id, shape_index, index }` — removes a column. Inverse
  re-inserts it.

`DeleteShape` (from Wave 1) already works for any shape by index, so deleting
a whole table needs no new command. But `DeleteShape`'s inverse must capture
the removed `TableShape` fully (it already snapshots the whole `Shape`, so this
works automatically — verify with a test).

## Component specifications

### 2. Renderer (`slides-render`)

Render a `Table` as a grid of `<rect>` cells with `<text>` content:

- Outer frame from `transform`. Cell width = `column_widths[col]`, height =
  `row.height`. Cell x/y computed by accumulating widths/heights.
- Each cell: a `<rect>` with the cell's fill (or a default). Text drawn
  left/center/right aligned within the cell, vertically centered.
- Borders: each cell's edges drawn as `<line>` or via `stroke` on the rect.
  Inherit from `default_borders` when the cell has no override.
- Header row: bold text + a distinct fill (e.g. a slightly darker shade of the
  table's fill, or theme accent at low opacity).
- Determinism: cell order is stable (row-major), attribute order fixed. The
  render hash covers the full table state.
- Missing/empty table renders an empty frame (do not panic).

### 3. Loader (`slides-pptx/src/load.rs`)

Map `p:graphicFrame` containing `a:tbl` → `TableShape`:

- Resolve the graphic frame's `p:xfrm` for the transform (off/ext/rotation).
- Read `a:tbl` → rows and cells:
  - `a:tr` (table row) → `TableRow` with `height` from `@h`.
  - `a:tc` (table cell) → `TableCell` with text from `a:txBody/a:p/a:r/a:t`
    (plain text only this wave; rich runs collapse to text with a loss warning
    if they carry marks).
  - Cell fill from `a:tcPr/a:solidFill/a:srgbClr`.
  - Cell borders from `a:tcPr` `a:lnL`/`a:lnR`/`a:lnT`/`a:lnB`.
  - Alignment from `a:txBody/a:p/a:pPr/@algn` (l/ctr/r).
- `a:tblGrid/a:gridCol @w` → `column_widths`.
- `a:tblPr @firstRow="1"` → `header_row = true`.
- Clamp to 50×50: if the table exceeds the cap, truncate and add a loss
  warning. Do NOT panic.
- If the graphic frame contains something other than `a:tbl` (e.g. a chart or
  SmartArt), it stays `Passthrough` (unchanged from today).

### 4. Saver (`slides-pptx/src/save.rs`)

For each dirty slide with a `Table` shape, emit the full `p:graphicFrame` +
`a:tbl` from the model:

- `p:graphicFrame` with `p:nvGraphicFramePr`, `p:xfrm`, and `a:graphic`/
  `a:graphicData` containing `a:tbl`.
- `a:tblPr` with `firstRow` attribute when `header_row`.
- `a:tblGrid` with one `a:gridCol` per column.
- `a:tr` per row with `a:tc` per cell: `a:txBody` (plain text), `a:tcPr`
  (fill, borders, alignment).
- **Lossless guarantee (§4.9) unchanged:** unedited slides are copied
  byte-for-byte. Tables appear only on dirty slides.

### 5. Commands (desktop)

- The existing `add_shape` / `delete_shape` commands from Wave 1 handle
  insertion/deletion by index; `AddTable` and `DeleteShape` reuse them. Add
  Tauri commands for the table-specific mutations:
  - `set_cell_text(slide_id, shape_index, row, col, text)`
  - `set_cell_style(slide_id, shape_index, row, col, fill, borders, align)`
  - `resize_table(slide_id, shape_index, column_widths, row_heights)`
  - `insert_row` / `insert_column` / `delete_row` / `delete_column`
  - `add_table(slide_id, rows, cols)` — builds a default table (e.g. 3×3 with
    header row) and applies `AddTable`.
- Frontend: render the table on the canvas (reuse the renderer SVG or render
  directly). Cell text editing via click → input. Toolbar button to insert a
  table with a size picker (rows × cols). Context menu for insert/delete row/
  column.

Keep the frontend functional but minimal: the wave must display tables, let
the user insert one, edit cell text, and add/remove rows/columns.

## Dependency ordering (how to execute)

Components 2–5 consume the new `slides-core` types, so the model lands first:

1. **Model (component 1)** — single worktree, reviewed and merged first.
2. **Parallel fan-out on the merged model:**
   - `slides-render` (needs model only)
   - `slides-pptx` loader + saver together (needs model)
   - desktop commands + frontend (needs model + render + pptx) — final merge

## Acceptance criteria

The wave is done when all of these hold:

1. A PPTX containing a table loads into a `TableShape` (not passthrough), and
   saving produces a byte-for-byte-identical package for every *unmodified*
   slide plus correctly regenerated parts for edited slides. A round-trip of
   an edited table preserves cell text, fills, borders, and column widths.
2. The 50×50 cap is enforced on insert/resize; the loader clamps + warns.
3. `slides_render::render_slide` produces deterministic SVG (stable hash) for
   tables.
4. Every new command round-trips through undo correctly (full `Deck` equality
   including the table's cell grid).
5. The editor canvas displays tables; the toolbar inserts a table; cell text
   is editable; rows/columns can be added and removed.
6. **Quality gate green** (`AGENTS.md` order).
7. No telemetry, analytics, or remote calls introduced (§4, §8).
8. `scripts/verify-public-release.sh` passes.

## Test fixtures

- Add a hand-built PPTX fixture with one table (3×3, header row, varied cell
  fills and borders). No real-world decks, no EXIF, no local paths.
- Round-trip test: load → edit a cell → save → assert untouched parts
  byte-identical and the edited cell round-trips.
- Renderer hash-gate test for a table.
- 50×50 cap enforcement test.
