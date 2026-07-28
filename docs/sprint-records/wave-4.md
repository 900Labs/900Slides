# Wave 4 — v0.2.0 charts

Status: Proposed
Owner: 900 Labs
Scope target: `PRODUCT_SPEC.md` §5.2 (charts) and §6.2 (`Shape::Chart`)
Last updated: 2026-07-28

Wave 4 makes **charts first-class**: bar, column, line, area, pie, and scatter
charts with an in-place data-table editor. Unlike tables (which are in-slide
shapes), charts reference **separate PPTX parts** (`ppt/charts/chartN.xml`)
via `p:graphicFrame`, so the loader/saver must track chart parts in addition
to slides. This is the most complex wave in v0.2.0.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | Add `ChartShape`, `ChartType`, `ChartData`, commands |
| 2 | Chart crate | `crates/slides-chart/` | New (was stub): chart data model + SVG rendering |
| 3 | Renderer | `crates/slides-render/src/lib.rs` | Delegate chart rendering to `slides-chart` |
| 4 | Loader | `crates/slides-pptx/src/load.rs` + session | Map chart parts → `ChartShape` |
| 5 | Saver | `crates/slides-pptx/src/save.rs` | Patch chart parts for edited data |
| 6 | Commands | `crates/slides-core` + `apps/desktop` | Chart commands + data-table UI |

## Explicitly out of this wave

- Animations/transitions (`slides-animation`).
- Templates, masters, aspect ratios other than 16:9.
- Dual-display presenter, spell-check, PDF/ODP/PNG export.
- Chart sub-types (stacked bar, 100% stacked, 3D, doughnut, exploded pie).
  Only the six primary types listed in §5.2.
- Trend lines, error bars, secondary axes, log scales.
- Cross-app integration (embedding a 900Sheets chart — §5.2 line 48).
- Chart styling beyond default theme colors (legend position, font sizes,
  axis formatting). A clean default style is applied; custom styling is a
  follow-up.

## The shared contract — model changes (component 1, lands first)

All additive; `SCHEMA_VERSION` stays `1`. Old decks deserialize unchanged.

### New `Shape::Chart` variant

```rust
pub enum Shape {
    TextBox(TextBox),
    Image(ImageShape),
    Geometric(GeometricShape),
    Table(TableShape),
    Chart(ChartShape),     // NEW
    Passthrough(PassthroughObject),
}
```

### Chart model

```rust
/// A chart shape: a data visualization rendered as SVG.
pub struct ChartShape {
    /// Placement on the slide.
    pub transform: Transform,
    /// Chart type (bar, column, line, area, pie, scatter).
    pub chart_type: ChartType,
    /// Chart title, or empty for no title.
    #[serde(default)]
    pub title: String,
    /// The chart's data (categories + series, or XY points).
    pub data: ChartData,
    /// The original OOXML chart part path, for lossless save.
    /// None for charts inserted in 900Slides (written as new parts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_part: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Bar,      // horizontal bars
    Column,   // vertical bars
    Line,     // line chart
    Area,     // area chart
    Pie,      // pie chart
    Scatter,  // scatter plot
}

/// The data backing a chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChartData {
    /// Category-based data (bar, column, line, area, pie).
    CategoryBased {
        /// Category (x-axis / legend) labels.
        categories: Vec<String>,
        /// One or more data series.
        series: Vec<CategorySeries>,
    },
    /// XY data (scatter charts).
    XY {
        series: Vec<XYSeries>,
    },
}

/// A single data series in a category-based chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategorySeries {
    /// Series name (shown in legend).
    pub name: String,
    /// One value per category.
    pub values: Vec<f64>,
}

/// A single data series in a scatter chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XYSeries {
    /// Series name (shown in legend).
    pub name: String,
    /// (x, y) coordinate pairs.
    pub points: Vec<(f64, f64)>,
}
```

### New commands

- `AddChart { slide_id, chart: ChartShape }` — appends a chart shape. Inverse
  is `DeleteShape` for the resulting index.
- `SetChartType { slide_id, shape_index, chart_type }` — changes the type
  (may convert between CategoryBased and XY if type is scatter ↔ non-scatter;
  in that case, validate that the conversion is sensible or reject). Inverse
  snapshots the prior type.
- `SetChartData { slide_id, shape_index, data: ChartData }` — replaces the
  entire data set (the data-table editor sends the full updated grid). Inverse
  snapshots prior data.
- `SetChartTitle { slide_id, shape_index, title: String }` — sets or clears
  the title. Inverse snapshots prior title.

### Chart data invariants (validate)

- CategoryBased: at least 1 category, at least 1 series, every series has
  `values.len() == categories.len()`.
- XY: at least 1 series, every series has at least 1 point.
- Cap: max 100 categories, max 20 series, max 1000 points per series (prevent
  pathological data from freezing the renderer).

## Component 2 — Chart crate (`slides-chart`, no longer a stub)

The chart crate renders a `ChartShape` to a deterministic SVG string. It does
NOT depend on `slides-render`; `slides-render` depends on it.

```rust
/// Renders a chart to a deterministic SVG string.
pub fn render_chart_svg(chart: &slides_core::ChartShape, width_emu: f64, height_emu: f64) -> String;
```

### Rendering rules

The SVG is a self-contained `<svg viewBox="0 0 W H">` element (W, H in EMU).
Deterministic attribute order, stable element order, no HashMap iteration.

- **Plot area**: margin for axes/labels (e.g. 10% of each dimension).
- **Axes** (bar/column/line/area/scatter): x-axis and y-axis lines, tick labels
  (category names or values), grid lines (light, behind data).
- **Pie**: no axes; a legend listing category names with color swatches.
- **Legend** (when >1 series, or pie): top-right or bottom, listing series names.
- **Title**: centered at the top when non-empty.

Per type:
- **Bar**: horizontal rectangles, one per value, grouped by series.
- **Column**: vertical rectangles.
- **Line**: polylines connecting points; one per series.
- **Area**: filled polygons (line + fill to the x-axis baseline).
- **Pie**: wedges; category labels with leader lines or a legend.
- **Scatter**: `<circle>` points; one per (x,y).

Colors: cycle through a fixed palette (e.g. 6 theme-derived colors). Deterministic.
All text XML-escaped. Numeric formatting: round to a reasonable precision.

## Component 3 — Renderer (`slides-render`)

Small change: replace the `Shape::Chart(_) => {}` no-op arm with a call to
`slides_chart::render_chart_svg`, wrapping the result in a `<g transform>` for
positioning. Add `slides-chart` to `slides-render`'s Cargo.toml deps.

## Component 4 — Loader (`slides-pptx/src/load.rs` + session)

Chart loading is more involved than tables because charts live in **separate
parts**:

1. `p:graphicFrame` with `a:graphicData uri="...chart..."` → resolve the
   relationship to `ppt/charts/chartN.xml`.
2. Read `chartN.xml`: parse `c:chartSpace/c:chart`:
   - `c:title` → title text.
   - `c:plotArea/c:barChart` (or `c:bar3DChart`) → Bar.
   - `c:plotArea/c:lineChart` → Line.
   - `c:plotArea/c:areaChart` → Area.
   - `c:plotArea/c:pieChart` → Pie.
   - `c:plotArea/c:scatterChart` → Scatter.
   - Note: OOXML uses `barDir="bar"` for horizontal bars and `barDir="col"`
     for vertical columns — both appear under `c:barChart`.
   - Series: `c:ser` → name from `c:tx/c:strRef/c:strCache/c:pt/c:v`,
     values from `c:val/c:numRef/c:numCache/c:pt/c:v`.
   - Categories: `c:cat/c:strRef/c:strCache/c:pt/c:v` (shared across series).
3. Build `ChartData` and `ChartShape`. Store `source_part = Some("ppt/charts/chartN.xml")`
   for lossless save.
4. The `p:graphicFrame` itself is the in-slide element; the chart XML is a
   separate part. The session tracks `chart_parts: HashMap<String, Vec<u8>>`
   (original chart part bytes) for patching on save.
5. If the chart type is unrecognized or the data is malformed, fall back to
   Passthrough + loss warning. Do NOT panic.

## Component 5 — Saver (`slides-pptx/src/save.rs`)

For charts, the saver must handle **two** things:
1. The in-slide `p:graphicFrame` (patched like any shape — but since the
   graphicFrame just references the chart part, it rarely changes unless the
   frame moves/resizes).
2. The **chart part** (`ppt/charts/chartN.xml`): when chart data is edited,
   patch the data sections (series values, category names, title) while
   preserving the rest of the chart XML byte-for-byte.

Session tracking: add `dirty_chart_parts: HashSet<String>` alongside
`dirty_slides`. When a chart command modifies a chart, mark its source_part
dirty. On save, for each dirty chart part, patch the data sections:
- `c:numCache` values: replace with the model's values.
- `c:strCache` categories/series names: replace.
- `c:title`: replace title text.

Newly inserted charts (no source_part) get a new `ppt/charts/chartN.xml` part
written from scratch, with a relationship from the slide.

**Lossless guarantee (§4.9):** unedited chart parts and all non-chart parts
stay byte-for-byte identical.

## Component 6 — Commands (desktop)

- `add_chart(slide_id, chart_type)` — inserts a chart with default sample data
  (e.g. 3 categories, 2 series with small values). Returns snapshot.
- `set_chart_type(slide_id, shape_index, chart_type)`.
- `set_chart_data(slide_id, shape_index, data)` — from the data-table editor.
- `set_chart_title(slide_id, shape_index, title)`.

Frontend:
- Toolbar "Chart" button → dropdown of the 6 types.
- Canvas renders the chart SVG (via `render_slide_svg` or the renderer).
- Double-click a chart → opens an overlay data-table editor (a grid: rows =
  series, columns = categories; editable cells; add/remove series/category).
- Chart type switcher in the overlay.

## Dependency ordering

1. **Model** (component 1) — single worktree, merged first.
2. **Parallel fan-out:**
   - `slides-chart` crate (needs model)
   - `slides-pptx` loader + saver (needs model)
3. **Renderer** (component 3) — after the chart crate (small, ~20 lines).
4. **Desktop** — after all of the above.

## Acceptance criteria

1. A PPTX containing a bar/column/line/area/pie/scatter chart loads into a
   `ChartShape` (not passthrough), and saving without edits produces a
   byte-for-byte-identical package. Editing chart data and saving patches
   only the data sections; everything else is unchanged.
2. All six chart types render to deterministic SVG.
3. The data-table editor lets the user edit categories, series, and values;
   changes render immediately and round-trip through save.
4. Every new command round-trips through undo correctly.
5. Quality gate green. No telemetry. Privacy gate passes.

## Test fixtures

- A hand-built PPTX with one chart part (bar chart, 3 categories, 2 series).
  No real-world decks.
- Round-trip: load → edit data → save → assert chart part patched correctly,
  all other parts byte-identical.
- Renderer determinism for each chart type.
- Chart data validation tests.
