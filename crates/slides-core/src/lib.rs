//! Deck model, commands, undo, theme.

pub mod accessibility;

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current deck model schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// A deck: the root of a presentation document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    /// Schema version of this deck.
    pub schema_version: u32,
    /// Stable identifier for this deck.
    pub id: String,
    /// Theme applied to the whole deck.
    pub theme: Theme,
    /// Ordered list of slides.
    pub slides: Vec<Slide>,
    /// Fixed slide dimensions (aspect ratio), if any. Defaults to `None`
    /// (`#[serde(default)]`) so decks serialized before this field existed
    /// deserialize unchanged — a non-breaking, additive change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide_size: Option<SlideSize>,
    /// Named slide sections, in slide order. Defaults to empty
    /// (`#[serde(default)]`) so decks serialized before this field existed
    /// deserialize unchanged — a non-breaking, additive change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SlideSection>,
    /// Key-addressed store of image bytes referenced by image shapes.
    #[serde(default)]
    pub media: MediaStore,
    /// Presenter configuration (laser pointer, highlighter). Additive;
    /// defaults to all-off so old decks are unaffected.
    #[serde(default)]
    pub presenter_settings: PresenterSettings,
    /// Built-in template this deck is based on (e.g. "default", "pitch").
    /// Additive; old decks have no template and render identically to before.
    #[serde(default)]
    pub template: Option<String>,
    /// The deck's available layouts, derived from its template. Additive and
    /// skipped when empty so old decks serialize unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layouts: Vec<Layout>,
    /// The deck's slide master: background layers and placeholder definitions.
    /// Additive; old decks deserialize into an empty master.
    #[serde(default)]
    pub master: Master,
    /// Threaded comments anchored to slides, shapes, or text ranges. Additive
    /// and skipped when empty so old decks serialize unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CommentThread>,
}

impl Default for Deck {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: String::new(),
            theme: Theme::default(),
            slides: Vec::new(),
            slide_size: None,
            sections: Vec::new(),
            media: MediaStore::default(),
            presenter_settings: PresenterSettings::default(),
            template: None,
            layouts: Vec::new(),
            master: Master::default(),
            comments: Vec::new(),
        }
    }
}

impl Deck {
    /// Creates an empty deck with the default template.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            ..Self::default()
        }
    }

    /// Returns a mutable reference to the slide with the given id, if any.
    pub fn slide_mut(&mut self, id: &str) -> Option<&mut Slide> {
        self.slides.iter_mut().find(|s| s.id == id)
    }

    /// Returns a reference to the slide with the given id, if any.
    pub fn slide(&self, id: &str) -> Option<&Slide> {
        self.slides.iter().find(|s| s.id == id)
    }

    /// Returns a reference to the comment thread with the given id, if any.
    pub fn comment_thread(&self, id: &str) -> Option<&CommentThread> {
        self.comments.iter().find(|thread| thread.id == id)
    }

    /// Returns a mutable reference to the comment thread with the given id.
    pub fn comment_thread_mut(&mut self, id: &str) -> Option<&mut CommentThread> {
        self.comments.iter_mut().find(|thread| thread.id == id)
    }
}

/// Fixed slide dimensions, in EMU, used to pin the deck's aspect ratio.
///
/// The `Option<SlideSize>` on [`Deck`] defaults to `None`
/// (`#[serde(default)]`) so decks serialized before this field existed
/// deserialize unchanged — a non-breaking, additive change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlideSize {
    /// Slide width, in EMU.
    pub width_emu: f64,
    /// Slide height, in EMU.
    pub height_emu: f64,
}

impl SlideSize {
    /// 16:9 widescreen (12,192,000 x 6,858,000 EMU) — the PPTX default.
    pub fn widescreen_16_9() -> Self {
        Self {
            width_emu: 12_192_000.0,
            height_emu: 6_858_000.0,
        }
    }

    /// 4:3 standard (9,144,000 x 6,858,000 EMU).
    pub fn standard_4_3() -> Self {
        Self {
            width_emu: 9_144_000.0,
            height_emu: 6_858_000.0,
        }
    }

    /// 16:10 widescreen (12,149,333 x 7,593,333 EMU).
    pub fn widescreen_16_10() -> Self {
        Self {
            width_emu: 12_149_333.0,
            height_emu: 7_593_333.0,
        }
    }
}

/// A named section that starts at `start_slide_id` and spans the slides that
/// follow it, up to the next section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlideSection {
    /// Human-readable section name.
    pub name: String,
    /// Id of the first slide in this section.
    pub start_slide_id: String,
}

/// Theme applied to a deck.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    /// Background color.
    pub background: Color,
    /// Font family for headings.
    pub heading_font: String,
    /// Font family for body text.
    pub body_font: String,
    /// Accent color.
    pub accent_color: Color,
    /// High-contrast accessibility mode. Defaults to `false`
    /// (`#[serde(default)]`) so decks serialized before this field existed
    /// deserialize unchanged — a non-breaking, additive change.
    #[serde(default)]
    pub high_contrast: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::white(),
            heading_font: String::from("Calibri"),
            body_font: String::from("Calibri"),
            accent_color: Color::rgb(0, 112, 192),
            high_contrast: false,
        }
    }
}

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
    /// Projector compensation filters (brightness, contrast, etc.).
    #[serde(default)]
    pub projector_filters: ProjectorFilters,
}

/// CSS filters applied to the audience window for projector compensation.
/// Persisted per-deck via `PresenterSettings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectorFilters {
    /// Invert all colors.
    #[serde(default)]
    pub invert: bool,
    /// Brightness multiplier (1.0 = normal, 0.0 = black, 2.0 = double).
    #[serde(default = "default_brightness")]
    pub brightness: f64,
    /// Contrast multiplier (1.0 = normal).
    #[serde(default = "default_contrast")]
    pub contrast: f64,
    /// Saturation multiplier (1.0 = normal, 0.0 = grayscale).
    #[serde(default = "default_saturation")]
    pub saturation: f64,
    /// Sepia intensity (0.0 = none, 1.0 = full sepia).
    #[serde(default)]
    pub sepia: f64,
    /// Hue rotation in degrees (0.0 = none, 360.0 = full rotation).
    #[serde(default)]
    pub hue_rotate: f64,
}

impl Default for ProjectorFilters {
    fn default() -> Self {
        Self {
            invert: false,
            brightness: default_brightness(),
            contrast: default_contrast(),
            saturation: default_saturation(),
            sepia: 0.0,
            hue_rotate: 0.0,
        }
    }
}

fn default_brightness() -> f64 {
    1.0
}
fn default_contrast() -> f64 {
    1.0
}
fn default_saturation() -> f64 {
    1.0
}

impl Default for PresenterSettings {
    fn default() -> Self {
        Self {
            laser_pointer: false,
            laser_color: default_laser_color(),
            highlighter: false,
            highlighter_color: default_highlighter_color(),
            projector_filters: ProjectorFilters::default(),
        }
    }
}

fn default_laser_color() -> String {
    "#ff0000".to_string()
}

fn default_highlighter_color() -> String {
    "#ffff00".to_string()
}

/// An RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel.
    pub a: u8,
}

impl Color {
    /// Creates an opaque color from RGB channels.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates a white color.
    pub const fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    /// Creates a black color.
    pub const fn black() -> Self {
        Self::rgb(0, 0, 0)
    }
}

/// Stored media bytes plus metadata, keyed in a [`MediaStore`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaEntry {
    /// MIME type of the stored bytes (e.g. `image/png`).
    pub mime: String,
    /// Raw media bytes.
    pub bytes: Vec<u8>,
    /// Native pixel width of the media.
    pub width: u32,
    /// Native pixel height of the media.
    pub height: u32,
}

/// A deterministic, key-addressed store of media bytes for a [`Deck`].
///
/// Images reference bytes by key rather than inlining them, so the deck model
/// stays diffable and undo history stays bounded. The underlying map is a
/// [`BTreeMap`] so iteration order is stable across runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MediaStore(BTreeMap<String, MediaEntry>);

impl MediaStore {
    /// Creates an empty media store.
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Returns the entry stored under `key`, if any.
    pub fn get(&self, key: &str) -> Option<&MediaEntry> {
        self.0.get(key)
    }

    /// Returns `true` if an entry is stored under `key`.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Inserts `entry` under `key`, replacing any existing entry.
    pub fn insert(&mut self, key: impl Into<String>, entry: MediaEntry) {
        self.0.insert(key.into(), entry);
    }

    /// Removes and returns the entry stored under `key`, if any.
    pub fn remove(&mut self, key: &str) -> Option<MediaEntry> {
        self.0.remove(key)
    }

    /// Returns the number of stored entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if no entries are stored.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over the stored entries by key.
    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, String, MediaEntry> {
        self.0.iter()
    }
}

/// A single slide within a deck.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    /// Stable identifier for this slide.
    pub id: String,
    /// Plain-text speaker notes.
    pub notes: String,
    /// Shapes placed on this slide.
    pub shapes: Vec<Shape>,
    /// Per-slide build-in animation sequence, if any. Defaults to `None`
    /// (`#[serde(default)]`) so decks serialized before this field existed
    /// deserialize unchanged — a non-breaking, additive change.
    #[serde(default)]
    pub animation: Option<Animation>,
    /// Transition played when advancing to this slide, if any. Defaults to
    /// `None` (`#[serde(default)]`) so old decks deserialize unchanged.
    #[serde(default)]
    pub transition: Option<Transition>,
    /// Rich-text speaker notes, as paragraphs. When `Some`, the editor uses
    /// these; otherwise it falls back to the plain [`Slide::notes`] field.
    /// Defaults to `None` (`#[serde(default)]`) so old decks deserialize
    /// unchanged — a non-breaking, additive change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rich_notes: Option<Vec<Paragraph>>,
    /// Name of the layout (from [`Deck::layouts`]) this slide uses, if any.
    /// Defaults to `None` (`#[serde(default)]`) so old decks deserialize
    /// unchanged — a non-breaking, additive change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_ref: Option<String>,
    /// Per-slide reduce-motion override. When `Some(true)`, the presenter
    /// renders this slide's build-ins instantly (no animation), overriding any
    /// system-level preference for this slide only. When `None`, the system
    /// preference is used. Defaults to `None` so old decks deserialize
    /// unchanged — a non-breaking, additive change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduce_motion: Option<bool>,
    /// Per-slide rehearsed duration in milliseconds. `None` means no timing
    /// recorded. Used by the presenter's auto-advance mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rehearsed_duration_ms: Option<u32>,
}

/// A shape or content object placed on a slide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Shape {
    /// An editable text box.
    TextBox(TextBox),
    /// An opaque, byte-for-byte preserved object.
    Passthrough(PassthroughObject),
    /// An image, referencing bytes in the deck's [`MediaStore`].
    Image(ImageShape),
    /// A geometric shape.
    Geometric(GeometricShape),
    /// A table: a grid of cells with per-column widths and per-row heights.
    Table(TableShape),
    /// A chart: a plotted data series.
    Chart(ChartShape),
}

impl Shape {
    /// Returns the stable id of this shape.
    ///
    /// For editable variants this is the shape's own `id`; for
    /// [`Shape::Passthrough`] it is the passthrough object's id.
    pub fn id(&self) -> &str {
        match self {
            Shape::TextBox(text_box) => &text_box.id,
            Shape::Passthrough(passthrough) => &passthrough.id,
            Shape::Image(image) => &image.id,
            Shape::Geometric(geometric) => &geometric.id,
            Shape::Table(table) => &table.id,
            Shape::Chart(chart) => &chart.id,
        }
    }

    /// Sets the stable id of this shape.
    ///
    /// For [`Shape::Passthrough`], sets the passthrough object's id.
    pub fn set_id(&mut self, id: String) {
        match self {
            Shape::TextBox(text_box) => text_box.id = id,
            Shape::Passthrough(passthrough) => passthrough.id = id,
            Shape::Image(image) => image.id = id,
            Shape::Geometric(geometric) => geometric.id = id,
            Shape::Table(table) => table.id = id,
            Shape::Chart(chart) => chart.id = id,
        }
    }

    /// Generates a new unique shape id (UUID v4 without hyphens).
    pub fn generate_id() -> String {
        Uuid::new_v4().simple().to_string()
    }
}

/// An editable text box shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBox {
    /// Stable identifier for cross-slide matching (Magic Move). Defaults to
    /// empty string so old decks deserialize unchanged.
    #[serde(default)]
    pub id: String,
    /// Bounding rectangle of the text box, in EMU.
    pub frame: Rect,
    /// Paragraphs of text inside the box.
    pub paragraphs: Vec<Paragraph>,
}

/// An image placed on a slide, referencing bytes in the deck's [`MediaStore`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageShape {
    /// Stable identifier for cross-slide matching (Magic Move). Defaults to
    /// empty string so old decks deserialize unchanged.
    #[serde(default)]
    pub id: String,
    /// Position, size, and rotation of the image.
    pub transform: Transform,
    /// Key of this image's bytes in the deck's [`MediaStore`].
    pub media_ref: String,
    /// Optional crop applied to the image.
    pub crop: Option<Crop>,
    /// Accessibility alt text describing the image for screen readers. Defaults
    /// to `None` (`#[serde(default)]`) so decks serialized before this field
    /// existed deserialize unchanged — a non-breaking, additive change. The
    /// accessibility checker flags images with `None` or empty alt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,
}

/// A geometric shape placed on a slide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricShape {
    /// Stable identifier for cross-slide matching (Magic Move). Defaults to
    /// empty string so old decks deserialize unchanged.
    #[serde(default)]
    pub id: String,
    /// Position, size, and rotation of the shape.
    pub transform: Transform,
    /// Primitive geometry of the shape.
    pub geometry: Geometry,
    /// Visual style of the shape.
    pub style: Style,
}

/// Maximum number of rows in a table (PRODUCT_SPEC.md §5.2).
pub const MAX_TABLE_ROWS: usize = 50;
/// Maximum number of columns in a table (PRODUCT_SPEC.md §5.2).
pub const MAX_TABLE_COLS: usize = 50;

/// Errors returned by [`TableShape::new`] and [`TableShape::validate`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    /// The table must have at least one row.
    #[error("table must have at least one row")]
    Empty,
    /// All rows must have the same number of columns.
    #[error("all rows must have the same number of columns")]
    RaggedRows,
    /// `column_widths` length must equal the column count.
    #[error("column_widths length ({got}) must equal column count ({want})")]
    ColumnCountMismatch {
        /// The length of `column_widths` that was provided.
        got: usize,
        /// The number of columns implied by the rows.
        want: usize,
    },
    /// The table exceeds the 50x50 cell limit.
    #[error("table exceeds the 50x50 cell limit ({rows}x{cols})")]
    TooLarge {
        /// The number of rows in the offending table.
        rows: usize,
        /// The number of columns in the offending table.
        cols: usize,
    },
}

/// A table shape: a grid of cells with per-column widths and per-row heights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableShape {
    /// Stable identifier for cross-slide matching (Magic Move). Defaults to
    /// empty string so old decks deserialize unchanged.
    #[serde(default)]
    pub id: String,
    /// Placement of the table on the slide.
    pub transform: Transform,
    /// Rows, top to bottom. Must be non-empty.
    pub rows: Vec<TableRow>,
    /// Per-column width in EMU. Length must equal the number of columns.
    pub column_widths: Vec<f64>,
    /// Default cell borders applied when a cell has no explicit border.
    #[serde(default)]
    pub default_borders: TableBorders,
    /// Whether the first row is rendered as a header (bold, distinct fill).
    #[serde(default)]
    pub header_row: bool,
}

impl TableShape {
    /// Constructs a table, validating its structural invariants.
    ///
    /// Returns [`TableError`] if the rows are empty, ragged, mismatched with
    /// `column_widths`, or exceed the [`MAX_TABLE_ROWS`] x [`MAX_TABLE_COLS`]
    /// cap. The new table starts with empty default borders and no header row.
    pub fn new(
        transform: Transform,
        rows: Vec<TableRow>,
        column_widths: Vec<f64>,
    ) -> Result<Self, TableError> {
        if rows.is_empty() {
            return Err(TableError::Empty);
        }
        let col_count = rows[0].cells.len();
        if !rows.iter().all(|row| row.cells.len() == col_count) {
            return Err(TableError::RaggedRows);
        }
        if column_widths.len() != col_count {
            return Err(TableError::ColumnCountMismatch {
                got: column_widths.len(),
                want: col_count,
            });
        }
        if rows.len() > MAX_TABLE_ROWS || col_count > MAX_TABLE_COLS {
            return Err(TableError::TooLarge {
                rows: rows.len(),
                cols: col_count,
            });
        }
        Ok(Self {
            id: String::new(),
            transform,
            rows,
            column_widths,
            default_borders: TableBorders::default(),
            header_row: false,
        })
    }

    /// Builds a table that fills `frame` with `rows` rows and `cols` columns of
    /// equal size, empty cells, and no header row.
    ///
    /// Panics if `rows` or `cols` is zero, since an empty table is a
    /// programming error rather than a recoverable condition.
    pub fn default_grid(rows: usize, cols: usize, frame: Rect) -> Self {
        assert!(rows > 0, "default_grid requires at least one row");
        assert!(cols > 0, "default_grid requires at least one column");
        let row_height = frame.height / rows as f64;
        let column_width = frame.width / cols as f64;
        let table_rows = (0..rows)
            .map(|_| TableRow {
                height: row_height,
                cells: (0..cols).map(|_| TableCell::default()).collect(),
            })
            .collect::<Vec<_>>();
        let column_widths = (0..cols).map(|_| column_width).collect::<Vec<_>>();
        Self {
            id: String::new(),
            transform: Transform {
                frame,
                rotation: 0.0,
            },
            rows: table_rows,
            column_widths,
            default_borders: TableBorders::default(),
            header_row: false,
        }
    }

    /// Returns the number of rows in the table.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the number of columns in the table, or `0` if it has no rows.
    pub fn col_count(&self) -> usize {
        self.rows.first().map_or(0, |row| row.cells.len())
    }

    /// Returns a reference to the cell at `(row, col)`, if in range.
    pub fn cell(&self, row: usize, col: usize) -> Option<&TableCell> {
        self.rows.get(row).and_then(|row| row.cells.get(col))
    }

    /// Returns a mutable reference to the cell at `(row, col)`, if in range.
    pub fn cell_mut(&mut self, row: usize, col: usize) -> Option<&mut TableCell> {
        self.rows
            .get_mut(row)
            .and_then(|row| row.cells.get_mut(col))
    }

    /// Returns `true` if the table satisfies the same invariants as [`new`].
    pub fn validate(&self) -> bool {
        if self.rows.is_empty() {
            return false;
        }
        let col_count = self.rows[0].cells.len();
        if !self.rows.iter().all(|row| row.cells.len() == col_count) {
            return false;
        }
        if self.column_widths.len() != col_count {
            return false;
        }
        if self.rows.len() > MAX_TABLE_ROWS || col_count > MAX_TABLE_COLS {
            return false;
        }
        true
    }
}

/// A single row of cells in a [`TableShape`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    /// Row height in EMU.
    pub height: f64,
    /// Cells, left to right.
    pub cells: Vec<TableCell>,
}

/// A single cell in a [`TableShape`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TableCell {
    /// Plain text content of the cell.
    #[serde(default)]
    pub text: String,
    /// Cell fill, or `None` to inherit the table default.
    #[serde(default)]
    pub fill: Option<Fill>,
    /// Cell-level border overrides. When `None`, inherit the table default.
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
    /// Left-aligned text.
    #[default]
    Left,
    /// Centered text.
    Center,
    /// Right-aligned text.
    Right,
}

/// The four borders of a cell (or the [`TableShape`] default).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TableBorders {
    /// Top edge, if any.
    #[serde(default)]
    pub top: Option<BorderEdge>,
    /// Bottom edge, if any.
    #[serde(default)]
    pub bottom: Option<BorderEdge>,
    /// Left edge, if any.
    #[serde(default)]
    pub left: Option<BorderEdge>,
    /// Right edge, if any.
    #[serde(default)]
    pub right: Option<BorderEdge>,
}

/// A single border edge: color, width, and dash style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BorderEdge {
    /// Edge color.
    pub color: Color,
    /// Width in EMU.
    pub width_emu: f64,
    /// Dash pattern of the edge.
    #[serde(default)]
    pub dash: DashStyle,
}

/// Maximum number of data series in a chart (PRODUCT_SPEC.md §5.2).
pub const MAX_CHART_SERIES: usize = 50;
/// Maximum number of data points per series in a chart (PRODUCT_SPEC.md §5.2).
pub const MAX_CHART_POINTS: usize = 1000;

/// Errors returned by [`ChartShape::new`] and [`ChartShape::validate`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChartError {
    /// The chart must have at least one series.
    #[error("chart must have at least one series")]
    Empty,
    /// The chart exceeds the series cap.
    #[error("chart exceeds the {max} series limit ({got})")]
    TooManySeries {
        /// The number of series in the offending chart.
        got: usize,
        /// The configured series cap ([`MAX_CHART_SERIES`]).
        max: usize,
    },
    /// A series exceeds the per-series point cap.
    #[error("series {index} exceeds the {max} point limit ({got})")]
    TooManyPoints {
        /// Index of the offending series.
        index: usize,
        /// The number of points in the offending series.
        got: usize,
        /// The configured per-series point cap ([`MAX_CHART_POINTS`]).
        max: usize,
    },
    /// A category series must have one value per category.
    #[error("series {index} has {got} values but there are {want} categories")]
    SeriesCategoryMismatch {
        /// Index of the offending series.
        index: usize,
        /// The number of values the series carries.
        got: usize,
        /// The number of categories the chart defines.
        want: usize,
    },
    /// The chart data kind does not match the chart type.
    #[error("chart data kind does not match chart type")]
    DataTypeMismatch,
}

/// The kind of chart, mirroring PRODUCT_SPEC.md §5.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    /// Horizontal bars.
    Bar,
    /// Vertical columns.
    Column,
    /// Connected line segments.
    Line,
    /// Filled area below a line.
    Area,
    /// Pie slices.
    Pie,
    /// Scatter (XY) points.
    Scatter,
}

impl ChartType {
    /// Returns `true` if this type is driven by category data.
    pub fn is_category(&self) -> bool {
        matches!(
            self,
            ChartType::Bar | ChartType::Column | ChartType::Line | ChartType::Area | ChartType::Pie
        )
    }

    /// Returns `true` if this type is driven by XY data.
    pub fn is_xy(&self) -> bool {
        matches!(self, ChartType::Scatter)
    }
}

/// The data backing a chart.
///
/// Category charts ([`ChartType::is_category`]) take [`ChartData::Category`];
/// scatter charts take [`ChartData::XY`]. The two kinds cannot be mixed with a
/// single chart type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ChartData {
    /// Category-aligned data for bar, column, line, area, and pie charts.
    Category {
        /// Category labels, shared across every series.
        categories: Vec<String>,
        /// One or more value series, each aligned with `categories`.
        series: Vec<CategorySeries>,
    },
    /// XY (scatter) data.
    #[serde(rename = "xy")]
    XY {
        /// One or more point series.
        series: Vec<XYSeries>,
    },
}

/// A value series aligned with a set of categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategorySeries {
    /// Series name, shown in the legend.
    #[serde(default)]
    pub name: String,
    /// One numeric value per category.
    pub values: Vec<f64>,
}

/// A series of (x, y) points for scatter charts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XYSeries {
    /// Series name, shown in the legend.
    #[serde(default)]
    pub name: String,
    /// Ordered (x, y) pairs.
    pub points: Vec<XYPoint>,
}

/// A single (x, y) point in an [`XYSeries`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XYPoint {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

impl XYPoint {
    /// Creates a new (x, y) point.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A chart shape placed on a slide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartShape {
    /// Stable identifier for cross-slide matching (Magic Move). Defaults to
    /// empty string so old decks deserialize unchanged.
    #[serde(default)]
    pub id: String,
    /// Position, size, and rotation of the chart.
    pub transform: Transform,
    /// Kind of chart.
    pub chart_type: ChartType,
    /// Data plotted by the chart.
    pub data: ChartData,
    /// Optional chart title.
    #[serde(default)]
    pub title: Option<String>,
}

impl ChartShape {
    /// Constructs a chart, validating its structural invariants.
    ///
    /// Returns [`ChartError`] if the data has no series, exceeds the series or
    /// per-series point caps, has a category series that does not align with its
    /// categories, or whose kind does not match `chart_type`.
    pub fn new(
        transform: Transform,
        chart_type: ChartType,
        data: ChartData,
        title: Option<String>,
    ) -> Result<Self, ChartError> {
        Self::validate_parts(chart_type, &data)?;
        Ok(Self {
            id: String::new(),
            transform,
            chart_type,
            data,
            title,
        })
    }

    fn validate_parts(chart_type: ChartType, data: &ChartData) -> Result<(), ChartError> {
        match data {
            ChartData::Category { categories, series } => {
                if !chart_type.is_category() {
                    return Err(ChartError::DataTypeMismatch);
                }
                Self::check_series_cap(series.len())?;
                let want = categories.len();
                for (index, s) in series.iter().enumerate() {
                    Self::check_point_cap(index, s.values.len())?;
                    if s.values.len() != want {
                        return Err(ChartError::SeriesCategoryMismatch {
                            index,
                            got: s.values.len(),
                            want,
                        });
                    }
                }
            }
            ChartData::XY { series } => {
                if !chart_type.is_xy() {
                    return Err(ChartError::DataTypeMismatch);
                }
                Self::check_series_cap(series.len())?;
                for (index, s) in series.iter().enumerate() {
                    Self::check_point_cap(index, s.points.len())?;
                }
            }
        }
        Ok(())
    }

    fn check_series_cap(len: usize) -> Result<(), ChartError> {
        if len == 0 {
            Err(ChartError::Empty)
        } else if len > MAX_CHART_SERIES {
            Err(ChartError::TooManySeries {
                got: len,
                max: MAX_CHART_SERIES,
            })
        } else {
            Ok(())
        }
    }

    fn check_point_cap(index: usize, len: usize) -> Result<(), ChartError> {
        if len > MAX_CHART_POINTS {
            Err(ChartError::TooManyPoints {
                index,
                got: len,
                max: MAX_CHART_POINTS,
            })
        } else {
            Ok(())
        }
    }

    /// Returns `true` if the chart satisfies the same invariants as [`new`].
    pub fn validate(&self) -> bool {
        Self::validate_parts(self.chart_type, &self.data).is_ok()
    }
}

/// A rectangle in EMU (English Metric Units; 914400 EMU per inch).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Horizontal position, in EMU.
    pub x: f64,
    /// Vertical position, in EMU.
    pub y: f64,
    /// Width, in EMU.
    pub width: f64,
    /// Height, in EMU.
    pub height: f64,
}

impl Rect {
    /// Creates a new rectangle.
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Placement of a shape: a bounding frame plus a rotation around its center.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    /// Bounding rectangle, in EMU.
    pub frame: Rect,
    /// Rotation around the frame center, in degrees.
    pub rotation: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            frame: Rect::new(0.0, 0.0, 0.0, 0.0),
            rotation: 0.0,
        }
    }
}

/// The geometric primitive a [`GeometricShape`] is built from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Geometry {
    /// A plain rectangle.
    Rectangle,
    /// A rectangle with rounded corners.
    RoundedRectangle {
        /// Corner radius, in EMU.
        radius: f64,
    },
    /// An ellipse (a circle when square).
    Ellipse,
    /// A triangle.
    Triangle,
    /// A straight line.
    Line,
    /// A single-headed arrow.
    Arrow,
    /// A right-arrow callout shape.
    RightArrowCallout,
    /// A five-pointed star.
    Star5,
}

/// Fill applied to a shape's interior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fill {
    /// A single solid color.
    Solid(Color),
}

/// Dash pattern for an [`Outline`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashStyle {
    /// A continuous, unbroken line.
    #[default]
    Solid,
    /// A dashed line.
    Dash,
    /// A dotted line.
    Dot,
    /// An alternating dash-dot line.
    DashDot,
}

/// Outline (stroke) of a shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outline {
    /// Stroke color.
    pub color: Color,
    /// Stroke width, in EMU.
    pub width_emu: f64,
    /// Dash pattern of the stroke.
    pub dash: DashStyle,
}

/// Drop shadow drawn behind a shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    /// Horizontal offset, in EMU.
    pub offset_x: f64,
    /// Vertical offset, in EMU.
    pub offset_y: f64,
    /// Blur radius, in EMU.
    pub blur: f64,
    /// Shadow color.
    pub color: Color,
    /// Shadow opacity, in the range `0.0..=1.0`.
    pub opacity: f64,
}

/// Crop applied to an image, as fractions of its native size in `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    /// Fraction cropped from the left edge.
    pub left: f64,
    /// Fraction cropped from the top edge.
    pub top: f64,
    /// Fraction cropped from the right edge.
    pub right: f64,
    /// Fraction cropped from the bottom edge.
    pub bottom: f64,
}

/// Visual style applied to a [`GeometricShape`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Style {
    /// Interior fill, if any.
    pub fill: Option<Fill>,
    /// Outline (stroke), if any.
    pub outline: Option<Outline>,
    /// Drop shadow, if any.
    pub shadow: Option<Shadow>,
}

/// Standard 16:9 widescreen slide width, in EMU.
const TEMPLATE_SLIDE_WIDTH_EMU: f64 = 12_192_000.0;
/// Standard 16:9 widescreen slide height, in EMU.
const TEMPLATE_SLIDE_HEIGHT_EMU: f64 = 6_858_000.0;

/// A slide master: background layers painted behind every slide in the deck,
/// plus placeholder definitions.
///
/// Additive; old decks deserialize into an empty master via [`Master::default`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Master {
    /// Background shapes (painted first, behind all slide content).
    #[serde(default)]
    pub background_shapes: Vec<BackgroundShape>,
    /// Named placeholders that layouts reference.
    #[serde(default)]
    pub placeholders: Vec<PlaceholderDef>,
}

impl Master {
    /// A blank 16:9 master (no background shapes, no placeholders).
    pub fn default_16_9() -> Self {
        Self::default()
    }
}

/// A simple background shape on a master (rectangles, accents).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackgroundShape {
    /// The geometric shape data.
    pub geometry: Geometry,
    /// Visual style of the background shape.
    pub style: Style,
    /// Position, size, and rotation of the background shape.
    pub transform: Transform,
}

/// A named placeholder (e.g. "title", "content", "footer").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceholderDef {
    /// Placeholder name, referenced by layouts.
    pub name: String,
    /// Bounding rectangle of the placeholder, in EMU.
    pub frame: Rect,
}

/// A named layout variant of the master.
///
/// A layout carries placeholder overrides keyed by placeholder name; the
/// deck's [`Slide`]s reference a layout by name via [`Slide::layout_ref`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Layout name (e.g. "Title Slide", "Title and Content", "Section Header").
    pub name: String,
    /// Placeholder overrides keyed by placeholder name.
    #[serde(default)]
    pub placeholders: Vec<PlaceholderDef>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            name: "Blank".to_string(),
            placeholders: Vec::new(),
        }
    }
}

/// A built-in template definition: a name, a theme, a master, and layouts.
pub struct TemplateDefinition {
    /// Stable template identifier (e.g. "default", "educator").
    pub name: &'static str,
    /// Human-readable template name shown in the picker.
    pub display_name: &'static str,
    /// Theme applied by this template.
    pub theme: Theme,
    /// Slide master applied by this template.
    pub master: Master,
    /// Named layouts available in this template.
    pub layouts: Vec<Layout>,
}

/// The six built-in templates, each with a theme, master, and layouts.
///
/// `TemplateRegistry` holds no state; it is a namespace for the registry
/// accessors [`TemplateRegistry::names`] and [`TemplateRegistry::get`].
pub struct TemplateRegistry;

impl TemplateRegistry {
    /// Returns the names of the six built-in templates, in canonical order.
    pub fn names() -> Vec<&'static str> {
        vec![
            "default",
            "educator",
            "pitch",
            "conference_talk",
            "community_update",
            "photo_essay",
        ]
    }

    /// Returns the named built-in template, if it exists.
    pub fn get(name: &str) -> Option<TemplateDefinition> {
        match name {
            "default" => Some(default_template()),
            "educator" => Some(educator_template()),
            "pitch" => Some(pitch_template()),
            "conference_talk" => Some(conference_talk_template()),
            "community_update" => Some(community_update_template()),
            "photo_essay" => Some(photo_essay_template()),
            _ => None,
        }
    }
}

/// Builds a named placeholder.
fn placeholder(name: &str, frame: Rect) -> PlaceholderDef {
    PlaceholderDef {
        name: name.to_string(),
        frame,
    }
}

/// A standard "title" placeholder frame for 16:9 slides.
fn title_placeholder() -> PlaceholderDef {
    placeholder(
        "title",
        Rect::new(457_200.0, 457_200.0, 11_277_600.0, 1_143_000.0),
    )
}

/// A standard "content" placeholder frame for 16:9 slides.
fn content_placeholder() -> PlaceholderDef {
    placeholder(
        "content",
        Rect::new(457_200.0, 1_828_800.0, 11_277_600.0, 4_343_400.0),
    )
}

/// A standard "footer" placeholder frame for 16:9 slides.
fn footer_placeholder() -> PlaceholderDef {
    placeholder(
        "footer",
        Rect::new(457_200.0, 6_400_800.0, 11_277_600.0, 228_600.0),
    )
}

/// Builds a master with a full-bleed background rectangle, a top accent bar,
/// and the standard title/content/footer placeholders.
fn master_with_accent(accent: Color, background: Color) -> Master {
    Master {
        background_shapes: vec![
            BackgroundShape {
                geometry: Geometry::Rectangle,
                style: Style {
                    fill: Some(Fill::Solid(background)),
                    outline: None,
                    shadow: None,
                },
                transform: Transform {
                    frame: Rect::new(
                        0.0,
                        0.0,
                        TEMPLATE_SLIDE_WIDTH_EMU,
                        TEMPLATE_SLIDE_HEIGHT_EMU,
                    ),
                    rotation: 0.0,
                },
            },
            BackgroundShape {
                geometry: Geometry::Rectangle,
                style: Style {
                    fill: Some(Fill::Solid(accent)),
                    outline: None,
                    shadow: None,
                },
                transform: Transform {
                    frame: Rect::new(0.0, 0.0, TEMPLATE_SLIDE_WIDTH_EMU, 95_250.0),
                    rotation: 0.0,
                },
            },
        ],
        placeholders: vec![
            title_placeholder(),
            content_placeholder(),
            footer_placeholder(),
        ],
    }
}

/// Builds a list of layouts from their names, each with no placeholder
/// overrides (they inherit the master's placeholders).
fn named_layouts(names: &[&str]) -> Vec<Layout> {
    names
        .iter()
        .map(|name| Layout {
            name: (*name).to_string(),
            placeholders: Vec::new(),
        })
        .collect()
}

/// The default template: Calibri, blue accent, white background — matching
/// [`Theme::default`] — with a blank master.
fn default_template() -> TemplateDefinition {
    TemplateDefinition {
        name: "default",
        display_name: "Default",
        theme: Theme::default(),
        master: Master::default(),
        layouts: named_layouts(&["Title Slide", "Title and Content", "Blank"]),
    }
}

/// The Educator template: serif (Georgia), warm cream background, orange
/// accent.
fn educator_template() -> TemplateDefinition {
    let accent = Color::rgb(232, 122, 48);
    let background = Color::rgb(250, 243, 224);
    TemplateDefinition {
        name: "educator",
        display_name: "Educator",
        theme: Theme {
            background,
            heading_font: "Georgia".to_string(),
            body_font: "Georgia".to_string(),
            accent_color: accent,
            high_contrast: false,
        },
        master: master_with_accent(accent, background),
        layouts: named_layouts(&["Lesson Title", "Bulleted Content", "Definition", "Exercise"]),
    }
}

/// The Pitch template: sans-serif (Inter/Helvetica), dark background, teal
/// accent.
fn pitch_template() -> TemplateDefinition {
    let accent = Color::rgb(45, 212, 191);
    let background = Color::rgb(26, 26, 46);
    TemplateDefinition {
        name: "pitch",
        display_name: "Pitch",
        theme: Theme {
            background,
            heading_font: "Inter".to_string(),
            body_font: "Helvetica".to_string(),
            accent_color: accent,
            high_contrast: false,
        },
        master: master_with_accent(accent, background),
        layouts: named_layouts(&["Cover", "Problem/Solution", "Metrics", "Team", "Closing"]),
    }
}

/// The Conference Talk template: monospace headings (JetBrains Mono/Consolas),
/// dark background, green accent.
fn conference_talk_template() -> TemplateDefinition {
    let accent = Color::rgb(63, 185, 80);
    let background = Color::rgb(13, 17, 23);
    TemplateDefinition {
        name: "conference_talk",
        display_name: "Conference Talk",
        theme: Theme {
            background,
            heading_font: "JetBrains Mono".to_string(),
            body_font: "Consolas".to_string(),
            accent_color: accent,
            high_contrast: false,
        },
        master: master_with_accent(accent, background),
        layouts: named_layouts(&["Title", "Code Block", "Big Idea", "Q&A"]),
    }
}

/// The Community Update template: friendly sans (Verdana), light green
/// background, dark green accent.
fn community_update_template() -> TemplateDefinition {
    let accent = Color::rgb(46, 125, 50);
    let background = Color::rgb(232, 245, 225);
    TemplateDefinition {
        name: "community_update",
        display_name: "Community Update",
        theme: Theme {
            background,
            heading_font: "Verdana".to_string(),
            body_font: "Verdana".to_string(),
            accent_color: accent,
            high_contrast: false,
        },
        master: master_with_accent(accent, background),
        layouts: named_layouts(&["Welcome", "Announcements", "Events", "Call to Action"]),
    }
}

/// The Photo Essay template: clean serif (Georgia) headings + sans body, black
/// background, white text, neutral accent (photos are the focus).
fn photo_essay_template() -> TemplateDefinition {
    let accent = Color::rgb(200, 200, 200);
    let background = Color::rgb(0, 0, 0);
    TemplateDefinition {
        name: "photo_essay",
        display_name: "Photo Essay",
        theme: Theme {
            background,
            heading_font: "Georgia".to_string(),
            body_font: "Helvetica".to_string(),
            accent_color: accent,
            high_contrast: false,
        },
        master: master_with_accent(accent, background),
        layouts: named_layouts(&["Cover", "Full Photo", "Captioned Photo", "Gallery"]),
    }
}

/// A paragraph of text, made of runs and a list style.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    /// Inline text runs.
    pub runs: Vec<Run>,
    /// List style for this paragraph.
    pub list_style: ListStyle,
    /// Paragraph-level style (heading, blockquote, code block, indentation).
    #[serde(default)]
    pub style: ParagraphStyle,
}

/// List style of a paragraph.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListStyle {
    /// Plain paragraph with no list marker.
    #[default]
    None,
    /// Numbered list.
    Ordered,
    /// Bulleted list.
    Unordered,
}

/// Heading level for a paragraph style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadingLevel {
    /// Heading level 1.
    H1,
    /// Heading level 2.
    H2,
    /// Heading level 3.
    H3,
    /// Heading level 4.
    H4,
    /// Heading level 5.
    H5,
    /// Heading level 6.
    H6,
}

/// Style block applied to a paragraph.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphStyle {
    /// Heading level, if this paragraph is a heading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<HeadingLevel>,
    /// Whether this paragraph is a block quote.
    #[serde(default)]
    pub blockquote: bool,
    /// Whether this paragraph is a fenced code block.
    #[serde(default)]
    pub code_block: bool,
    /// Stepped code highlighting ranges (e.g. "1-3|4|5,7"). Each pipe-separated
    /// segment is one step; comma-separated entries within a segment are
    /// simultaneous. `None` or empty means no stepping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_step_ranges: Option<String>,
    /// Indentation level of the paragraph.
    #[serde(default)]
    pub indent_level: u32,
}

/// Vertical alignment of a run relative to the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerticalAlign {
    /// Normal baseline alignment.
    #[default]
    Baseline,
    /// Superscript text.
    Superscript,
    /// Subscript text.
    Subscript,
}

/// A validated hyperlink attached to a run.
///
/// The public constructor [`Link::new`] enforces the URL allowlist. Fields are
/// public so the model can be constructed directly, but callers that build
/// links from untrusted input (PPTX loaders, desktop commands) must use
/// [`Link::new`] so the safety boundary is respected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    /// URL target of the link.
    pub url: String,
    /// Optional display text override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
}

/// Errors returned by [`Link::new`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    /// The URL uses a disallowed scheme.
    #[error("disallowed URL scheme: {0}")]
    DisallowedScheme(String),
    /// The URL contains control characters.
    #[error("URL contains control characters")]
    ControlCharacters,
}

impl Link {
    /// Validates `url` against the link allowlist and constructs a Link.
    ///
    /// Rejects:
    /// - `javascript:`, `vbscript:`, `mocha:`, `livescript:`, `file:`,
    ///   and `data:` schemes (case-insensitive prefix),
    /// - any value with a colon before the first slash (i.e. an unknown scheme),
    /// - any control character (U+0000..=U+001F or U+007F).
    ///
    /// Allowed: `http`, `https`, `mailto:`, `tel:`, `#fragment`, and
    /// schemeless relative paths.
    pub fn new(url: impl Into<String>) -> std::result::Result<Self, LinkError> {
        let url = url.into();
        if url.chars().any(|c| {
            let code = c as u32;
            code <= 0x1F || code == 0x7F
        }) {
            return Err(LinkError::ControlCharacters);
        }

        let trimmed = url.trim();
        if trimmed.starts_with('#') {
            return Ok(Self { url, display: None });
        }

        let lowered = trimmed.to_ascii_lowercase();
        const DANGEROUS_SCHEMES: &[&str] = &[
            "javascript:",
            "vbscript:",
            "mocha:",
            "livescript:",
            "file:",
            "data:",
        ];
        if let Some(scheme) = DANGEROUS_SCHEMES.iter().find(|s| lowered.starts_with(*s)) {
            return Err(LinkError::DisallowedScheme(
                scheme.trim_end_matches(':').to_string(),
            ));
        }

        const ALLOWED_SCHEMES: &[&str] = &["https:", "http:", "mailto:", "tel:"];
        if ALLOWED_SCHEMES.iter().any(|s| lowered.starts_with(s)) {
            return Ok(Self { url, display: None });
        }

        if let Some(colon) = lowered.find(':') {
            let before_colon = &lowered[..colon];
            if !before_colon.contains('/') {
                return Err(LinkError::DisallowedScheme(before_colon.to_string()));
            }
        }

        Ok(Self { url, display: None })
    }

    /// Constructs a Link WITHOUT validation. Only for internal use (e.g. the
    /// PPTX loader that needs to preserve an existing link then decide whether
    /// to surface a loss warning). Prefer [`Link::new`] for untrusted input.
    pub fn new_unchecked(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            display: None,
        }
    }
}

/// An inline run of text with formatting.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    /// Text content.
    pub text: String,
    /// Bold formatting.
    pub bold: bool,
    /// Italic formatting.
    pub italic: bool,
    /// Underline formatting.
    pub underline: bool,
    /// Strikethrough formatting.
    #[serde(default)]
    pub strikethrough: bool,
    /// Vertical alignment (superscript, subscript, or baseline).
    #[serde(default)]
    pub vertical_align: VerticalAlign,
    /// Hyperlink attached to this run.
    #[serde(default)]
    pub link: Option<Link>,
    /// Inline code: monospaced, marked run.
    #[serde(default)]
    pub code: bool,
    /// Run-level font family override; used by code.
    #[serde(default)]
    pub font_family: Option<String>,
    /// Glyph (text) color override, used by the accessibility checker for
    /// contrast measurement. Defaults to `None` (`#[serde(default)]`) so decks
    /// serialized before this field existed deserialize unchanged — a
    /// non-breaking, additive change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Font size in EMU (12,700 EMU per point), used by the accessibility
    /// checker to detect small text and to pick the WCAG large-text threshold.
    /// Defaults to `None` (`#[serde(default)]`) so decks serialized before this
    /// field existed deserialize unchanged — a non-breaking, additive change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
}

impl Run {
    /// Creates a plain run of text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            vertical_align: VerticalAlign::Baseline,
            link: None,
            code: false,
            font_family: None,
            color: None,
            font_size: None,
        }
    }

    /// Returns a new run with bold set.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Returns a new run with italic set.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Returns a new run with underline set.
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Returns a new run with strikethrough set.
    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// Returns a new run with superscript vertical alignment.
    pub fn superscript(mut self) -> Self {
        self.vertical_align = VerticalAlign::Superscript;
        self
    }

    /// Returns a new run with subscript vertical alignment.
    pub fn subscript(mut self) -> Self {
        self.vertical_align = VerticalAlign::Subscript;
        self
    }

    /// Returns a new run marked as inline code.
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// Returns a new run with the given font family override.
    pub fn font(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// Returns a new run with the given glyph color override.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Returns a new run with the given font size, in EMU.
    pub fn font_size(mut self, emu: f64) -> Self {
        self.font_size = Some(emu);
        self
    }

    /// Returns a new run with the given validated link.
    ///
    /// Returns an error if the URL fails the link safety allowlist.
    pub fn link(mut self, url: impl Into<String>) -> std::result::Result<Self, LinkError> {
        self.link = Some(Link::new(url)?);
        Ok(self)
    }
}

/// An opaque object preserved byte-for-byte from the source document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassthroughObject {
    /// Identifier from the source object.
    pub id: String,
    /// Human-readable label for the object.
    pub label: String,
    /// Source part path (e.g. `ppt/slides/slide1.xml`).
    pub source_part: String,
    /// Raw XML bytes from the source document.
    pub raw_bytes: Vec<u8>,
    /// Bounding rectangle of the object, in EMU, if it could be parsed.
    pub frame: Option<Rect>,
}

/// Maximum duration of a slide transition, in milliseconds (PRODUCT_SPEC.md §5.2).
pub const MAX_TRANSITION_MS: u32 = 5000;
/// Maximum duration of a single build-in effect, in milliseconds (PRODUCT_SPEC.md §5.2).
pub const MAX_BUILD_STEP_MS: u32 = 3000;

/// A transition played when advancing TO this slide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    /// Kind of transition.
    pub kind: TransitionKind,
    /// Duration in milliseconds. Deterministic. Clamped to `0..=MAX_TRANSITION_MS`.
    pub duration_ms: u32,
}

impl Transition {
    /// Creates a transition, clamping `duration_ms` into `0..=MAX_TRANSITION_MS`.
    pub fn new(kind: TransitionKind, duration_ms: u32) -> Self {
        Self {
            kind,
            duration_ms: duration_ms.min(MAX_TRANSITION_MS),
        }
    }
}

/// The kind of slide-to-slide transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    /// No transition.
    None,
    /// Cross-fade between slides.
    Fade,
    /// Slide the new slide in.
    Slide,
    /// Push the old slide out as the new one enters.
    Push,
    /// Wipe reveal.
    Wipe,
    /// Magic Move: interpolate shapes that share a stable id with the
    /// preceding slide.
    Morph,
}

/// A build-in animation sequence for a slide: an ordered list of steps.
///
/// Each step reveals (or hides) one shape with an effect. Steps fire in order
/// (one presenter click per step), per the v0.2.0 constrained model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    /// Ordered build steps. Step 0 fires on the first build click.
    pub steps: Vec<BuildStep>,
}

impl Animation {
    /// Creates an animation sequence from an ordered list of steps.
    pub fn new(steps: Vec<BuildStep>) -> Self {
        Self { steps }
    }
}

/// When a build step fires.
///
/// Additive with `#[serde(default)]`; old decks (which carry no `trigger`
/// field) deserialize into [`Trigger::OnClick`], preserving the original
/// one-click-per-step behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// The step fires on a presenter click (the original, default behavior).
    #[default]
    OnClick,
    /// The step fires simultaneously with the previous step.
    WithPrevious,
    /// The step fires immediately after the previous step completes.
    AfterPrevious,
}

/// One build-in step targeting a single shape by index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildStep {
    /// Index into `slide.shapes` of the shape this step reveals (or hides).
    pub shape_index: usize,
    /// The reveal or hide effect.
    pub effect: BuildEffect,
    /// Duration of the effect in milliseconds. Deterministic. Clamped to
    /// `0..=MAX_BUILD_STEP_MS`.
    pub duration_ms: u32,
    /// When this step fires. Defaults to [`Trigger::OnClick`] so decks
    /// serialized before this field existed deserialize unchanged — a
    /// non-breaking, additive change.
    #[serde(default)]
    pub trigger: Trigger,
    /// Delay before the effect starts, in milliseconds, after the trigger
    /// fires. Defaults to `0` (`#[serde(default)]`) so old decks deserialize
    /// unchanged.
    #[serde(default)]
    pub delay_ms: u32,
    /// Optional motion path (waypoints in EMU). Only meaningful when
    /// [`effect`](Self::effect) is [`BuildEffect::MotionPath`]. Defaults to
    /// `None` and is skipped when `None` so old decks deserialize and
    /// re-serialize unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_path: Option<Vec<Rect>>,
}

impl BuildStep {
    /// Creates a build step, clamping `duration_ms` into `0..=MAX_BUILD_STEP_MS`.
    pub fn new(shape_index: usize, effect: BuildEffect, duration_ms: u32) -> Self {
        Self {
            shape_index,
            effect,
            duration_ms: duration_ms.min(MAX_BUILD_STEP_MS),
            trigger: Trigger::default(),
            delay_ms: 0,
            motion_path: None,
        }
    }
}

/// The reveal or hide effect for a build step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildEffect {
    /// Fade the shape in (opacity 0 -> 1).
    Fade,
    /// Slide the shape in from the left.
    SlideInLeft,
    /// Slide the shape in from the right.
    SlideInRight,
    /// Slide the shape in from the top.
    SlideInTop,
    /// Slide the shape in from the bottom.
    SlideInBottom,
    /// Toggle visibility hidden -> visible instantly.
    Appear,
    /// Hide a shape that was already visible (opacity 1 -> 0).
    Disappear,
    /// Move the shape along the step's `motion_path` waypoints, in EMU.
    MotionPath,
}

// ===== Local comments (Wave 17) ==============================================

/// A single comment in a thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    /// Unique id (UUID without hyphens).
    pub id: String,
    /// Author name (free text; no account system).
    pub author: String,
    /// Comment body (plain text).
    pub body: String,
    /// ISO 8601 timestamp (UTC).
    pub timestamp: String,
    /// Whether this specific comment is marked resolved (only applies to
    /// the thread root; replies inherit the thread's resolved state).
    #[serde(default)]
    pub resolved: bool,
}

/// A comment thread anchored to a target within the deck.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentThread {
    /// Unique id (UUID without hyphens).
    pub id: String,
    /// Where this thread is anchored.
    pub anchor: CommentAnchor,
    /// The root comment + replies in chronological order.
    pub comments: Vec<Comment>,
    /// Optional assignee (free text name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    /// Whether the entire thread is resolved.
    #[serde(default)]
    pub resolved: bool,
}

/// Where a comment thread is anchored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommentAnchor {
    /// Anchored to a whole slide.
    Slide { slide_id: String },
    /// Anchored to a specific shape (by id).
    Shape { slide_id: String, shape_id: String },
    /// Anchored to a text range within a specific text box.
    TextRange {
        slide_id: String,
        shape_id: String,
        /// Byte offsets into the text box's concatenated text.
        start: usize,
        end: usize,
    },
}

impl CommentAnchor {
    /// Returns the id of the slide this anchor refers to.
    pub fn slide_id(&self) -> &str {
        match self {
            CommentAnchor::Slide { slide_id }
            | CommentAnchor::Shape { slide_id, .. }
            | CommentAnchor::TextRange { slide_id, .. } => slide_id,
        }
    }

    /// Returns `true` if this anchor references a shape on the given slide and
    /// (for text ranges) its offsets are well-formed. The caller is responsible
    /// for first confirming the slide exists.
    fn shape_is_valid(&self, slide: &Slide) -> bool {
        match self {
            CommentAnchor::Slide { .. } => true,
            CommentAnchor::Shape { shape_id, .. } => {
                slide.shapes.iter().any(|shape| shape.id() == shape_id)
            }
            CommentAnchor::TextRange {
                shape_id,
                start,
                end,
                ..
            } => start <= end && slide.shapes.iter().any(|shape| shape.id() == shape_id),
        }
    }
}

/// Returns the current UTC time as an ISO 8601 string (`YYYY-MM-DDTHH:MM:SSZ`).
///
/// Implemented with the standard library only (no `chrono` dependency) using
/// Howard Hinnant's civil-from-days algorithm to convert epoch seconds to a
/// proleptic Gregorian date.
fn iso8601_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let secs_of_day = (secs % 86_400) as u32;

    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// A reversible command that mutates a [`Deck`].
pub trait Command: std::fmt::Debug + Send {
    /// Applies the command to the deck.
    fn apply(&self, deck: &mut Deck);

    /// Returns the inverse command that would restore the deck from its current
    /// state before `apply` was called.
    fn inverse(&self, deck: &Deck) -> Box<dyn Command>;

    /// Returns an estimate of the serialized size of this command, in bytes.
    fn serialized_size(&self) -> usize;

    /// Returns the ids of slides that this command may mutate.
    fn affected_slide_ids(&self) -> Vec<String> {
        Vec::new()
    }

    /// Validates that the command can be applied to the given deck.
    ///
    /// The default implementation accepts every command; specific commands
    /// should override this to reject invalid indices or shapes.
    fn validate(&self, _deck: &Deck) -> bool {
        true
    }
}

/// Errors returned by [`CommandBus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    /// The transaction exceeded the per-transaction size limit.
    #[error("transaction exceeded the per-transaction size limit")]
    TransactionTooLarge,
    /// The undo history is full.
    #[error("undo history is full")]
    HistoryFull,
    /// The command is invalid for the current deck state.
    #[error("invalid command")]
    InvalidCommand,
}

/// Transactional command bus with bounded undo/redo history.
///
/// The bus does not own the [`Deck`]; it is passed in on each call so the same
/// model can be shared with other layers.
#[derive(Debug, Default)]
pub struct CommandBus {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    total_size: usize,
}

impl CommandBus {
    /// Maximum number of transactions in the undo history.
    pub const MAX_TRANSACTIONS: usize = 100;
    /// Maximum total size of all stored inverse transactions, in bytes.
    pub const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
    /// Maximum size of any single transaction, in bytes.
    pub const MAX_PER_TRANSACTION: usize = 32 * 1024 * 1024;

    /// Applies a command transactionally, producing an inverse and pushing it
    /// onto the undo stack. Clears the redo stack (standard semantics — a new
    /// action invalidates redo history).
    ///
    /// If a bound is exceeded, the deck is left unchanged and an error is
    /// returned. If the command fails validation, it is rejected without
    /// modifying the deck or the history.
    pub fn apply(
        &mut self,
        command: Box<dyn Command>,
        deck: &mut Deck,
    ) -> Result<(), CommandError> {
        if !command.validate(deck) {
            return Err(CommandError::InvalidCommand);
        }

        let inverse = command.inverse(deck);
        let inv_size = inverse.serialized_size();
        let cmd_size = command.serialized_size();

        if cmd_size > Self::MAX_PER_TRANSACTION {
            return Err(CommandError::TransactionTooLarge);
        }
        if self.undo_stack.len() >= Self::MAX_TRANSACTIONS {
            return Err(CommandError::HistoryFull);
        }
        if self.total_size + inv_size > Self::MAX_TOTAL_BYTES {
            return Err(CommandError::HistoryFull);
        }

        command.apply(deck);
        self.undo_stack.push(inverse);
        self.total_size += inv_size;
        self.redo_stack.clear();
        Ok(())
    }

    /// Pops the most recent transaction and applies its inverse, pushing the
    /// forward command onto the redo stack.
    ///
    /// Returns the affected slide ids if a command was undone, or `None` if the
    /// history was empty.
    pub fn undo(&mut self, deck: &mut Deck) -> Option<Vec<String>> {
        let inverse = self.undo_stack.pop()?;
        let affected = inverse.affected_slide_ids();
        self.total_size = self.total_size.saturating_sub(inverse.serialized_size());
        // Recover the forward command for redo BEFORE applying the inverse.
        let redo_cmd = inverse.inverse(deck);
        inverse.apply(deck);
        self.redo_stack.push(redo_cmd);
        Some(affected)
    }

    /// Re-applies the most recently undone command and pushes its inverse back
    /// onto the undo stack.
    ///
    /// Returns the affected slide ids if a command was redone, or `None` if the
    /// redo stack was empty.
    pub fn redo(&mut self, deck: &mut Deck) -> Option<Vec<String>> {
        let forward = self.redo_stack.pop()?;
        let affected = forward.affected_slide_ids();
        let undo_inv = forward.inverse(deck);
        let inv_size = undo_inv.serialized_size();
        forward.apply(deck);
        self.undo_stack.push(undo_inv);
        self.total_size += inv_size;
        Some(affected)
    }

    /// Returns the number of transactions that can currently be undone.
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns the number of transactions that can currently be redone.
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Returns the total serialized size of all stored transactions.
    pub fn total_size(&self) -> usize {
        self.total_size
    }
}

/// Replaces a paragraph's runs in a specific slide's text box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditText {
    slide_id: String,
    shape_index: usize,
    paragraph_index: usize,
    replacement_runs: Vec<Run>,
}

impl EditText {
    /// Creates a new text-edit command.
    pub fn new(
        slide_id: impl Into<String>,
        shape_index: usize,
        paragraph_index: usize,
        replacement_runs: Vec<Run>,
    ) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            paragraph_index,
            replacement_runs,
        }
    }
}

impl Command for EditText {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::TextBox(text_box) = shape else {
            return;
        };
        let Some(paragraph) = text_box.paragraphs.get_mut(self.paragraph_index) else {
            return;
        };
        paragraph.runs = self.replacement_runs.clone();
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let current_runs = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::TextBox(text_box) => text_box.paragraphs.get(self.paragraph_index),
                _ => None,
            })
            .map(|paragraph| paragraph.runs.clone())
            .unwrap_or_default();

        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            paragraph_index: self.paragraph_index,
            replacement_runs: current_runs,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::TextBox(text_box) = shape else {
            return false;
        };
        self.paragraph_index < text_box.paragraphs.len()
    }
}

/// Replaces all paragraphs in a specific slide's text box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditTextBox {
    slide_id: String,
    shape_index: usize,
    replacement_paragraphs: Vec<Paragraph>,
}

impl EditTextBox {
    /// Creates a new text-box edit command.
    pub fn new(
        slide_id: impl Into<String>,
        shape_index: usize,
        replacement_paragraphs: Vec<Paragraph>,
    ) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            replacement_paragraphs,
        }
    }
}

impl Command for EditTextBox {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::TextBox(text_box) = shape else {
            return;
        };
        text_box.paragraphs = self.replacement_paragraphs.clone();
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let current_paragraphs = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::TextBox(text_box) => Some(text_box.paragraphs.clone()),
                _ => None,
            })
            .unwrap_or_default();

        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            replacement_paragraphs: current_paragraphs,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        matches!(shape, Shape::TextBox(_))
    }
}

/// Merge-patch over a single run's style flags.
///
/// Only fields set to `Some(...)` are modified; `None` fields leave the target
/// run unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetRunStyle {
    slide_id: String,
    shape_index: usize,
    paragraph_index: usize,
    run_index: usize,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strikethrough: Option<bool>,
    vertical_align: Option<VerticalAlign>,
    code: Option<bool>,
}

impl SetRunStyle {
    /// Creates a new set-run-style command with all style fields unset.
    pub fn new(
        slide_id: impl Into<String>,
        shape_index: usize,
        paragraph_index: usize,
        run_index: usize,
    ) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            paragraph_index,
            run_index,
            bold: None,
            italic: None,
            underline: None,
            strikethrough: None,
            vertical_align: None,
            code: None,
        }
    }

    /// Sets the bold flag to apply.
    pub fn bold(mut self, bold: bool) -> Self {
        self.bold = Some(bold);
        self
    }

    /// Sets the italic flag to apply.
    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = Some(italic);
        self
    }

    /// Sets the underline flag to apply.
    pub fn underline(mut self, underline: bool) -> Self {
        self.underline = Some(underline);
        self
    }

    /// Sets the strikethrough flag to apply.
    pub fn strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = Some(strikethrough);
        self
    }

    /// Sets vertical alignment to superscript.
    pub fn superscript(mut self) -> Self {
        self.vertical_align = Some(VerticalAlign::Superscript);
        self
    }

    /// Sets vertical alignment to subscript.
    pub fn subscript(mut self) -> Self {
        self.vertical_align = Some(VerticalAlign::Subscript);
        self
    }

    /// Sets vertical alignment to baseline.
    pub fn baseline(mut self) -> Self {
        self.vertical_align = Some(VerticalAlign::Baseline);
        self
    }

    /// Sets the inline-code flag to apply.
    pub fn code(mut self, code: bool) -> Self {
        self.code = Some(code);
        self
    }
}

impl Command for SetRunStyle {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::TextBox(text_box) = shape else {
            return;
        };
        let Some(paragraph) = text_box.paragraphs.get_mut(self.paragraph_index) else {
            return;
        };
        let Some(run) = paragraph.runs.get_mut(self.run_index) else {
            return;
        };
        if let Some(bold) = self.bold {
            run.bold = bold;
        }
        if let Some(italic) = self.italic {
            run.italic = italic;
        }
        if let Some(underline) = self.underline {
            run.underline = underline;
        }
        if let Some(strikethrough) = self.strikethrough {
            run.strikethrough = strikethrough;
        }
        if let Some(vertical_align) = self.vertical_align {
            run.vertical_align = vertical_align;
        }
        if let Some(code) = self.code {
            run.code = code;
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let snapshot = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::TextBox(text_box) => text_box.paragraphs.get(self.paragraph_index),
                _ => None,
            })
            .and_then(|paragraph| paragraph.runs.get(self.run_index));

        let mut inverse = Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            paragraph_index: self.paragraph_index,
            run_index: self.run_index,
            bold: None,
            italic: None,
            underline: None,
            strikethrough: None,
            vertical_align: None,
            code: None,
        };
        if let Some(run) = snapshot {
            if self.bold.is_some() {
                inverse.bold = Some(run.bold);
            }
            if self.italic.is_some() {
                inverse.italic = Some(run.italic);
            }
            if self.underline.is_some() {
                inverse.underline = Some(run.underline);
            }
            if self.strikethrough.is_some() {
                inverse.strikethrough = Some(run.strikethrough);
            }
            if self.vertical_align.is_some() {
                inverse.vertical_align = Some(run.vertical_align);
            }
            if self.code.is_some() {
                inverse.code = Some(run.code);
            }
        }
        Box::new(inverse)
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::TextBox(text_box) = shape else {
            return false;
        };
        let Some(paragraph) = text_box.paragraphs.get(self.paragraph_index) else {
            return false;
        };
        self.run_index < paragraph.runs.len()
    }
}

/// Replaces the paragraph-level style of a specific paragraph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetParagraphStyle {
    slide_id: String,
    shape_index: usize,
    paragraph_index: usize,
    style: ParagraphStyle,
}

impl SetParagraphStyle {
    /// Creates a new set-paragraph-style command.
    pub fn new(
        slide_id: impl Into<String>,
        shape_index: usize,
        paragraph_index: usize,
        style: ParagraphStyle,
    ) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            paragraph_index,
            style,
        }
    }
}

impl Command for SetParagraphStyle {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::TextBox(text_box) = shape else {
            return;
        };
        let Some(paragraph) = text_box.paragraphs.get_mut(self.paragraph_index) else {
            return;
        };
        paragraph.style = self.style.clone();
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let style = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::TextBox(text_box) => text_box
                    .paragraphs
                    .get(self.paragraph_index)
                    .map(|p| p.style.clone()),
                _ => None,
            })
            .unwrap_or_default();
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            paragraph_index: self.paragraph_index,
            style,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::TextBox(text_box) = shape else {
            return false;
        };
        self.paragraph_index < text_box.paragraphs.len()
    }
}

/// Counts how many image shapes across the deck reference `key`.
fn count_media_refs(deck: &Deck, key: &str) -> usize {
    deck.slides
        .iter()
        .flat_map(|slide| slide.shapes.iter())
        .filter_map(|shape| match shape {
            Shape::Image(image) => Some(&image.media_ref),
            _ => None,
        })
        .filter(|media_ref| *media_ref == key)
        .count()
}

/// Estimates the serialized size of a [`MediaEntry`] without round-tripping its
/// `bytes` through JSON (which would serialize `Vec<u8>` as a 4x-larger integer
/// array and blow up undo-budget accounting for image commands).
fn media_entry_size(entry: &MediaEntry) -> usize {
    // bytes are serialized as their real length; the rest is a small fixed
    // overhead for the JSON envelope, mime string, and numeric fields.
    entry.bytes.len() + entry.mime.len() + 64
}

/// Appends a shape onto the end of a slide's shape list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddShape {
    slide_id: String,
    shape: Shape,
}

impl AddShape {
    /// Creates a new add-shape command.
    pub fn new(slide_id: impl Into<String>, shape: Shape) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape,
        }
    }
}

impl Command for AddShape {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.shapes.push(self.shape.clone());
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let shape_index = deck
            .slide(&self.slide_id)
            .map_or(0, |slide| slide.shapes.len());
        Box::new(DeleteShape::new(self.slide_id.clone(), shape_index))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id).is_some()
    }
}

/// Removes the shape at a given index from a slide.
///
/// When the removed shape is an image whose media key is no longer referenced
/// by any other shape on the deck, the orphaned [`MediaEntry`] is also removed
/// from the deck's [`MediaStore`]; the inverse [`InsertShapeAt`] restores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteShape {
    slide_id: String,
    shape_index: usize,
}

impl DeleteShape {
    /// Creates a new delete-shape command.
    pub fn new(slide_id: impl Into<String>, shape_index: usize) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
        }
    }
}

impl Command for DeleteShape {
    fn apply(&self, deck: &mut Deck) {
        let removed = deck.slide_mut(&self.slide_id).and_then(|slide| {
            if self.shape_index < slide.shapes.len() {
                Some(slide.shapes.remove(self.shape_index))
            } else {
                None
            }
        });
        let Some(media_ref) = removed.as_ref().and_then(|shape| match shape {
            Shape::Image(image) => Some(image.media_ref.clone()),
            _ => None,
        }) else {
            return;
        };
        if count_media_refs(deck, &media_ref) == 0 {
            deck.media.remove(&media_ref);
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let shape = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .cloned();
        let media = shape.as_ref().and_then(|shape| match shape {
            Shape::Image(image) if count_media_refs(deck, &image.media_ref) <= 1 => deck
                .media
                .get(&image.media_ref)
                .map(|entry| (image.media_ref.clone(), entry.clone())),
            _ => None,
        });
        let shape = shape.unwrap_or_else(|| {
            Shape::Geometric(GeometricShape {
                id: String::new(),
                transform: Transform::default(),
                geometry: Geometry::Rectangle,
                style: Style::default(),
            })
        });
        Box::new(InsertShapeAt {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            shape,
            media,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id)
            .is_some_and(|slide| self.shape_index < slide.shapes.len())
    }
}

/// Re-inserts a shape (and, when captured, a media entry) at a given index.
///
/// This is the inverse of [`DeleteShape`]; it is not normally constructed
/// directly by callers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsertShapeAt {
    slide_id: String,
    shape_index: usize,
    shape: Shape,
    /// Media entry restored alongside the shape, when the deleted image was the
    /// sole reference to its key.
    media: Option<(String, MediaEntry)>,
}

impl Command for InsertShapeAt {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            if self.shape_index <= slide.shapes.len() {
                slide.shapes.insert(self.shape_index, self.shape.clone());
            }
        }
        if let Some((key, entry)) = &self.media {
            deck.media.insert(key.clone(), entry.clone());
        }
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(DeleteShape::new(self.slide_id.clone(), self.shape_index))
    }

    fn serialized_size(&self) -> usize {
        // Account the optional media entry's bytes directly instead of
        // serializing Vec<u8> as a 4x-larger JSON integer array.
        self.slide_id.len()
            + self
                .media
                .as_ref()
                .map(|(key, entry)| key.len() + media_entry_size(entry))
                .unwrap_or(0)
            + 128
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id).is_some()
    }
}

/// Sets the transform (or frame, for text boxes) of a shape on a slide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveShape {
    slide_id: String,
    shape_index: usize,
    transform: Transform,
}

impl MoveShape {
    /// Creates a new move-shape command.
    pub fn new(slide_id: impl Into<String>, shape_index: usize, transform: Transform) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            transform,
        }
    }
}

impl Command for MoveShape {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        match shape {
            Shape::TextBox(text_box) => text_box.frame = self.transform.frame,
            Shape::Image(image) => image.transform = self.transform,
            Shape::Geometric(geometric) => geometric.transform = self.transform,
            Shape::Table(table) => table.transform = self.transform,
            Shape::Chart(chart) => chart.transform = self.transform,
            Shape::Passthrough(_) => {}
        };
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::TextBox(text_box) => Some(Transform {
                    frame: text_box.frame,
                    rotation: 0.0,
                }),
                Shape::Image(image) => Some(image.transform),
                Shape::Geometric(geometric) => Some(geometric.transform),
                Shape::Table(table) => Some(table.transform),
                Shape::Chart(chart) => Some(chart.transform),
                Shape::Passthrough(_) => None,
            })
            .unwrap_or_default();
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            transform: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        !matches!(shape, Shape::Passthrough(_))
    }
}

/// Sets the style of a geometric shape on a slide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetShapeStyle {
    slide_id: String,
    shape_index: usize,
    style: Style,
}

impl SetShapeStyle {
    /// Creates a new set-shape-style command.
    pub fn new(slide_id: impl Into<String>, shape_index: usize, style: Style) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            style,
        }
    }
}

impl Command for SetShapeStyle {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        if let Shape::Geometric(geometric) = shape {
            geometric.style = self.style.clone();
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::Geometric(geometric) => Some(geometric.style.clone()),
                _ => None,
            })
            .unwrap_or_default();
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            style: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        matches!(shape, Shape::Geometric(_))
    }
}

/// Inserts a media entry (if the key is new) and appends an image shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsertImage {
    slide_id: String,
    media_key: String,
    entry: MediaEntry,
    transform: Transform,
    crop: Option<Crop>,
}

impl InsertImage {
    /// Creates a new insert-image command.
    pub fn new(
        slide_id: impl Into<String>,
        media_key: impl Into<String>,
        entry: MediaEntry,
        transform: Transform,
        crop: Option<Crop>,
    ) -> Self {
        Self {
            slide_id: slide_id.into(),
            media_key: media_key.into(),
            entry,
            transform,
            crop,
        }
    }
}

impl Command for InsertImage {
    fn apply(&self, deck: &mut Deck) {
        if !deck.media.contains_key(&self.media_key) {
            deck.media
                .insert(self.media_key.clone(), self.entry.clone());
        }
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.shapes.push(Shape::Image(ImageShape {
                id: Shape::generate_id(),
                transform: self.transform,
                media_ref: self.media_key.clone(),
                crop: self.crop.clone(),
                alt_text: None,
            }));
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let shape_index = deck
            .slide(&self.slide_id)
            .map_or(0, |slide| slide.shapes.len());
        let remove_media_key =
            (!deck.media.contains_key(&self.media_key)).then(|| self.media_key.clone());
        Box::new(RemoveInsertedImage {
            slide_id: self.slide_id.clone(),
            shape_index,
            remove_media_key,
        })
    }

    fn serialized_size(&self) -> usize {
        // Account media bytes directly instead of serializing Vec<u8> as a
        // 4x-larger JSON integer array, which would blow up undo-budget
        // accounting for image inserts.
        self.slide_id.len() + self.media_key.len() + media_entry_size(&self.entry) + 128
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id).is_some()
    }
}

/// Inverse of [`InsertImage`]: removes the appended image shape and, when the
/// original command added the media key, removes that media entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RemoveInsertedImage {
    slide_id: String,
    shape_index: usize,
    /// Media key removed by the original [`InsertImage`], if it was new.
    remove_media_key: Option<String>,
}

impl Command for RemoveInsertedImage {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            if self.shape_index < slide.shapes.len() {
                slide.shapes.remove(self.shape_index);
            }
        }
        if let Some(key) = &self.remove_media_key {
            deck.media.remove(key);
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let Some(Shape::Image(image)) = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
        else {
            return Box::new(RemoveInsertedImage {
                slide_id: self.slide_id.clone(),
                shape_index: self.shape_index,
                remove_media_key: None,
            });
        };
        let Some(entry) = self
            .remove_media_key
            .as_ref()
            .and_then(|key| deck.media.get(key))
            .cloned()
        else {
            return Box::new(RemoveInsertedImage {
                slide_id: self.slide_id.clone(),
                shape_index: self.shape_index,
                remove_media_key: None,
            });
        };
        Box::new(InsertImage::new(
            self.slide_id.clone(),
            image.media_ref.clone(),
            entry,
            image.transform,
            image.crop.clone(),
        ))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, _deck: &Deck) -> bool {
        true
    }
}

/// Appends a [`Shape::Table`] onto the end of a slide's shape list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddTable {
    slide_id: String,
    table: TableShape,
}

impl AddTable {
    /// Creates a new add-table command.
    pub fn new(slide_id: impl Into<String>, table: TableShape) -> Self {
        Self {
            slide_id: slide_id.into(),
            table,
        }
    }
}

impl Command for AddTable {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.shapes.push(Shape::Table(self.table.clone()));
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let shape_index = deck
            .slide(&self.slide_id)
            .map_or(0, |slide| slide.shapes.len());
        Box::new(DeleteShape::new(self.slide_id.clone(), shape_index))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id).is_some() && self.table.validate()
    }
}

/// Sets the plain-text content of a single cell in a table shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetCellText {
    slide_id: String,
    shape_index: usize,
    row: usize,
    col: usize,
    text: String,
}

impl SetCellText {
    /// Creates a new set-cell-text command.
    pub fn new(
        slide_id: impl Into<String>,
        shape_index: usize,
        row: usize,
        col: usize,
        text: impl Into<String>,
    ) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            row,
            col,
            text: text.into(),
        }
    }
}

impl Command for SetCellText {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::Table(table) = shape else {
            return;
        };
        if let Some(cell) = table.cell_mut(self.row, self.col) {
            cell.text = self.text.clone();
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::Table(table) => table.cell(self.row, self.col).map(|cell| cell.text.clone()),
                _ => None,
            })
            .unwrap_or_default();
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            row: self.row,
            col: self.col,
            text: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::Table(table) = shape else {
            return false;
        };
        self.row < table.row_count() && self.col < table.col_count()
    }
}

/// Merge-patch over a single cell's style in a table shape.
///
/// The outer [`Option`] on `fill` and `borders` indicates whether this command
/// touches that field; the inner [`Option`] is the field's new value, where
/// `None` clears it. `align` is a single [`Option`] since it has no "cleared"
/// state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetCellStyle {
    slide_id: String,
    shape_index: usize,
    row: usize,
    col: usize,
    fill: Option<Option<Fill>>,
    borders: Option<Option<TableBorders>>,
    align: Option<CellAlign>,
}

impl SetCellStyle {
    /// Creates a new set-cell-style command with no fields set.
    pub fn new(slide_id: impl Into<String>, shape_index: usize, row: usize, col: usize) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            row,
            col,
            fill: None,
            borders: None,
            align: None,
        }
    }

    /// Sets the cell fill to apply (`None` clears the fill).
    pub fn fill(mut self, fill: Option<Fill>) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Sets the cell borders to apply (`None` clears the borders).
    pub fn borders(mut self, borders: Option<TableBorders>) -> Self {
        self.borders = Some(borders);
        self
    }

    /// Sets the cell text alignment to apply.
    pub fn align(mut self, align: CellAlign) -> Self {
        self.align = Some(align);
        self
    }
}

impl Command for SetCellStyle {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::Table(table) = shape else {
            return;
        };
        let Some(cell) = table.cell_mut(self.row, self.col) else {
            return;
        };
        if let Some(fill) = self.fill.clone() {
            cell.fill = fill;
        }
        if let Some(borders) = self.borders.clone() {
            cell.borders = borders;
        }
        if let Some(align) = self.align {
            cell.align = align;
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let snapshot = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::Table(table) => table.cell(self.row, self.col),
                _ => None,
            });

        let mut inverse = Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            row: self.row,
            col: self.col,
            fill: None,
            borders: None,
            align: None,
        };
        if let Some(cell) = snapshot {
            if self.fill.is_some() {
                inverse.fill = Some(cell.fill.clone());
            }
            if self.borders.is_some() {
                inverse.borders = Some(cell.borders.clone());
            }
            if self.align.is_some() {
                inverse.align = Some(cell.align);
            }
        }
        Box::new(inverse)
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::Table(table) = shape else {
            return false;
        };
        self.row < table.row_count() && self.col < table.col_count()
    }
}

/// Resizes a table shape's column widths and row heights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResizeTable {
    slide_id: String,
    shape_index: usize,
    column_widths: Vec<f64>,
    row_heights: Vec<f64>,
}

impl ResizeTable {
    /// Creates a new resize-table command.
    pub fn new(
        slide_id: impl Into<String>,
        shape_index: usize,
        column_widths: Vec<f64>,
        row_heights: Vec<f64>,
    ) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            column_widths,
            row_heights,
        }
    }
}

impl Command for ResizeTable {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::Table(table) = shape else {
            return;
        };
        if self.column_widths.len() == table.col_count() {
            table.column_widths = self.column_widths.clone();
        }
        if self.row_heights.len() == table.row_count() {
            for (row, height) in table.rows.iter_mut().zip(self.row_heights.iter()) {
                row.height = *height;
            }
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let (prior_widths, prior_heights) = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::Table(table) => Some((
                    table.column_widths.clone(),
                    table.rows.iter().map(|row| row.height).collect(),
                )),
                _ => None,
            })
            .unwrap_or_default();
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            column_widths: prior_widths,
            row_heights: prior_heights,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::Table(table) = shape else {
            return false;
        };
        self.column_widths.len() == table.col_count()
            && self.row_heights.len() == table.row_count()
            && table.row_count() <= MAX_TABLE_ROWS
            && table.col_count() <= MAX_TABLE_COLS
    }
}

/// Inserts a row into a table shape at a given index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsertRow {
    slide_id: String,
    shape_index: usize,
    index: usize,
    row: TableRow,
}

impl InsertRow {
    /// Creates a new insert-row command.
    pub fn new(
        slide_id: impl Into<String>,
        shape_index: usize,
        index: usize,
        row: TableRow,
    ) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            index,
            row,
        }
    }
}

impl Command for InsertRow {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::Table(table) = shape else {
            return;
        };
        if self.index <= table.rows.len() {
            table.rows.insert(self.index, self.row.clone());
        }
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(DeleteRow::new(
            self.slide_id.clone(),
            self.shape_index,
            self.index,
        ))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::Table(table) = shape else {
            return false;
        };
        self.index <= table.row_count()
            && self.row.cells.len() == table.col_count()
            && table.row_count() < MAX_TABLE_ROWS
    }
}

/// Removes a row from a table shape at a given index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteRow {
    slide_id: String,
    shape_index: usize,
    index: usize,
}

impl DeleteRow {
    /// Creates a new delete-row command.
    pub fn new(slide_id: impl Into<String>, shape_index: usize, index: usize) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            index,
        }
    }
}

impl Command for DeleteRow {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::Table(table) = shape else {
            return;
        };
        if self.index < table.rows.len() {
            table.rows.remove(self.index);
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let row = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::Table(table) => table.rows.get(self.index).cloned(),
                _ => None,
            })
            .unwrap_or_else(|| TableRow {
                height: 0.0,
                cells: Vec::new(),
            });
        Box::new(InsertRow::new(
            self.slide_id.clone(),
            self.shape_index,
            self.index,
            row,
        ))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::Table(table) = shape else {
            return false;
        };
        table.row_count() > 1 && self.index < table.row_count()
    }
}

/// Inserts a column into a table shape at a given index.
///
/// Each row gains a new empty cell at `index`, and `column_widths` gains
/// `width` at `index`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsertColumn {
    slide_id: String,
    shape_index: usize,
    index: usize,
    width: f64,
}

impl InsertColumn {
    /// Creates a new insert-column command.
    pub fn new(slide_id: impl Into<String>, shape_index: usize, index: usize, width: f64) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            index,
            width,
        }
    }
}

impl Command for InsertColumn {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::Table(table) = shape else {
            return;
        };
        if self.index > table.col_count() {
            return;
        }
        for row in &mut table.rows {
            if self.index <= row.cells.len() {
                row.cells.insert(self.index, TableCell::default());
            }
        }
        if self.index <= table.column_widths.len() {
            table.column_widths.insert(self.index, self.width);
        }
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(DeleteColumn::new(
            self.slide_id.clone(),
            self.shape_index,
            self.index,
        ))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::Table(table) = shape else {
            return false;
        };
        self.index <= table.col_count() && table.col_count() < MAX_TABLE_COLS
    }
}

/// Removes a column from a table shape at a given index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteColumn {
    slide_id: String,
    shape_index: usize,
    index: usize,
}

impl DeleteColumn {
    /// Creates a new delete-column command.
    pub fn new(slide_id: impl Into<String>, shape_index: usize, index: usize) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            index,
        }
    }
}

impl Command for DeleteColumn {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::Table(table) = shape else {
            return;
        };
        if self.index >= table.col_count() {
            return;
        }
        for row in &mut table.rows {
            if self.index < row.cells.len() {
                row.cells.remove(self.index);
            }
        }
        if self.index < table.column_widths.len() {
            table.column_widths.remove(self.index);
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let table = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::Table(table) => Some(table),
                _ => None,
            });
        let (cells, width) = match table {
            Some(table) if self.index < table.col_count() => {
                let cells = table
                    .rows
                    .iter()
                    .map(|row| row.cells.get(self.index).cloned().unwrap_or_default())
                    .collect::<Vec<_>>();
                let width = table.column_widths.get(self.index).copied().unwrap_or(0.0);
                (cells, width)
            }
            _ => (Vec::new(), 0.0),
        };
        Box::new(RestoreColumn {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            index: self.index,
            cells,
            width,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::Table(table) = shape else {
            return false;
        };
        table.col_count() > 1 && self.index < table.col_count()
    }
}

/// Inverse of [`DeleteColumn`]: re-inserts a captured column (cells plus width)
/// at a given index.
///
/// This carries the exact cells removed by [`DeleteColumn`] so undo restores
/// the full column, including cell text and style. It is not normally
/// constructed directly by callers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RestoreColumn {
    slide_id: String,
    shape_index: usize,
    index: usize,
    cells: Vec<TableCell>,
    width: f64,
}

impl Command for RestoreColumn {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        let Shape::Table(table) = shape else {
            return;
        };
        if self.index > table.col_count() {
            return;
        }
        for (row, cell) in table.rows.iter_mut().zip(self.cells.iter()) {
            if self.index <= row.cells.len() {
                row.cells.insert(self.index, cell.clone());
            }
        }
        if self.index <= table.column_widths.len() {
            table.column_widths.insert(self.index, self.width);
        }
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(DeleteColumn::new(
            self.slide_id.clone(),
            self.shape_index,
            self.index,
        ))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, _deck: &Deck) -> bool {
        true
    }
}

/// Appends a [`Shape::Chart`] onto the end of a slide's shape list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddChart {
    slide_id: String,
    chart: ChartShape,
}

impl AddChart {
    /// Creates a new add-chart command.
    pub fn new(slide_id: impl Into<String>, chart: ChartShape) -> Self {
        Self {
            slide_id: slide_id.into(),
            chart,
        }
    }
}

impl Command for AddChart {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.shapes.push(Shape::Chart(self.chart.clone()));
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let shape_index = deck
            .slide(&self.slide_id)
            .map_or(0, |slide| slide.shapes.len());
        Box::new(DeleteShape::new(self.slide_id.clone(), shape_index))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id).is_some() && self.chart.validate()
    }
}

/// Sets the [`ChartType`] of a chart shape.
///
/// Validation rejects a type whose data kind does not match the chart's
/// existing data (for example, switching a category chart to scatter without
/// also swapping its data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetChartType {
    slide_id: String,
    shape_index: usize,
    chart_type: ChartType,
}

impl SetChartType {
    /// Creates a new set-chart-type command.
    pub fn new(slide_id: impl Into<String>, shape_index: usize, chart_type: ChartType) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            chart_type,
        }
    }
}

impl Command for SetChartType {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        if let Shape::Chart(chart) = shape {
            chart.chart_type = self.chart_type;
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::Chart(chart) => Some(chart.chart_type),
                _ => None,
            })
            .unwrap_or(self.chart_type);
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            chart_type: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::Chart(chart) = shape else {
            return false;
        };
        ChartShape::validate_parts(self.chart_type, &chart.data).is_ok()
    }
}

/// Replaces the [`ChartData`] of a chart shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetChartData {
    slide_id: String,
    shape_index: usize,
    data: ChartData,
}

impl SetChartData {
    /// Creates a new set-chart-data command.
    pub fn new(slide_id: impl Into<String>, shape_index: usize, data: ChartData) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            data,
        }
    }
}

impl Command for SetChartData {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        if let Shape::Chart(chart) = shape {
            chart.data = self.data.clone();
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::Chart(chart) => Some(chart.data.clone()),
                _ => None,
            })
            .unwrap_or_else(|| self.data.clone());
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            data: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let Shape::Chart(chart) = shape else {
            return false;
        };
        ChartShape::validate_parts(chart.chart_type, &self.data).is_ok()
    }
}

/// Sets the optional title of a chart shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetChartTitle {
    slide_id: String,
    shape_index: usize,
    title: Option<String>,
}

impl SetChartTitle {
    /// Creates a new set-chart-title command.
    pub fn new(slide_id: impl Into<String>, shape_index: usize, title: Option<String>) -> Self {
        Self {
            slide_id: slide_id.into(),
            shape_index,
            title,
        }
    }
}

impl Command for SetChartTitle {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        if let Shape::Chart(chart) = shape {
            chart.title = self.title.clone();
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                Shape::Chart(chart) => chart.title.clone(),
                _ => None,
            });
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            title: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        matches!(shape, Shape::Chart(_))
    }
}

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Sets or clears the transition played when advancing to a slide.
///
/// Inverse snapshots the slide's prior `Option<Transition>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetTransition {
    slide_id: String,
    transition: Option<Transition>,
}

impl SetTransition {
    /// Creates a new set-transition command. Pass `None` to clear the transition.
    pub fn new(slide_id: impl Into<String>, transition: Option<Transition>) -> Self {
        Self {
            slide_id: slide_id.into(),
            transition,
        }
    }
}

impl Command for SetTransition {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.transition = self.transition.clone();
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.transition.clone());
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            transition: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id).is_some()
    }
}

/// Replaces or clears the entire build-in animation sequence for a slide.
///
/// Inverse snapshots the slide's prior `Option<Animation>`. If the supplied
/// animation is `Some`, every step's `shape_index` must target an existing
/// shape, or the whole command is rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetSlideAnimation {
    slide_id: String,
    animation: Option<Animation>,
}

impl SetSlideAnimation {
    /// Creates a new set-animation command. Pass `None` to clear the sequence.
    pub fn new(slide_id: impl Into<String>, animation: Option<Animation>) -> Self {
        Self {
            slide_id: slide_id.into(),
            animation,
        }
    }
}

impl Command for SetSlideAnimation {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.animation = self.animation.clone();
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.animation.clone());
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            animation: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(animation) = &self.animation else {
            return true;
        };
        animation
            .steps
            .iter()
            .all(|step| step.shape_index < slide.shapes.len())
    }
}

/// Appends a single build step to a slide's animation sequence.
///
/// If the slide has no animation yet, one is created. The inverse is
/// [`RemoveBuildStepAt`] at the appended position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddBuildStep {
    slide_id: String,
    step: BuildStep,
}

impl AddBuildStep {
    /// Creates a new add-build-step command.
    pub fn new(slide_id: impl Into<String>, step: BuildStep) -> Self {
        Self {
            slide_id: slide_id.into(),
            step,
        }
    }
}

impl Command for AddBuildStep {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let animation = slide
            .animation
            .get_or_insert_with(|| Animation::new(Vec::new()));
        animation.steps.push(self.step.clone());
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        // Computed before apply: the appended position is the current length.
        let index = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.animation.as_ref())
            .map_or(0, |animation| animation.steps.len());
        Box::new(RemoveBuildStepAt::new(self.slide_id.clone(), index))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        self.step.shape_index < slide.shapes.len()
    }
}

/// Removes a build step from a slide's animation by position.
///
/// When the last step is removed, the slide's animation is reset to `None`
/// (the canonical "no steps ⟺ `None`" form), which keeps [`AddBuildStep`]
/// fully reversible. Inverse is [`InsertBuildStepAt`], restoring the removed
/// step at the same position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoveBuildStepAt {
    slide_id: String,
    index: usize,
}

impl RemoveBuildStepAt {
    /// Creates a new remove-build-step command.
    pub fn new(slide_id: impl Into<String>, index: usize) -> Self {
        Self {
            slide_id: slide_id.into(),
            index,
        }
    }
}

impl Command for RemoveBuildStepAt {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(animation) = slide.animation.as_mut() else {
            return;
        };
        if self.index < animation.steps.len() {
            animation.steps.remove(self.index);
            if animation.steps.is_empty() {
                slide.animation = None;
            }
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        // Computed before apply: the step at `index` is still present.
        let step = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.animation.as_ref())
            .and_then(|animation| animation.steps.get(self.index).cloned());
        let step = step.unwrap_or_else(|| BuildStep::new(0, BuildEffect::Appear, 0));
        Box::new(InsertBuildStepAt::new(
            self.slide_id.clone(),
            self.index,
            step,
        ))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(animation) = slide.animation.as_ref() else {
            return false;
        };
        self.index < animation.steps.len()
    }
}

/// Inserts a build step into a slide's animation at a given position.
///
/// Primarily the inverse of [`RemoveBuildStepAt`]. If the slide has no
/// animation yet, one is created. Inverse is [`RemoveBuildStepAt`] at the
/// same position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsertBuildStepAt {
    slide_id: String,
    index: usize,
    step: BuildStep,
}

impl InsertBuildStepAt {
    /// Creates a new insert-build-step command.
    pub fn new(slide_id: impl Into<String>, index: usize, step: BuildStep) -> Self {
        Self {
            slide_id: slide_id.into(),
            index,
            step,
        }
    }
}

impl Command for InsertBuildStepAt {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let animation = slide
            .animation
            .get_or_insert_with(|| Animation::new(Vec::new()));
        let at = self.index.min(animation.steps.len());
        animation.steps.insert(at, self.step.clone());
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(RemoveBuildStepAt::new(self.slide_id.clone(), self.index))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let steps_len = slide
            .animation
            .as_ref()
            .map_or(0, |animation| animation.steps.len());
        self.index <= steps_len && self.step.shape_index < slide.shapes.len()
    }
}

/// Reorders a build step within a slide's animation sequence.
///
/// The step at `from` is removed and re-inserted at `to` (shifting the
/// intervening steps). Inverse moves it back — a [`MoveBuildStep`] with
/// `from` and `to` swapped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveBuildStep {
    slide_id: String,
    from: usize,
    to: usize,
}

impl MoveBuildStep {
    /// Creates a new move-build-step command.
    pub fn new(slide_id: impl Into<String>, from: usize, to: usize) -> Self {
        Self {
            slide_id: slide_id.into(),
            from,
            to,
        }
    }
}

impl Command for MoveBuildStep {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(animation) = slide.animation.as_mut() else {
            return;
        };
        let steps = &mut animation.steps;
        if self.from < steps.len() && self.to < steps.len() {
            let step = steps.remove(self.from);
            steps.insert(self.to, step);
        }
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(MoveBuildStep::new(
            self.slide_id.clone(),
            self.to,
            self.from,
        ))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(animation) = slide.animation.as_ref() else {
            return false;
        };
        self.from < animation.steps.len() && self.to < animation.steps.len()
    }
}

/// Sets the [`Trigger`] of a single build step.
///
/// Inverse snapshots the step's prior `trigger`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetBuildStepTrigger {
    slide_id: String,
    step_index: usize,
    trigger: Trigger,
}

impl SetBuildStepTrigger {
    /// Creates a new set-build-step-trigger command.
    pub fn new(slide_id: impl Into<String>, step_index: usize, trigger: Trigger) -> Self {
        Self {
            slide_id: slide_id.into(),
            step_index,
            trigger,
        }
    }
}

impl Command for SetBuildStepTrigger {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(animation) = slide.animation.as_mut() else {
            return;
        };
        let Some(step) = animation.steps.get_mut(self.step_index) else {
            return;
        };
        step.trigger = self.trigger;
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.animation.as_ref())
            .and_then(|animation| animation.steps.get(self.step_index))
            .map(|step| step.trigger)
            .unwrap_or(self.trigger);
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            step_index: self.step_index,
            trigger: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(animation) = slide.animation.as_ref() else {
            return false;
        };
        self.step_index < animation.steps.len()
    }
}

/// Sets the delay (`delay_ms`) of a single build step.
///
/// Inverse snapshots the step's prior `delay_ms`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetBuildStepDelay {
    slide_id: String,
    step_index: usize,
    delay_ms: u32,
}

impl SetBuildStepDelay {
    /// Creates a new set-build-step-delay command.
    pub fn new(slide_id: impl Into<String>, step_index: usize, delay_ms: u32) -> Self {
        Self {
            slide_id: slide_id.into(),
            step_index,
            delay_ms,
        }
    }
}

impl Command for SetBuildStepDelay {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(animation) = slide.animation.as_mut() else {
            return;
        };
        let Some(step) = animation.steps.get_mut(self.step_index) else {
            return;
        };
        step.delay_ms = self.delay_ms;
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.animation.as_ref())
            .and_then(|animation| animation.steps.get(self.step_index))
            .map(|step| step.delay_ms)
            .unwrap_or(self.delay_ms);
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            step_index: self.step_index,
            delay_ms: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(animation) = slide.animation.as_ref() else {
            return false;
        };
        self.step_index < animation.steps.len()
    }
}

/// Sets or clears the motion path of a single build step.
///
/// Inverse snapshots the step's prior `Option<Vec<Rect>>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetBuildStepMotionPath {
    slide_id: String,
    step_index: usize,
    path: Option<Vec<Rect>>,
}

impl SetBuildStepMotionPath {
    /// Creates a new set-build-step-motion-path command. Pass `None` to clear.
    pub fn new(slide_id: impl Into<String>, step_index: usize, path: Option<Vec<Rect>>) -> Self {
        Self {
            slide_id: slide_id.into(),
            step_index,
            path,
        }
    }
}

impl Command for SetBuildStepMotionPath {
    fn apply(&self, deck: &mut Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(animation) = slide.animation.as_mut() else {
            return;
        };
        let Some(step) = animation.steps.get_mut(self.step_index) else {
            return;
        };
        step.motion_path = self.path.clone();
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.animation.as_ref())
            .and_then(|animation| animation.steps.get(self.step_index))
            .and_then(|step| step.motion_path.clone());
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            step_index: self.step_index,
            path: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(animation) = slide.animation.as_ref() else {
            return false;
        };
        self.step_index < animation.steps.len()
    }
}

/// Sets or clears the per-slide reduce-motion override.
///
/// When `reduce_motion` is `Some(true)`, the presenter renders this slide's
/// build-ins instantly. Inverse snapshots the slide's prior `Option<bool>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetSlideReduceMotion {
    slide_id: String,
    reduce_motion: Option<bool>,
}

impl SetSlideReduceMotion {
    /// Creates a new set-slide-reduce-motion command. Pass `None` to clear the
    /// override (defer to the system preference).
    pub fn new(slide_id: impl Into<String>, reduce_motion: Option<bool>) -> Self {
        Self {
            slide_id: slide_id.into(),
            reduce_motion,
        }
    }
}

impl Command for SetSlideReduceMotion {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.reduce_motion = self.reduce_motion;
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.reduce_motion);
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            reduce_motion: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id).is_some()
    }
}

/// Sets or clears a slide's rehearsed duration (for auto-advance).
/// Inverse snapshots the prior `Option<u32>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetSlideRehearsedDuration {
    slide_id: String,
    duration_ms: Option<u32>,
}

impl SetSlideRehearsedDuration {
    pub fn new(slide_id: impl Into<String>, duration_ms: Option<u32>) -> Self {
        Self {
            slide_id: slide_id.into(),
            duration_ms,
        }
    }
}

impl Command for SetSlideRehearsedDuration {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.rehearsed_duration_ms = self.duration_ms;
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.rehearsed_duration_ms);
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            duration_ms: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id).is_some()
    }
}

/// Sets or clears the deck's slide size (aspect ratio).
///
/// This is a deck-level command: it affects the whole deck rather than a
/// single slide, so `affected_slide_ids` is left at the trait default (no
/// specific slide). Inverse snapshots the deck's prior `Option<SlideSize>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetSlideSize {
    slide_size: Option<SlideSize>,
}

impl SetSlideSize {
    /// Creates a new set-slide-size command. Pass `None` to clear the size.
    pub fn new(slide_size: Option<SlideSize>) -> Self {
        Self { slide_size }
    }
}

impl Command for SetSlideSize {
    fn apply(&self, deck: &mut Deck) {
        deck.slide_size = self.slide_size.clone();
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        Box::new(Self {
            slide_size: deck.slide_size.clone(),
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn validate(&self, _deck: &Deck) -> bool {
        true
    }
}

/// Replaces the deck's entire slide-section list.
///
/// This is a deck-level command: it affects the whole deck rather than a
/// single slide, so `affected_slide_ids` is left at the trait default (no
/// specific slide). Inverse snapshots the deck's prior `Vec<SlideSection>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetSections {
    sections: Vec<SlideSection>,
}

impl SetSections {
    /// Creates a new set-sections command that replaces the whole list.
    pub fn new(sections: Vec<SlideSection>) -> Self {
        Self { sections }
    }
}

impl Command for SetSections {
    fn apply(&self, deck: &mut Deck) {
        deck.sections = self.sections.clone();
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        Box::new(Self {
            sections: deck.sections.clone(),
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn validate(&self, _deck: &Deck) -> bool {
        true
    }
}

/// Sets or clears the rich-text speaker notes for a slide.
///
/// When `rich_notes` is `Some`, the editor uses it; otherwise it uses the
/// slide's plain [`Slide::notes`] field. Inverse snapshots the slide's prior
/// `Option<Vec<Paragraph>>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetRichNotes {
    slide_id: String,
    rich_notes: Option<Vec<Paragraph>>,
}

impl SetRichNotes {
    /// Creates a new set-rich-notes command. Pass `None` to clear rich notes.
    pub fn new(slide_id: impl Into<String>, rich_notes: Option<Vec<Paragraph>>) -> Self {
        Self {
            slide_id: slide_id.into(),
            rich_notes,
        }
    }
}

impl Command for SetRichNotes {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.rich_notes = self.rich_notes.clone();
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.rich_notes.clone());
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            rich_notes: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.slide(&self.slide_id).is_some()
    }
}

/// Sets the deck theme's high-contrast accessibility mode.
///
/// This is a deck-level command: it affects the whole deck's theme rather than
/// a single slide, so `affected_slide_ids` is left at the trait default (no
/// specific slide). Inverse snapshots the theme's prior `high_contrast` bool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetHighContrast {
    high_contrast: bool,
}

impl SetHighContrast {
    /// Creates a new set-high-contrast command.
    pub fn new(high_contrast: bool) -> Self {
        Self { high_contrast }
    }
}

impl Command for SetHighContrast {
    fn apply(&self, deck: &mut Deck) {
        deck.theme.high_contrast = self.high_contrast;
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        Box::new(Self {
            high_contrast: deck.theme.high_contrast,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn validate(&self, _deck: &Deck) -> bool {
        true
    }
}

/// Sets the deck's presenter settings (laser pointer, highlighter).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetPresenterSettings {
    settings: PresenterSettings,
}

impl SetPresenterSettings {
    pub fn new(settings: PresenterSettings) -> Self {
        Self { settings }
    }
}

impl Command for SetPresenterSettings {
    fn apply(&self, deck: &mut Deck) {
        deck.presenter_settings = self.settings.clone();
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        Box::new(Self {
            settings: deck.presenter_settings.clone(),
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn validate(&self, _deck: &Deck) -> bool {
        true
    }
}

/// Applies a built-in template's theme, master, and layouts to the deck.
///
/// This is a deck-level command: it replaces the deck's `theme`, `master`,
/// `layouts`, and `template` name. Validation rejects any name that is not one
/// of the six built-ins ([`TemplateRegistry::names`]). The inverse snapshots
/// the deck's prior theme, master, layouts, and template name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetTemplate {
    template_name: String,
}

impl SetTemplate {
    /// Creates a new set-template command.
    pub fn new(template_name: impl Into<String>) -> Self {
        Self {
            template_name: template_name.into(),
        }
    }
}

impl Command for SetTemplate {
    fn apply(&self, deck: &mut Deck) {
        let Some(definition) = TemplateRegistry::get(&self.template_name) else {
            return;
        };
        deck.theme = definition.theme;
        deck.master = definition.master;
        deck.layouts = definition.layouts;
        deck.template = Some(self.template_name.clone());
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        Box::new(RestoreTemplate {
            theme: deck.theme.clone(),
            master: deck.master.clone(),
            layouts: deck.layouts.clone(),
            template: deck.template.clone(),
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn validate(&self, _deck: &Deck) -> bool {
        TemplateRegistry::names().contains(&self.template_name.as_str())
    }
}

/// Restores the deck's prior theme, master, layouts, and template name.
///
/// This is the inverse of [`SetTemplate`]; it is not normally constructed
/// directly by callers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct RestoreTemplate {
    theme: Theme,
    master: Master,
    layouts: Vec<Layout>,
    template: Option<String>,
}

impl Command for RestoreTemplate {
    fn apply(&self, deck: &mut Deck) {
        deck.theme = self.theme.clone();
        deck.master = self.master.clone();
        deck.layouts = self.layouts.clone();
        deck.template = self.template.clone();
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        Box::new(RestoreTemplate {
            theme: deck.theme.clone(),
            master: deck.master.clone(),
            layouts: deck.layouts.clone(),
            template: deck.template.clone(),
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn validate(&self, _deck: &Deck) -> bool {
        true
    }
}

/// Sets or clears a slide's layout reference.
///
/// Pass `Some(name)` to assign a layout (the name must be one of the deck's
/// [`Deck::layouts`]) or `None` to clear it. The inverse snapshots the slide's
/// prior `Option<String>` layout reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetSlideLayout {
    slide_id: String,
    layout_name: Option<String>,
}

impl SetSlideLayout {
    /// Creates a new set-slide-layout command. Pass `None` to clear the
    /// slide's layout reference.
    pub fn new(slide_id: impl Into<String>, layout_name: Option<String>) -> Self {
        Self {
            slide_id: slide_id.into(),
            layout_name,
        }
    }
}

impl Command for SetSlideLayout {
    fn apply(&self, deck: &mut Deck) {
        if let Some(slide) = deck.slide_mut(&self.slide_id) {
            slide.layout_ref = self.layout_name.clone();
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.layout_ref.clone());
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            layout_name: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        if deck.slide(&self.slide_id).is_none() {
            return false;
        }
        match &self.layout_name {
            None => true,
            Some(name) => deck.layouts.iter().any(|layout| layout.name == *name),
        }
    }
}

// ===== Comment commands (Wave 17) ============================================

/// Creates a new comment thread anchored to a slide, shape, or text range,
/// seeded with a single root comment.
///
/// The thread id and root comment id are generated (UUID, no hyphens) at
/// construction time so [`Command::inverse`] can target the thread by id before
/// [`Command::apply`] runs. Inverse: [`DeleteCommentThread`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddComment {
    anchor: CommentAnchor,
    author: String,
    body: String,
    thread_id: String,
    comment_id: String,
}

impl AddComment {
    /// Creates a new add-comment command for `anchor`, authored by `author`.
    pub fn new(anchor: CommentAnchor, author: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            anchor,
            author: author.into(),
            body: body.into(),
            thread_id: Shape::generate_id(),
            comment_id: Shape::generate_id(),
        }
    }
}

impl Command for AddComment {
    fn apply(&self, deck: &mut Deck) {
        let thread = CommentThread {
            id: self.thread_id.clone(),
            anchor: self.anchor.clone(),
            comments: vec![Comment {
                id: self.comment_id.clone(),
                author: self.author.clone(),
                body: self.body.clone(),
                timestamp: iso8601_now(),
                resolved: false,
            }],
            assigned_to: None,
            resolved: false,
        };
        deck.comments.push(thread);
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(DeleteCommentThread::new(
            self.thread_id.clone(),
            self.anchor.slide_id().to_string(),
        ))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |string| string.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.anchor.slide_id().to_string()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        let Some(slide) = deck.slide(self.anchor.slide_id()) else {
            return false;
        };
        self.anchor.shape_is_valid(slide)
    }
}

/// Appends a reply to an existing comment thread.
///
/// The reply comment id is generated at construction time. Inverse:
/// [`RemoveCommentReply`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplyToComment {
    thread_id: String,
    slide_id: String,
    author: String,
    body: String,
    comment_id: String,
}

impl ReplyToComment {
    /// Creates a new reply command for `thread_id` (which lives on `slide_id`).
    pub fn new(
        thread_id: impl Into<String>,
        slide_id: impl Into<String>,
        author: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            thread_id: thread_id.into(),
            slide_id: slide_id.into(),
            author: author.into(),
            body: body.into(),
            comment_id: Shape::generate_id(),
        }
    }
}

impl Command for ReplyToComment {
    fn apply(&self, deck: &mut Deck) {
        if let Some(thread) = deck.comment_thread_mut(&self.thread_id) {
            thread.comments.push(Comment {
                id: self.comment_id.clone(),
                author: self.author.clone(),
                body: self.body.clone(),
                timestamp: iso8601_now(),
                resolved: false,
            });
        }
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(RemoveCommentReply {
            thread_id: self.thread_id.clone(),
            slide_id: self.slide_id.clone(),
            comment_id: self.comment_id.clone(),
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |string| string.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.comment_thread(&self.thread_id).is_some()
    }
}

/// Removes a single reply (by id) from a thread.
///
/// The inverse of [`ReplyToComment`]; not normally constructed by callers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoveCommentReply {
    thread_id: String,
    slide_id: String,
    comment_id: String,
}

impl Command for RemoveCommentReply {
    fn apply(&self, deck: &mut Deck) {
        if let Some(thread) = deck.comment_thread_mut(&self.thread_id) {
            thread
                .comments
                .retain(|comment| comment.id != self.comment_id);
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let comment = deck
            .comment_thread(&self.thread_id)
            .and_then(|thread| thread.comments.iter().find(|c| c.id == self.comment_id))
            .cloned()
            .unwrap_or(Comment {
                id: self.comment_id.clone(),
                author: String::new(),
                body: String::new(),
                timestamp: String::new(),
                resolved: false,
            });
        Box::new(RestoreCommentReply {
            thread_id: self.thread_id.clone(),
            slide_id: self.slide_id.clone(),
            comment,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |string| string.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.comment_thread(&self.thread_id)
            .is_some_and(|thread| thread.comments.iter().any(|c| c.id == self.comment_id))
    }
}

/// Re-inserts a previously removed reply into a thread.
///
/// The inverse of [`RemoveCommentReply`]; not normally constructed by callers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreCommentReply {
    thread_id: String,
    slide_id: String,
    comment: Comment,
}

impl Command for RestoreCommentReply {
    fn apply(&self, deck: &mut Deck) {
        if let Some(thread) = deck.comment_thread_mut(&self.thread_id) {
            thread.comments.push(self.comment.clone());
        }
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(RemoveCommentReply {
            thread_id: self.thread_id.clone(),
            slide_id: self.slide_id.clone(),
            comment_id: self.comment.id.clone(),
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |string| string.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.comment_thread(&self.thread_id).is_some()
    }
}

/// Sets the resolved flag of a comment thread. Inverse snapshots the prior
/// value (reusing [`Self`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetCommentResolved {
    thread_id: String,
    slide_id: String,
    resolved: bool,
}

impl SetCommentResolved {
    /// Creates a new set-resolved command for `thread_id` (on `slide_id`).
    pub fn new(thread_id: impl Into<String>, slide_id: impl Into<String>, resolved: bool) -> Self {
        Self {
            thread_id: thread_id.into(),
            slide_id: slide_id.into(),
            resolved,
        }
    }
}

impl Command for SetCommentResolved {
    fn apply(&self, deck: &mut Deck) {
        if let Some(thread) = deck.comment_thread_mut(&self.thread_id) {
            thread.resolved = self.resolved;
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .comment_thread(&self.thread_id)
            .is_some_and(|thread| thread.resolved);
        Box::new(Self {
            thread_id: self.thread_id.clone(),
            slide_id: self.slide_id.clone(),
            resolved: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |string| string.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.comment_thread(&self.thread_id).is_some()
    }
}

/// Sets or clears the assignee of a comment thread. Inverse snapshots the prior
/// value (reusing [`Self`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssignComment {
    thread_id: String,
    slide_id: String,
    assignee: Option<String>,
}

impl AssignComment {
    /// Creates a new assign command for `thread_id` (on `slide_id`). Pass
    /// `None` to clear the assignee.
    pub fn new(
        thread_id: impl Into<String>,
        slide_id: impl Into<String>,
        assignee: Option<String>,
    ) -> Self {
        Self {
            thread_id: thread_id.into(),
            slide_id: slide_id.into(),
            assignee,
        }
    }
}

impl Command for AssignComment {
    fn apply(&self, deck: &mut Deck) {
        if let Some(thread) = deck.comment_thread_mut(&self.thread_id) {
            thread.assigned_to = self.assignee.clone();
        }
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let prior = deck
            .comment_thread(&self.thread_id)
            .and_then(|thread| thread.assigned_to.clone());
        Box::new(Self {
            thread_id: self.thread_id.clone(),
            slide_id: self.slide_id.clone(),
            assignee: prior,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |string| string.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.comment_thread(&self.thread_id).is_some()
    }
}

/// Removes a comment thread by id. Inverse: [`RestoreCommentThread`], which
/// snapshots the removed thread and re-inserts it at its original position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteCommentThread {
    thread_id: String,
    slide_id: String,
}

impl DeleteCommentThread {
    /// Creates a new delete-thread command for `thread_id` (on `slide_id`).
    pub fn new(thread_id: impl Into<String>, slide_id: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            slide_id: slide_id.into(),
        }
    }
}

impl Command for DeleteCommentThread {
    fn apply(&self, deck: &mut Deck) {
        deck.comments.retain(|thread| thread.id != self.thread_id);
    }

    fn inverse(&self, deck: &Deck) -> Box<dyn Command> {
        let (index, thread) = deck
            .comments
            .iter()
            .position(|thread| thread.id == self.thread_id)
            .zip(deck.comment_thread(&self.thread_id).cloned())
            .unwrap_or((
                deck.comments.len(),
                CommentThread {
                    id: self.thread_id.clone(),
                    anchor: CommentAnchor::Slide {
                        slide_id: self.slide_id.clone(),
                    },
                    comments: Vec::new(),
                    assigned_to: None,
                    resolved: false,
                },
            ));
        Box::new(RestoreCommentThread {
            index,
            slide_id: self.slide_id.clone(),
            thread,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |string| string.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        deck.comment_thread(&self.thread_id).is_some()
    }
}

/// Re-inserts a previously removed comment thread at its original position.
///
/// The inverse of [`DeleteCommentThread`]; not normally constructed by callers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreCommentThread {
    index: usize,
    slide_id: String,
    thread: CommentThread,
}

impl Command for RestoreCommentThread {
    fn apply(&self, deck: &mut Deck) {
        if self.index <= deck.comments.len() {
            deck.comments.insert(self.index, self.thread.clone());
        }
    }

    fn inverse(&self, _deck: &Deck) -> Box<dyn Command> {
        Box::new(DeleteCommentThread::new(
            self.thread.id.clone(),
            self.slide_id.clone(),
        ))
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |string| string.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &Deck) -> bool {
        self.index <= deck.comments.len() && deck.slide(self.slide_id()).is_some()
    }
}

impl RestoreCommentThread {
    fn slide_id(&self) -> &str {
        self.thread.anchor.slide_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_exist() {
        let _deck = Deck::new();
        let _slide = Slide::default();
        let _shape = Shape::TextBox(TextBox {
            id: String::new(),
            frame: Rect::new(0.0, 0.0, 100.0, 100.0),
            paragraphs: Vec::new(),
        });
        assert!(!version().is_empty());
    }

    #[test]
    fn deck_serializes_and_deserializes() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "slide-1".to_string(),
            notes: "speaker note".to_string(),
            shapes: vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 914_400.0, 457_200.0),
                paragraphs: vec![Paragraph {
                    runs: vec![
                        Run::new("Hello").bold(),
                        Run::new(" world").italic().underline(),
                    ],
                    list_style: ListStyle::Unordered,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });

        let json = serde_json::to_string(&deck).expect("serialize deck");
        let restored: Deck = serde_json::from_str(&json).expect("deserialize deck");
        assert_eq!(deck, restored);
    }

    #[test]
    fn command_bus_applies_and_undoes_edit_text() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("before")],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });

        let mut bus = CommandBus::default();
        let cmd = Box::new(EditText::new("s1", 0, 0, vec![Run::new("after")]));
        bus.apply(cmd, &mut deck).expect("apply should succeed");

        if let Shape::TextBox(tb) = &deck.slides[0].shapes[0] {
            assert_eq!(tb.paragraphs[0].runs[0].text, "after");
        } else {
            panic!("expected text box");
        }

        assert!(bus.undo(&mut deck).is_some());
        if let Shape::TextBox(tb) = &deck.slides[0].shapes[0] {
            assert_eq!(tb.paragraphs[0].runs[0].text, "before");
        } else {
            panic!("expected text box");
        }
    }

    #[test]
    fn command_bus_undo_then_redo_restores() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("before")],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(EditText::new("s1", 0, 0, vec![Run::new("changed")])),
            &mut deck,
        )
        .expect("apply");

        // Undo: deck reverts to original.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(bus.undo_len(), 0);
        assert_eq!(bus.redo_len(), 1);
        assert_eq!(deck, original);

        // Redo: deck goes back to changed state.
        assert!(bus.redo(&mut deck).is_some());
        assert_eq!(bus.undo_len(), 1);
        assert_eq!(bus.redo_len(), 0);
        if let Shape::TextBox(tb) = &deck.slides[0].shapes[0] {
            assert_eq!(tb.paragraphs[0].runs[0].text, "changed");
        }

        // Undo again, then apply a new command → redo stack clears.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(bus.redo_len(), 1);
        bus.apply(
            Box::new(EditText::new("s1", 0, 0, vec![Run::new("new")])),
            &mut deck,
        )
        .expect("apply");
        assert_eq!(bus.redo_len(), 0);
    }

    #[test]
    fn command_bus_rejects_oversized_transaction() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("seed")],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });

        let mut bus = CommandBus::default();
        let huge_text = "x".repeat(CommandBus::MAX_PER_TRANSACTION + 1);
        let cmd = Box::new(EditText::new("s1", 0, 0, vec![Run::new(huge_text)]));
        assert_eq!(
            bus.apply(cmd, &mut deck),
            Err(CommandError::TransactionTooLarge)
        );

        if let Shape::TextBox(tb) = &deck.slides[0].shapes[0] {
            assert_eq!(tb.paragraphs[0].runs[0].text, "seed");
        } else {
            panic!("expected text box");
        }
    }

    #[test]
    fn command_bus_rejects_invalid_edit_text() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("seed")],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });

        let mut bus = CommandBus::default();
        let bad = Box::new(EditText::new("s1", 0, 5, vec![Run::new("after")]));
        assert_eq!(bus.apply(bad, &mut deck), Err(CommandError::InvalidCommand));

        if let Shape::TextBox(tb) = &deck.slides[0].shapes[0] {
            assert_eq!(tb.paragraphs[0].runs[0].text, "seed");
        } else {
            panic!("expected text box");
        }
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn edit_text_box_applies_and_undoes_preserving_formatting() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![
                    Paragraph {
                        runs: vec![Run::new("Hello").bold()],
                        list_style: ListStyle::None,
                        ..Default::default()
                    },
                    Paragraph {
                        runs: vec![Run::new("World").italic()],
                        list_style: ListStyle::None,
                        ..Default::default()
                    },
                ],
            })],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });

        let mut bus = CommandBus::default();
        // Change only the second paragraph; the first should keep its bold run.
        let cmd = Box::new(EditTextBox::new(
            "s1",
            0,
            vec![
                Paragraph {
                    runs: vec![Run::new("Hello").bold()],
                    list_style: ListStyle::None,
                    ..Default::default()
                },
                Paragraph {
                    runs: vec![Run::new("Moon")],
                    list_style: ListStyle::None,
                    ..Default::default()
                },
            ],
        ));
        bus.apply(cmd, &mut deck).expect("apply should succeed");

        if let Shape::TextBox(tb) = &deck.slides[0].shapes[0] {
            assert_eq!(tb.paragraphs.len(), 2);
            assert!(tb.paragraphs[0].runs[0].bold);
            assert_eq!(tb.paragraphs[0].runs[0].text, "Hello");
            assert!(!tb.paragraphs[1].runs[0].bold);
            assert_eq!(tb.paragraphs[1].runs[0].text, "Moon");
        } else {
            panic!("expected text box");
        }

        assert!(bus.undo(&mut deck).is_some());
        if let Shape::TextBox(tb) = &deck.slides[0].shapes[0] {
            assert!(tb.paragraphs[0].runs[0].bold);
            assert_eq!(tb.paragraphs[0].runs[0].text, "Hello");
            assert!(tb.paragraphs[1].runs[0].italic);
            assert_eq!(tb.paragraphs[1].runs[0].text, "World");
        } else {
            panic!("expected text box");
        }
    }

    fn slide_with(id: &str, shapes: Vec<Shape>) -> Slide {
        Slide {
            id: id.to_string(),
            notes: String::new(),
            shapes,
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        }
    }

    fn sample_media_entry() -> MediaEntry {
        MediaEntry {
            mime: "image/png".to_string(),
            bytes: vec![0x89, 0x50, 0x4e, 0x47],
            width: 16,
            height: 16,
        }
    }

    fn geo_rectangle() -> Shape {
        Shape::Geometric(GeometricShape {
            id: String::new(),
            transform: Transform::default(),
            geometry: Geometry::Rectangle,
            style: Style::default(),
        })
    }

    #[test]
    fn defaults_are_canonical() {
        assert_eq!(DashStyle::default(), DashStyle::Solid);
        let style = Style::default();
        assert!(style.fill.is_none());
        assert!(style.outline.is_none());
        assert!(style.shadow.is_none());
        let transform = Transform::default();
        assert_eq!(transform.rotation, 0.0);
        assert_eq!(transform.frame, Rect::new(0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn media_store_basic_operations() {
        let mut store = MediaStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        store.insert("a", sample_media_entry());
        store.insert("b", sample_media_entry());
        assert_eq!(store.len(), 2);
        assert!(store.contains_key("a"));
        assert!(store.get("b").is_some());
        let removed = store.remove("a").expect("remove a");
        assert_eq!(removed.mime, "image/png");
        assert!(!store.contains_key("a"));
        let keys: Vec<&str> = store.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["b"]);
    }

    #[test]
    fn deck_with_image_geometric_and_media_round_trips() {
        let mut deck = Deck::new();
        deck.media.insert(
            "img1".to_string(),
            MediaEntry {
                mime: "image/png".to_string(),
                bytes: vec![0xde, 0xad],
                width: 320,
                height: 240,
            },
        );
        deck.slides.push(slide_with(
            "s1",
            vec![
                Shape::Geometric(GeometricShape {
                    id: String::new(),
                    transform: Transform {
                        frame: Rect::new(100.0, 100.0, 200.0, 200.0),
                        rotation: 12.5,
                    },
                    geometry: Geometry::RoundedRectangle { radius: 4.0 },
                    style: Style {
                        fill: Some(Fill::Solid(Color::rgb(10, 20, 30))),
                        outline: Some(Outline {
                            color: Color::black(),
                            width_emu: 9525.0,
                            dash: DashStyle::DashDot,
                        }),
                        shadow: Some(Shadow {
                            offset_x: 12700.0,
                            offset_y: 12700.0,
                            blur: 25400.0,
                            color: Color::black(),
                            opacity: 0.5,
                        }),
                    },
                }),
                Shape::Image(ImageShape {
                    id: String::new(),
                    transform: Transform {
                        frame: Rect::new(0.0, 0.0, 914_400.0, 685_800.0),
                        rotation: 0.0,
                    },
                    media_ref: "img1".to_string(),
                    crop: Some(Crop {
                        left: 0.1,
                        top: 0.0,
                        right: 0.1,
                        bottom: 0.0,
                    }),
                    alt_text: None,
                }),
            ],
        ));

        let json = serde_json::to_string(&deck).expect("serialize deck");
        let restored: Deck = serde_json::from_str(&json).expect("deserialize deck");
        assert_eq!(deck, restored);
        assert!(json.contains("\"geometric\""));
        assert!(json.contains("\"image\""));
        assert!(json.contains("\"rounded_rectangle\""));
        assert!(json.contains("\"dash_dot\""));
    }

    #[test]
    fn old_deck_without_media_field_deserializes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: Vec::new(),
            })],
        ));

        let mut value = serde_json::to_value(&deck).expect("serialize to value");
        let object = value.as_object_mut().expect("deck serializes to an object");
        assert!(
            object.remove("media").is_some(),
            "deck JSON should carry a media field"
        );

        let old_json = serde_json::to_string(&value).expect("reserialize without media");
        let restored: Deck = serde_json::from_str(&old_json).expect("old deck must load");
        assert!(
            restored.media.is_empty(),
            "missing media field must default to empty"
        );
        assert_eq!(restored.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn add_shape_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(AddShape::new("s1", geo_rectangle())), &mut deck)
            .expect("apply");
        assert_eq!(deck.slides[0].shapes.len(), 1);
        assert!(matches!(deck.slides[0].shapes[0], Shape::Geometric(_)));

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn delete_shape_round_trips_geometric() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![geo_rectangle(), geo_rectangle()]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(DeleteShape::new("s1", 0)), &mut deck)
            .expect("apply");
        assert_eq!(deck.slides[0].shapes.len(), 1);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn delete_shape_removes_orphaned_image_media_and_restores_on_undo() {
        let mut deck = Deck::new();
        deck.media.insert("only".to_string(), sample_media_entry());
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Image(ImageShape {
                id: String::new(),
                transform: Transform::default(),
                media_ref: "only".to_string(),
                crop: None,
                alt_text: None,
            })],
        ));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(DeleteShape::new("s1", 0)), &mut deck)
            .expect("apply");
        assert!(
            !deck.media.contains_key("only"),
            "orphaned media must be removed"
        );
        assert!(deck.slides[0].shapes.is_empty());

        assert!(bus.undo(&mut deck).is_some());
        assert!(
            deck.media.contains_key("only"),
            "orphaned media must be restored"
        );
        assert_eq!(deck, original);
    }

    #[test]
    fn delete_shape_keeps_media_when_still_referenced_elsewhere() {
        let mut deck = Deck::new();
        deck.media
            .insert("shared".to_string(), sample_media_entry());
        let image = || {
            Shape::Image(ImageShape {
                id: String::new(),
                transform: Transform::default(),
                media_ref: "shared".to_string(),
                crop: None,
                alt_text: None,
            })
        };
        deck.slides.push(slide_with("s1", vec![image(), image()]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(DeleteShape::new("s1", 0)), &mut deck)
            .expect("apply first");
        assert!(
            deck.media.contains_key("shared"),
            "still-referenced media must stay"
        );
        assert_eq!(deck.slides[0].shapes.len(), 1);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);

        // Deleting the final reference must drop the media entry.
        bus.apply(Box::new(DeleteShape::new("s1", 0)), &mut deck)
            .expect("apply a");
        bus.apply(Box::new(DeleteShape::new("s1", 0)), &mut deck)
            .expect("apply b");
        assert!(!deck.media.contains_key("shared"));
        assert!(deck.slides[0].shapes.is_empty());
    }

    #[test]
    fn move_shape_applies_and_undoes_across_kinds() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![
                Shape::TextBox(TextBox {
                    id: String::new(),
                    frame: Rect::new(0.0, 0.0, 10.0, 10.0),
                    paragraphs: Vec::new(),
                }),
                Shape::Geometric(GeometricShape {
                    id: String::new(),
                    transform: Transform {
                        frame: Rect::new(0.0, 0.0, 10.0, 10.0),
                        rotation: 0.0,
                    },
                    geometry: Geometry::Rectangle,
                    style: Style::default(),
                }),
                Shape::Image(ImageShape {
                    id: String::new(),
                    transform: Transform {
                        frame: Rect::new(0.0, 0.0, 10.0, 10.0),
                        rotation: 0.0,
                    },
                    media_ref: "x".to_string(),
                    crop: None,
                    alt_text: None,
                }),
            ],
        ));
        let original = deck.clone();

        let moved = Transform {
            frame: Rect::new(50.0, 60.0, 70.0, 80.0),
            rotation: 12.0,
        };
        let mut bus = CommandBus::default();
        bus.apply(Box::new(MoveShape::new("s1", 0, moved)), &mut deck)
            .expect("move text box");
        bus.apply(Box::new(MoveShape::new("s1", 1, moved)), &mut deck)
            .expect("move geometric");
        bus.apply(Box::new(MoveShape::new("s1", 2, moved)), &mut deck)
            .expect("move image");

        if let Shape::TextBox(tb) = &deck.slides[0].shapes[0] {
            assert_eq!(tb.frame, moved.frame);
        } else {
            panic!("expected text box");
        }
        if let Shape::Geometric(g) = &deck.slides[0].shapes[1] {
            assert_eq!(g.transform, moved);
        } else {
            panic!("expected geometric");
        }
        if let Shape::Image(i) = &deck.slides[0].shapes[2] {
            assert_eq!(i.transform, moved);
        } else {
            panic!("expected image");
        }

        bus.undo(&mut deck);
        bus.undo(&mut deck);
        bus.undo(&mut deck);
        assert_eq!(deck, original);
    }

    #[test]
    fn set_shape_style_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Geometric(GeometricShape {
                id: String::new(),
                transform: Transform::default(),
                geometry: Geometry::Ellipse,
                style: Style::default(),
            })],
        ));
        let original = deck.clone();

        let new_style = Style {
            fill: Some(Fill::Solid(Color::rgb(0, 0, 255))),
            outline: None,
            shadow: None,
        };
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetShapeStyle::new("s1", 0, new_style.clone())),
            &mut deck,
        )
        .expect("apply");
        if let Shape::Geometric(g) = &deck.slides[0].shapes[0] {
            assert_eq!(g.style, new_style);
        } else {
            panic!("expected geometric");
        }

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_shape_style_rejects_non_geometric_shapes() {
        let mut deck = Deck::new();
        deck.media.insert("m".to_string(), sample_media_entry());
        deck.slides.push(slide_with(
            "s1",
            vec![
                Shape::TextBox(TextBox {
                    id: String::new(),
                    frame: Rect::new(0.0, 0.0, 1.0, 1.0),
                    paragraphs: Vec::new(),
                }),
                Shape::Image(ImageShape {
                    id: String::new(),
                    transform: Transform::default(),
                    media_ref: "m".to_string(),
                    crop: None,
                    alt_text: None,
                }),
            ],
        ));

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(SetShapeStyle::new("s1", 0, Style::default())),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(SetShapeStyle::new("s1", 1, Style::default())),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn move_shape_rejects_passthrough() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Passthrough(PassthroughObject {
                id: "p".to_string(),
                label: "p".to_string(),
                source_part: "p".to_string(),
                raw_bytes: Vec::new(),
                frame: None,
            })],
        ));

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(MoveShape::new("s1", 0, Transform::default())),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn command_validation_rejects_bad_indices_and_slides() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(AddShape::new("missing", geo_rectangle())),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(Box::new(DeleteShape::new("s1", 9)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(InsertImage::new(
                    "missing",
                    "k",
                    sample_media_entry(),
                    Transform::default(),
                    None,
                )),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn insert_image_applies_and_undoes_with_fresh_media() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let original = deck.clone();

        let transform = Transform {
            frame: Rect::new(1.0, 2.0, 3.0, 4.0),
            rotation: 5.0,
        };
        let crop = Crop {
            left: 0.0,
            top: 0.1,
            right: 0.2,
            bottom: 0.3,
        };
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(InsertImage::new(
                "s1",
                "img1",
                sample_media_entry(),
                transform,
                Some(crop.clone()),
            )),
            &mut deck,
        )
        .expect("apply");

        assert!(deck.media.contains_key("img1"));
        assert_eq!(deck.slides[0].shapes.len(), 1);
        if let Shape::Image(image) = &deck.slides[0].shapes[0] {
            assert_eq!(image.media_ref, "img1");
            assert_eq!(image.transform, transform);
            assert_eq!(image.crop, Some(crop));
        } else {
            panic!("expected image");
        }

        assert!(bus.undo(&mut deck).is_some());
        assert!(
            !deck.media.contains_key("img1"),
            "fresh media must be removed on undo"
        );
        assert_eq!(deck, original);
    }

    #[test]
    fn insert_image_with_existing_key_keeps_media_on_undo() {
        let mut deck = Deck::new();
        deck.media
            .insert("shared".to_string(), sample_media_entry());
        deck.slides.push(slide_with("s1", Vec::new()));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(InsertImage::new(
                "s1",
                "shared",
                sample_media_entry(),
                Transform::default(),
                None,
            )),
            &mut deck,
        )
        .expect("apply");

        // Key already existed, so the store still has exactly one entry.
        assert_eq!(deck.media.len(), 1);
        assert!(deck.media.contains_key("shared"));
        assert_eq!(deck.slides[0].shapes.len(), 1);

        assert!(bus.undo(&mut deck).is_some());
        assert!(
            deck.media.contains_key("shared"),
            "pre-existing media must survive undo"
        );
        assert_eq!(deck, original);
    }

    #[test]
    fn rich_text_run_serializes_and_deserializes() {
        let run = Run::new("link")
            .strikethrough()
            .superscript()
            .link("mailto:hi@example.com")
            .expect("valid mailto link")
            .code()
            .font("Consolas");
        assert!(run.strikethrough);
        assert_eq!(run.vertical_align, VerticalAlign::Superscript);
        assert!(run.code);
        assert_eq!(run.font_family, Some("Consolas".to_string()));
        assert_eq!(
            run.link.as_ref().map(|l| l.url.as_str()),
            Some("mailto:hi@example.com")
        );

        let json = serde_json::to_string(&run).expect("serialize run");
        let restored: Run = serde_json::from_str(&json).expect("deserialize run");
        assert_eq!(run, restored);
    }

    #[test]
    fn paragraph_style_serializes_and_deserializes() {
        let paragraph = Paragraph {
            runs: vec![Run::new("heading text")],
            list_style: ListStyle::None,
            style: ParagraphStyle {
                heading: Some(HeadingLevel::H2),
                blockquote: true,
                code_block: true,
                code_step_ranges: None,
                indent_level: 2,
            },
        };

        let json = serde_json::to_string(&paragraph).expect("serialize paragraph");
        let restored: Paragraph = serde_json::from_str(&json).expect("deserialize paragraph");
        assert_eq!(paragraph, restored);
    }

    #[test]
    fn old_deck_without_new_text_fields_deserializes() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("legacy").bold().italic()],
                    list_style: ListStyle::Ordered,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });

        let mut value = serde_json::to_value(&deck).expect("serialize to value");
        let object = value.as_object_mut().expect("deck is an object");
        // Strip paragraph.style and the new run fields from the JSON.
        let slides = object.get_mut("slides").unwrap().as_array_mut().unwrap();
        for slide in slides {
            let shapes = slide.get_mut("shapes").unwrap().as_array_mut().unwrap();
            for shape in shapes {
                let tb = shape.get_mut("value").unwrap();
                let paragraphs = tb.get_mut("paragraphs").unwrap().as_array_mut().unwrap();
                for paragraph in paragraphs {
                    paragraph.as_object_mut().unwrap().remove("style");
                    let runs = paragraph.get_mut("runs").unwrap().as_array_mut().unwrap();
                    for run in runs {
                        let run_obj = run.as_object_mut().unwrap();
                        run_obj.remove("strikethrough");
                        run_obj.remove("vertical_align");
                        run_obj.remove("link");
                        run_obj.remove("code");
                        run_obj.remove("font_family");
                    }
                }
            }
        }

        let old_json = serde_json::to_string(&value).expect("reserialize old deck");
        let restored: Deck = serde_json::from_str(&old_json).expect("old deck must load");

        let slide = &restored.slides[0];
        let Shape::TextBox(tb) = &slide.shapes[0] else {
            panic!("expected text box");
        };
        let run = &tb.paragraphs[0].runs[0];
        assert!(run.bold);
        assert!(run.italic);
        assert!(!run.strikethrough);
        assert_eq!(run.vertical_align, VerticalAlign::Baseline);
        assert!(run.link.is_none());
        assert!(!run.code);
        assert!(run.font_family.is_none());
        assert_eq!(tb.paragraphs[0].style, ParagraphStyle::default());
    }

    #[test]
    fn link_allowlist_and_rejections() {
        assert!(Link::new("mailto:hi@example.com").is_ok());
        assert!(Link::new("tel:+123").is_ok());
        assert!(Link::new("#fragment").is_ok());
        assert!(Link::new("relative.html").is_ok());
        assert!(Link::new("./x").is_ok());
        assert!(Link::new("http://example.com").is_ok());
        assert!(Link::new("https://example.com").is_ok());

        assert_eq!(
            Link::new("javascript:alert(1)"),
            Err(LinkError::DisallowedScheme("javascript".to_string()))
        );
        assert_eq!(
            Link::new("JavaScript:alert(1)"),
            Err(LinkError::DisallowedScheme("javascript".to_string()))
        );
        assert_eq!(
            Link::new("vbscript:msgbox(1)"),
            Err(LinkError::DisallowedScheme("vbscript".to_string()))
        );
        assert_eq!(
            Link::new("mocha:test"),
            Err(LinkError::DisallowedScheme("mocha".to_string()))
        );
        assert_eq!(
            Link::new("livescript:test"),
            Err(LinkError::DisallowedScheme("livescript".to_string()))
        );
        assert_eq!(
            Link::new("data:text/html,<script>"),
            Err(LinkError::DisallowedScheme("data".to_string()))
        );
        assert_eq!(
            Link::new("file:///etc/hosts"),
            Err(LinkError::DisallowedScheme("file".to_string()))
        );
        assert_eq!(
            Link::new("unknown-scheme:foo"),
            Err(LinkError::DisallowedScheme("unknown-scheme".to_string()))
        );

        let control_url = "http://example.com\u{0001}x".to_string();
        assert_eq!(Link::new(control_url), Err(LinkError::ControlCharacters));
    }

    #[test]
    fn link_new_unchecked_does_not_validate() {
        let link = Link::new_unchecked("javascript:alert(1)");
        assert_eq!(link.url, "javascript:alert(1)");
    }

    #[test]
    fn set_run_style_applies_and_undoes_only_some_fields() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("seed").bold().strikethrough().superscript()],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(
                SetRunStyle::new("s1", 0, 0, 0)
                    .italic(true)
                    .code(true)
                    .subscript(),
            ),
            &mut deck,
        )
        .expect("apply");

        let Shape::TextBox(tb) = &deck.slides[0].shapes[0] else {
            panic!("expected text box");
        };
        let run = &tb.paragraphs[0].runs[0];
        assert!(run.bold, "bold should remain untouched");
        assert!(run.italic, "italic should be set");
        assert!(!run.underline, "underline should remain default");
        assert!(run.strikethrough, "strikethrough should remain untouched");
        assert_eq!(
            run.vertical_align,
            VerticalAlign::Subscript,
            "subscript should be set"
        );
        assert!(run.code, "code should be set");

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_run_style_merge_preserves_untouched_flags() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![Shape::TextBox(TextBox {
                id: String::new(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("seed").bold()],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetRunStyle::new("s1", 0, 0, 0).italic(true)),
            &mut deck,
        )
        .expect("apply");

        let Shape::TextBox(tb) = &deck.slides[0].shapes[0] else {
            panic!("expected text box");
        };
        let run = &tb.paragraphs[0].runs[0];
        assert!(run.bold, "bold should remain true");
        assert!(run.italic, "italic should be set");
    }

    #[test]
    fn set_run_style_validates_indices_and_shape_kind() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![
                Shape::TextBox(TextBox {
                    id: String::new(),
                    frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                    paragraphs: vec![Paragraph {
                        runs: vec![Run::new("seed")],
                        list_style: ListStyle::None,
                        ..Default::default()
                    }],
                }),
                Shape::Geometric(GeometricShape {
                    id: String::new(),
                    transform: Transform::default(),
                    geometry: Geometry::Rectangle,
                    style: Style::default(),
                }),
            ],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        });

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(SetRunStyle::new("missing", 0, 0, 0).italic(true)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(SetRunStyle::new("s1", 9, 0, 0).italic(true)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(SetRunStyle::new("s1", 0, 9, 0).italic(true)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(SetRunStyle::new("s1", 0, 0, 9).italic(true)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(SetRunStyle::new("s1", 1, 0, 0).italic(true)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
    }

    fn sample_table() -> TableShape {
        TableShape::default_grid(3, 3, Rect::new(0.0, 0.0, 9000.0, 6000.0))
    }

    fn filled_table() -> TableShape {
        let frame = Rect::new(0.0, 0.0, 914_400.0, 685_800.0);
        let cell = |text: &str| TableCell {
            text: text.to_string(),
            fill: Some(Fill::Solid(Color::rgb(200, 200, 200))),
            borders: None,
            align: CellAlign::Left,
        };
        let rows = vec![
            TableRow {
                height: 200_000.0,
                cells: vec![cell("A1"), cell("B1"), cell("C1")],
            },
            TableRow {
                height: 200_000.0,
                cells: vec![cell("A2"), cell("B2"), cell("C2")],
            },
        ];
        TableShape::new(
            Transform {
                frame,
                rotation: 0.0,
            },
            rows,
            vec![300_000.0, 300_000.0, 300_000.0],
        )
        .expect("valid table")
    }

    #[test]
    fn table_new_validates_invariants() {
        let row = |cols: usize| TableRow {
            height: 100.0,
            cells: (0..cols).map(|_| TableCell::default()).collect(),
        };

        let ok = TableShape::new(
            Transform::default(),
            vec![row(3), row(3), row(3)],
            vec![10.0, 20.0, 30.0],
        );
        let table = ok.expect("valid 3x3 table");
        assert_eq!(table.row_count(), 3);
        assert_eq!(table.col_count(), 3);
        assert!(!table.header_row);
        assert_eq!(table.default_borders, TableBorders::default());
        assert!(table.validate());

        assert_eq!(
            TableShape::new(Transform::default(), Vec::new(), Vec::new()),
            Err(TableError::Empty)
        );
        assert_eq!(
            TableShape::new(Transform::default(), vec![row(3), row(2)], vec![1.0, 1.0]),
            Err(TableError::RaggedRows)
        );
        assert_eq!(
            TableShape::new(Transform::default(), vec![row(3)], vec![1.0, 1.0]),
            Err(TableError::ColumnCountMismatch { got: 2, want: 3 })
        );

        let big_rows = (0..51).map(|_| row(51)).collect::<Vec<_>>();
        let big_widths = (0..51).map(|_| 1.0).collect::<Vec<_>>();
        assert_eq!(
            TableShape::new(Transform::default(), big_rows, big_widths),
            Err(TableError::TooLarge { rows: 51, cols: 51 })
        );

        let max_rows = (0..50).map(|_| row(50)).collect::<Vec<_>>();
        let max_widths = (0..50).map(|_| 1.0).collect::<Vec<_>>();
        assert!(TableShape::new(Transform::default(), max_rows, max_widths).is_ok());
    }

    #[test]
    fn table_default_grid_builds_equal_grid() {
        let frame = Rect::new(0.0, 0.0, 9000.0, 6000.0);
        let table = TableShape::default_grid(3, 3, frame);
        assert_eq!(table.row_count(), 3);
        assert_eq!(table.col_count(), 3);
        assert_eq!(table.transform.frame, frame);
        assert_eq!(table.transform.rotation, 0.0);
        assert_eq!(table.column_widths, vec![3000.0, 3000.0, 3000.0]);
        assert!(table.rows.iter().all(|r| (r.height - 2000.0).abs() < 1e-9));
        assert!(table.cell(0, 0).unwrap().text.is_empty());
        assert!(table.validate());
    }

    #[test]
    #[should_panic(expected = "default_grid requires at least one row")]
    fn table_default_grid_panics_on_zero_rows() {
        let _ = TableShape::default_grid(0, 3, Rect::new(0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn table_shape_serializes_and_deserializes() {
        let mut table = filled_table();
        table.header_row = true;
        table.default_borders = TableBorders {
            top: Some(BorderEdge {
                color: Color::black(),
                width_emu: 9525.0,
                dash: DashStyle::Solid,
            }),
            bottom: Some(BorderEdge {
                color: Color::black(),
                width_emu: 9525.0,
                dash: DashStyle::Dash,
            }),
            left: None,
            right: None,
        };
        table.cell_mut(1, 1).unwrap().align = CellAlign::Center;
        table.cell_mut(1, 1).unwrap().borders = Some(TableBorders {
            left: Some(BorderEdge {
                color: Color::rgb(255, 0, 0),
                width_emu: 1000.0,
                dash: DashStyle::Dot,
            }),
            ..Default::default()
        });

        let json = serde_json::to_string(&table).expect("serialize table");
        let restored: TableShape = serde_json::from_str(&json).expect("deserialize table");
        assert_eq!(table, restored);
        assert!(json.contains("\"header_row\":true"));
        assert!(json.contains("\"center\""));
        assert!(json.contains("\"dot\""));
    }

    #[test]
    fn old_deck_without_table_deserializes() {
        let mut deck = Deck::new();
        deck.media.insert("m".to_string(), sample_media_entry());
        deck.slides.push(slide_with(
            "s1",
            vec![
                Shape::TextBox(TextBox {
                    id: String::new(),
                    frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                    paragraphs: Vec::new(),
                }),
                Shape::Image(ImageShape {
                    id: String::new(),
                    transform: Transform::default(),
                    media_ref: "m".to_string(),
                    crop: None,
                    alt_text: None,
                }),
                geo_rectangle(),
                Shape::Passthrough(PassthroughObject {
                    id: "p".to_string(),
                    label: "p".to_string(),
                    source_part: "ppt/slides/slide1.xml".to_string(),
                    raw_bytes: Vec::new(),
                    frame: None,
                }),
            ],
        ));

        let json = serde_json::to_string(&deck).expect("serialize");
        let restored: Deck = serde_json::from_str(&json).expect("old deck must load");
        assert_eq!(deck, restored);
        assert!(!json.contains("\"table\""));
        assert_eq!(restored.slides[0].shapes.len(), 4);
    }

    #[test]
    fn add_table_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(AddTable::new("s1", sample_table())), &mut deck)
            .expect("apply");
        assert_eq!(deck.slides[0].shapes.len(), 1);
        assert!(matches!(deck.slides[0].shapes[0], Shape::Table(_)));

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn add_table_rejects_invalid_table_and_missing_slide() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));

        let bad = TableShape {
            id: String::new(),
            transform: Transform::default(),
            rows: vec![
                TableRow {
                    height: 1.0,
                    cells: vec![TableCell::default()],
                },
                TableRow {
                    height: 1.0,
                    cells: vec![TableCell::default(), TableCell::default()],
                },
            ],
            column_widths: vec![1.0],
            default_borders: TableBorders::default(),
            header_row: false,
        };

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(Box::new(AddTable::new("s1", bad)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(AddTable::new("missing", sample_table())),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
        assert_eq!(deck.slides[0].shapes.len(), 0);
    }

    #[test]
    fn set_cell_text_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(sample_table())]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetCellText::new("s1", 0, 1, 2, "hello")),
            &mut deck,
        )
        .expect("apply");
        let Shape::Table(t) = &deck.slides[0].shapes[0] else {
            panic!("expected table");
        };
        assert_eq!(t.cell(1, 2).unwrap().text, "hello");

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_cell_text_rejects_bad_indices_and_non_table() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Table(sample_table()), geo_rectangle()],
        ));

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(Box::new(SetCellText::new("s1", 0, 9, 0, "x")), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(Box::new(SetCellText::new("s1", 0, 0, 9, "x")), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(Box::new(SetCellText::new("s1", 9, 0, 0, "x")), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(Box::new(SetCellText::new("s1", 1, 0, 0, "x")), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(SetCellText::new("missing", 0, 0, 0, "x")),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn set_cell_style_merge_preserves_untouched_fields() {
        let mut deck = Deck::new();
        let mut table = sample_table();
        table.cell_mut(0, 0).unwrap().fill = Some(Fill::Solid(Color::rgb(255, 0, 0)));
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(table)]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetCellStyle::new("s1", 0, 0, 0).align(CellAlign::Center)),
            &mut deck,
        )
        .expect("apply");

        let Shape::Table(t) = &deck.slides[0].shapes[0] else {
            panic!("expected table");
        };
        let cell = t.cell(0, 0).unwrap();
        assert_eq!(cell.align, CellAlign::Center);
        assert_eq!(cell.fill, Some(Fill::Solid(Color::rgb(255, 0, 0))));

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_cell_style_sets_and_clears_fields() {
        let mut deck = Deck::new();
        let mut table = sample_table();
        table.cell_mut(0, 0).unwrap().fill = Some(Fill::Solid(Color::rgb(255, 0, 0)));
        table.cell_mut(0, 0).unwrap().borders = Some(TableBorders {
            top: Some(BorderEdge {
                color: Color::black(),
                width_emu: 1.0,
                dash: DashStyle::Solid,
            }),
            ..Default::default()
        });
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(table)]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(
                SetCellStyle::new("s1", 0, 0, 0)
                    .fill(None)
                    .borders(Some(TableBorders::default()))
                    .align(CellAlign::Right),
            ),
            &mut deck,
        )
        .expect("apply");

        let Shape::Table(t) = &deck.slides[0].shapes[0] else {
            panic!("expected table");
        };
        let cell = t.cell(0, 0).unwrap();
        assert!(cell.fill.is_none(), "fill should be cleared");
        assert_eq!(cell.borders, Some(TableBorders::default()));
        assert_eq!(cell.align, CellAlign::Right);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn resize_table_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(sample_table())]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(ResizeTable::new(
                "s1",
                0,
                vec![10.0, 20.0, 30.0],
                vec![5.0, 15.0, 25.0],
            )),
            &mut deck,
        )
        .expect("apply");
        let Shape::Table(t) = &deck.slides[0].shapes[0] else {
            panic!("expected table");
        };
        assert_eq!(t.column_widths, vec![10.0, 20.0, 30.0]);
        assert_eq!(t.rows[0].height, 5.0);
        assert_eq!(t.rows[2].height, 25.0);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn resize_table_rejects_mismatched_lengths_and_non_table() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Table(sample_table()), geo_rectangle()],
        ));

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(ResizeTable::new("s1", 0, vec![1.0], vec![1.0, 2.0, 3.0])),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(ResizeTable::new("s1", 0, vec![1.0, 2.0, 3.0], vec![1.0])),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(ResizeTable::new("s1", 1, vec![1.0], Vec::new())),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn insert_row_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(sample_table())]));
        let original = deck.clone();

        let new_row = TableRow {
            height: 999.0,
            cells: vec![
                TableCell {
                    text: "x".to_string(),
                    ..Default::default()
                },
                TableCell {
                    text: "y".to_string(),
                    ..Default::default()
                },
                TableCell {
                    text: "z".to_string(),
                    ..Default::default()
                },
            ],
        };

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(InsertRow::new("s1", 0, 1, new_row.clone())),
            &mut deck,
        )
        .expect("apply");
        let Shape::Table(t) = &deck.slides[0].shapes[0] else {
            panic!("expected table");
        };
        assert_eq!(t.row_count(), 4);
        assert_eq!(t.rows[1].height, 999.0);
        assert_eq!(t.cell(1, 0).unwrap().text, "x");

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn insert_row_rejects_bad_width_index_and_cap() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(sample_table())]));

        let mut bus = CommandBus::default();
        let wrong_cells = TableRow {
            height: 1.0,
            cells: vec![TableCell::default()],
        };
        assert_eq!(
            bus.apply(Box::new(InsertRow::new("s1", 0, 0, wrong_cells)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        let good_row = TableRow {
            height: 1.0,
            cells: vec![TableCell::default(); 3],
        };
        assert_eq!(
            bus.apply(
                Box::new(InsertRow::new("s1", 0, 9, good_row.clone())),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);

        let mut big = sample_table();
        while big.row_count() < MAX_TABLE_ROWS {
            big.rows.push(TableRow {
                height: 1.0,
                cells: vec![TableCell::default(); 3],
            });
        }
        let mut capped_deck = Deck::new();
        capped_deck
            .slides
            .push(slide_with("s1", vec![Shape::Table(big)]));
        let mut bus2 = CommandBus::default();
        assert_eq!(
            bus2.apply(
                Box::new(InsertRow::new("s1", 0, 0, good_row)),
                &mut capped_deck
            ),
            Err(CommandError::InvalidCommand)
        );
    }

    #[test]
    fn delete_row_applies_and_restores_on_undo() {
        let mut deck = Deck::new();
        let mut table = sample_table();
        table.cell_mut(1, 0).unwrap().text = "row1".to_string();
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(table)]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(DeleteRow::new("s1", 0, 1)), &mut deck)
            .expect("apply");
        let Shape::Table(t) = &deck.slides[0].shapes[0] else {
            panic!("expected table");
        };
        assert_eq!(t.row_count(), 2);
        assert_eq!(t.cell(1, 0).unwrap().text, "");

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn delete_row_keeps_at_least_one_row() {
        let mut deck = Deck::new();
        let table = TableShape::default_grid(1, 2, Rect::new(0.0, 0.0, 100.0, 100.0));
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(table)]));

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(Box::new(DeleteRow::new("s1", 0, 0)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
    }

    #[test]
    fn insert_column_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(sample_table())]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(InsertColumn::new("s1", 0, 1, 1234.0)), &mut deck)
            .expect("apply");
        let Shape::Table(t) = &deck.slides[0].shapes[0] else {
            panic!("expected table");
        };
        assert_eq!(t.col_count(), 4);
        assert_eq!(t.column_widths[1], 1234.0);
        assert!(t.cell(0, 1).unwrap().text.is_empty());

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn insert_column_rejects_cap() {
        let mut table = sample_table();
        while table.col_count() < MAX_TABLE_COLS {
            for row in &mut table.rows {
                row.cells.push(TableCell::default());
            }
            table.column_widths.push(1.0);
        }
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(table)]));

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(Box::new(InsertColumn::new("s1", 0, 0, 1.0)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
    }

    #[test]
    fn delete_column_restores_full_column_on_undo() {
        let mut deck = Deck::new();
        let mut table = sample_table();
        for r in 0..3 {
            table.cell_mut(r, 1).unwrap().text = format!("c{r}");
            table.cell_mut(r, 1).unwrap().fill = Some(Fill::Solid(Color::rgb(r as u8, 0, 0)));
        }
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(table)]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(DeleteColumn::new("s1", 0, 1)), &mut deck)
            .expect("apply");
        let Shape::Table(t) = &deck.slides[0].shapes[0] else {
            panic!("expected table");
        };
        assert_eq!(t.col_count(), 2);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn delete_column_keeps_at_least_one_column() {
        let mut deck = Deck::new();
        let table = TableShape::default_grid(2, 1, Rect::new(0.0, 0.0, 100.0, 100.0));
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(table)]));

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(Box::new(DeleteColumn::new("s1", 0, 0)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
    }

    #[test]
    fn insert_column_then_delete_column_round_trips() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(sample_table())]));
        let original = deck.clone();

        let table_cols = |deck: &Deck| match &deck.slides[0].shapes[0] {
            Shape::Table(t) => t.col_count(),
            _ => panic!("expected table"),
        };

        let mut bus = CommandBus::default();
        bus.apply(Box::new(InsertColumn::new("s1", 0, 2, 500.0)), &mut deck)
            .expect("insert column");
        assert_eq!(table_cols(&deck), 4);

        bus.apply(Box::new(DeleteColumn::new("s1", 0, 2)), &mut deck)
            .expect("delete column");
        assert_eq!(table_cols(&deck), 3);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(table_cols(&deck), 4);
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn delete_shape_of_whole_table_restores_on_undo() {
        let mut deck = Deck::new();
        let mut table = filled_table();
        table.header_row = true;
        deck.slides
            .push(slide_with("s1", vec![geo_rectangle(), Shape::Table(table)]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(DeleteShape::new("s1", 1)), &mut deck)
            .expect("apply");
        assert_eq!(deck.slides[0].shapes.len(), 1);
        assert!(matches!(deck.slides[0].shapes[0], Shape::Geometric(_)));

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
        let Shape::Table(t) = &deck.slides[0].shapes[1] else {
            panic!("expected restored table");
        };
        assert_eq!(t.row_count(), 2);
        assert_eq!(t.col_count(), 3);
        assert!(t.header_row);
        assert_eq!(t.cell(0, 0).unwrap().text, "A1");
        assert_eq!(t.cell(1, 2).unwrap().text, "C2");
    }

    #[test]
    fn move_shape_moves_table_transform() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![Shape::Table(sample_table())]));
        let original = deck.clone();

        let moved = Transform {
            frame: Rect::new(50.0, 60.0, 70.0, 80.0),
            rotation: 12.0,
        };
        let mut bus = CommandBus::default();
        bus.apply(Box::new(MoveShape::new("s1", 0, moved)), &mut deck)
            .expect("move table");
        let Shape::Table(t) = &deck.slides[0].shapes[0] else {
            panic!("expected table");
        };
        assert_eq!(t.transform, moved);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    fn sample_category_chart() -> ChartShape {
        ChartShape::new(
            Transform {
                frame: Rect::new(0.0, 0.0, 914_400.0, 685_800.0),
                rotation: 0.0,
            },
            ChartType::Column,
            ChartData::Category {
                categories: vec!["Q1".to_string(), "Q2".to_string(), "Q3".to_string()],
                series: vec![
                    CategorySeries {
                        name: "2023".to_string(),
                        values: vec![10.0, 20.0, 30.0],
                    },
                    CategorySeries {
                        name: "2024".to_string(),
                        values: vec![15.0, 25.0, 35.0],
                    },
                ],
            },
            Some("Revenue".to_string()),
        )
        .expect("valid category chart")
    }

    fn sample_xy_chart() -> ChartShape {
        ChartShape::new(
            Transform::default(),
            ChartType::Scatter,
            ChartData::XY {
                series: vec![XYSeries {
                    name: "Run A".to_string(),
                    points: vec![XYPoint::new(0.0, 1.0), XYPoint::new(1.0, 2.0)],
                }],
            },
            None,
        )
        .expect("valid xy chart")
    }

    #[test]
    fn chart_type_classification() {
        for t in [
            ChartType::Bar,
            ChartType::Column,
            ChartType::Line,
            ChartType::Area,
            ChartType::Pie,
        ] {
            assert!(t.is_category(), "{:?} should be category", t);
            assert!(!t.is_xy(), "{:?} should not be xy", t);
        }
        assert!(ChartType::Scatter.is_xy());
        assert!(!ChartType::Scatter.is_category());
    }

    #[test]
    fn chart_new_validates_invariants() {
        let cat = |vals: Vec<f64>| CategorySeries {
            name: "s".to_string(),
            values: vals,
        };

        let ok = ChartShape::new(
            Transform::default(),
            ChartType::Bar,
            ChartData::Category {
                categories: vec!["a".to_string(), "b".to_string()],
                series: vec![cat(vec![1.0, 2.0]), cat(vec![3.0, 4.0])],
            },
            None,
        );
        let chart = ok.expect("valid category chart");
        assert_eq!(chart.title, None);
        assert!(chart.validate());

        // Empty (no series).
        assert_eq!(
            ChartShape::new(
                Transform::default(),
                ChartType::Column,
                ChartData::Category {
                    categories: vec!["a".to_string()],
                    series: Vec::new(),
                },
                None,
            ),
            Err(ChartError::Empty)
        );

        // Ragged: the first series that does not align with categories is
        // reported (here, index 0 has 1 value but there are 2 categories).
        assert_eq!(
            ChartShape::new(
                Transform::default(),
                ChartType::Line,
                ChartData::Category {
                    categories: vec!["a".to_string(), "b".to_string()],
                    series: vec![cat(vec![1.0]), cat(vec![1.0, 2.0, 3.0])],
                },
                None,
            ),
            Err(ChartError::SeriesCategoryMismatch {
                index: 0,
                got: 1,
                want: 2,
            })
        );

        // Series cap.
        let over_series = (0..MAX_CHART_SERIES + 1)
            .map(|_| cat(vec![1.0]))
            .collect::<Vec<_>>();
        assert_eq!(
            ChartShape::new(
                Transform::default(),
                ChartType::Column,
                ChartData::Category {
                    categories: vec!["a".to_string()],
                    series: over_series,
                },
                None,
            ),
            Err(ChartError::TooManySeries {
                got: MAX_CHART_SERIES + 1,
                max: MAX_CHART_SERIES,
            })
        );

        // Point cap (per series).
        let over_points = cat(vec![1.0; MAX_CHART_POINTS + 1]);
        assert_eq!(
            ChartShape::new(
                Transform::default(),
                ChartType::Column,
                ChartData::Category {
                    categories: vec!["x".to_string(); MAX_CHART_POINTS + 1],
                    series: vec![over_points],
                },
                None,
            ),
            Err(ChartError::TooManyPoints {
                index: 0,
                got: MAX_CHART_POINTS + 1,
                max: MAX_CHART_POINTS,
            })
        );

        // Type/data mismatch: scatter needs XY data.
        assert_eq!(
            ChartShape::new(
                Transform::default(),
                ChartType::Scatter,
                ChartData::Category {
                    categories: vec!["a".to_string()],
                    series: vec![cat(vec![1.0])],
                },
                None,
            ),
            Err(ChartError::DataTypeMismatch)
        );

        // Type/data mismatch: category type needs category data.
        assert_eq!(
            ChartShape::new(
                Transform::default(),
                ChartType::Pie,
                ChartData::XY {
                    series: vec![XYSeries {
                        name: "s".to_string(),
                        points: vec![XYPoint::new(0.0, 0.0)],
                    }],
                },
                None,
            ),
            Err(ChartError::DataTypeMismatch)
        );

        // Max series / points are accepted at the boundary.
        let max_series = (0..MAX_CHART_SERIES)
            .map(|_| cat(vec![1.0]))
            .collect::<Vec<_>>();
        assert!(ChartShape::new(
            Transform::default(),
            ChartType::Column,
            ChartData::Category {
                categories: vec!["a".to_string()],
                series: max_series,
            },
            None,
        )
        .is_ok());

        // XY validation.
        let xy = ChartShape::new(
            Transform::default(),
            ChartType::Scatter,
            ChartData::XY {
                series: vec![XYSeries {
                    name: "s".to_string(),
                    points: vec![XYPoint::new(1.0, 2.0); MAX_CHART_POINTS],
                }],
            },
            None,
        );
        assert!(xy.is_ok());
        assert!(xy.unwrap().validate());
    }

    #[test]
    fn chart_shape_serializes_and_deserializes() {
        let mut chart = sample_category_chart();
        chart.title = Some("Sales by Quarter".to_string());
        let json = serde_json::to_string(&chart).expect("serialize chart");
        let restored: ChartShape = serde_json::from_str(&json).expect("deserialize chart");
        assert_eq!(chart, restored);
        assert!(json.contains("\"kind\":\"category\""));
        assert!(json.contains("\"column\""));

        let xy = sample_xy_chart();
        let xy_json = serde_json::to_string(&xy).expect("serialize xy chart");
        let xy_restored: ChartShape = serde_json::from_str(&xy_json).expect("deserialize xy chart");
        assert_eq!(xy, xy_restored);
        assert!(xy_json.contains("\"kind\":\"xy\""));
        assert!(xy_json.contains("\"scatter\""));
    }

    #[test]
    fn deck_with_chart_round_trips() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Chart(sample_category_chart())],
        ));

        let json = serde_json::to_string(&deck).expect("serialize deck");
        let restored: Deck = serde_json::from_str(&json).expect("deserialize deck");
        assert_eq!(deck, restored);
        assert!(json.contains("\"chart\""));
        assert!(matches!(restored.slides[0].shapes[0], Shape::Chart(_)));
    }

    #[test]
    fn add_chart_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(AddChart::new("s1", sample_category_chart())),
            &mut deck,
        )
        .expect("apply");
        assert_eq!(deck.slides[0].shapes.len(), 1);
        assert!(matches!(deck.slides[0].shapes[0], Shape::Chart(_)));

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn add_chart_rejects_invalid_chart_and_missing_slide() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));

        let bad = ChartShape {
            id: String::new(),
            transform: Transform::default(),
            chart_type: ChartType::Column,
            data: ChartData::Category {
                categories: vec!["a".to_string()],
                series: Vec::new(),
            },
            title: None,
        };

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(Box::new(AddChart::new("s1", bad)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(AddChart::new("missing", sample_category_chart())),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
        assert_eq!(deck.slides[0].shapes.len(), 0);
    }

    #[test]
    fn set_chart_type_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Chart(sample_category_chart())],
        ));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetChartType::new("s1", 0, ChartType::Line)),
            &mut deck,
        )
        .expect("apply");
        let Shape::Chart(c) = &deck.slides[0].shapes[0] else {
            panic!("expected chart");
        };
        assert_eq!(c.chart_type, ChartType::Line);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_chart_type_rejects_incompatible_type_and_non_chart() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Chart(sample_category_chart()), geo_rectangle()],
        ));

        let mut bus = CommandBus::default();
        // Scatter is incompatible with category data.
        assert_eq!(
            bus.apply(
                Box::new(SetChartType::new("s1", 0, ChartType::Scatter)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        // Non-chart shape.
        assert_eq!(
            bus.apply(
                Box::new(SetChartType::new("s1", 1, ChartType::Line)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        // Out of range and missing slide.
        assert_eq!(
            bus.apply(
                Box::new(SetChartType::new("s1", 9, ChartType::Line)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(SetChartType::new("missing", 0, ChartType::Line)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);

        // Switching between two category types is allowed.
        let mut bus2 = CommandBus::default();
        bus2.apply(
            Box::new(SetChartType::new("s1", 0, ChartType::Pie)),
            &mut deck,
        )
        .expect("compatible type change");
        let Shape::Chart(c) = &deck.slides[0].shapes[0] else {
            panic!("expected chart");
        };
        assert_eq!(c.chart_type, ChartType::Pie);
    }

    #[test]
    fn set_chart_data_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Chart(sample_category_chart())],
        ));
        let original = deck.clone();

        let new_data = ChartData::Category {
            categories: vec!["A".to_string(), "B".to_string()],
            series: vec![CategorySeries {
                name: "only".to_string(),
                values: vec![1.0, 2.0],
            }],
        };
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetChartData::new("s1", 0, new_data.clone())),
            &mut deck,
        )
        .expect("apply");
        let Shape::Chart(c) = &deck.slides[0].shapes[0] else {
            panic!("expected chart");
        };
        assert_eq!(c.data, new_data);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_chart_data_rejects_incompatible_data_and_non_chart() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Chart(sample_category_chart()), geo_rectangle()],
        ));

        let xy_data = ChartData::XY {
            series: vec![XYSeries {
                name: "s".to_string(),
                points: vec![XYPoint::new(0.0, 0.0)],
            }],
        };
        let ragged_data = ChartData::Category {
            categories: vec!["a".to_string(), "b".to_string()],
            series: vec![CategorySeries {
                name: "s".to_string(),
                values: vec![1.0],
            }],
        };

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(Box::new(SetChartData::new("s1", 0, xy_data)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(Box::new(SetChartData::new("s1", 0, ragged_data)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(SetChartData::new("s1", 1, sample_xy_chart().data)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn set_chart_title_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Chart(sample_category_chart())],
        ));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetChartTitle::new("s1", 0, Some("New Title".to_string()))),
            &mut deck,
        )
        .expect("apply");
        let Shape::Chart(c) = &deck.slides[0].shapes[0] else {
            panic!("expected chart");
        };
        assert_eq!(c.title.as_deref(), Some("New Title"));

        // Clearing the title (None) is also reversible.
        bus.apply(Box::new(SetChartTitle::new("s1", 0, None)), &mut deck)
            .expect("clear");
        let Shape::Chart(c) = &deck.slides[0].shapes[0] else {
            panic!("expected chart");
        };
        assert_eq!(c.title, None);

        assert!(bus.undo(&mut deck).is_some());
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_chart_title_rejects_non_chart() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Chart(sample_category_chart()), geo_rectangle()],
        ));

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(Box::new(SetChartTitle::new("s1", 1, None)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(Box::new(SetChartTitle::new("missing", 0, None)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(Box::new(SetChartTitle::new("s1", 9, None)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn move_shape_moves_chart_transform() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![Shape::Chart(sample_category_chart())],
        ));
        let original = deck.clone();

        let moved = Transform {
            frame: Rect::new(50.0, 60.0, 70.0, 80.0),
            rotation: 12.0,
        };
        let mut bus = CommandBus::default();
        bus.apply(Box::new(MoveShape::new("s1", 0, moved)), &mut deck)
            .expect("move chart");
        let Shape::Chart(c) = &deck.slides[0].shapes[0] else {
            panic!("expected chart");
        };
        assert_eq!(c.transform, moved);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn delete_chart_restores_on_undo() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![geo_rectangle(), Shape::Chart(sample_xy_chart())],
        ));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(DeleteShape::new("s1", 1)), &mut deck)
            .expect("delete chart");
        assert_eq!(deck.slides[0].shapes.len(), 1);
        assert!(matches!(deck.slides[0].shapes[0], Shape::Geometric(_)));

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
        let Shape::Chart(c) = &deck.slides[0].shapes[1] else {
            panic!("expected restored chart");
        };
        assert_eq!(c.chart_type, ChartType::Scatter);
    }

    #[test]
    fn old_deck_without_animation_deserializes() {
        // A deck serialized before animation/transition existed has neither key.
        // The Option fields (with #[serde(default)]) must deserialize to None.
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let original = deck.clone();

        let json = serde_json::to_string(&deck).expect("serialize deck");
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("parse json");
        let slide = value
            .get_mut("slides")
            .expect("slides")
            .get_mut(0)
            .expect("slide 0")
            .as_object_mut()
            .expect("slide object");
        assert!(slide.remove("animation").is_some());
        assert!(slide.remove("transition").is_some());
        let stripped = serde_json::to_string(&value).expect("reserialize");

        let restored: Deck = serde_json::from_str(&stripped).expect("deserialize old deck");
        assert_eq!(restored.slides[0].animation, None);
        assert_eq!(restored.slides[0].transition, None);
        assert_eq!(restored, original);
        assert_eq!(restored.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn transition_new_clamps_duration() {
        assert_eq!(Transition::new(TransitionKind::Fade, 0).duration_ms, 0);
        assert_eq!(
            Transition::new(TransitionKind::Fade, MAX_TRANSITION_MS).duration_ms,
            MAX_TRANSITION_MS
        );
        assert_eq!(
            Transition::new(TransitionKind::Fade, MAX_TRANSITION_MS + 1).duration_ms,
            MAX_TRANSITION_MS
        );
        assert_eq!(
            Transition::new(TransitionKind::Fade, u32::MAX).duration_ms,
            MAX_TRANSITION_MS
        );
        assert_eq!(
            Transition::new(TransitionKind::Push, 1234).kind,
            TransitionKind::Push
        );
    }

    #[test]
    fn build_step_new_clamps_duration() {
        assert_eq!(BuildStep::new(0, BuildEffect::Fade, 0).duration_ms, 0);
        assert_eq!(
            BuildStep::new(0, BuildEffect::Fade, MAX_BUILD_STEP_MS).duration_ms,
            MAX_BUILD_STEP_MS
        );
        assert_eq!(
            BuildStep::new(0, BuildEffect::Fade, MAX_BUILD_STEP_MS + 1).duration_ms,
            MAX_BUILD_STEP_MS
        );
        assert_eq!(
            BuildStep::new(7, BuildEffect::Appear, u32::MAX).duration_ms,
            MAX_BUILD_STEP_MS
        );
        assert_eq!(BuildStep::new(7, BuildEffect::Appear, 10).shape_index, 7);
    }

    #[test]
    fn set_transition_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        // None -> Some.
        bus.apply(
            Box::new(SetTransition::new(
                "s1",
                Some(Transition::new(TransitionKind::Fade, 400)),
            )),
            &mut deck,
        )
        .expect("set");
        assert_eq!(
            deck.slides[0].transition,
            Some(Transition::new(TransitionKind::Fade, 400))
        );
        // Undo restores None.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);

        // Some -> different Some; undo restores the first Some, then None.
        bus.apply(
            Box::new(SetTransition::new(
                "s1",
                Some(Transition::new(TransitionKind::Push, 600)),
            )),
            &mut deck,
        )
        .expect("set push");
        bus.apply(
            Box::new(SetTransition::new(
                "s1",
                Some(Transition::new(TransitionKind::Wipe, 800)),
            )),
            &mut deck,
        )
        .expect("set wipe");
        assert_eq!(
            deck.slides[0].transition,
            Some(Transition::new(TransitionKind::Wipe, 800))
        );
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(
            deck.slides[0].transition,
            Some(Transition::new(TransitionKind::Push, 600))
        );
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_transition_rejects_missing_slide() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(SetTransition::new(
                    "missing",
                    Some(Transition::new(TransitionKind::Fade, 400))
                )),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
        assert_eq!(deck.slides[0].transition, None);
    }

    #[test]
    fn set_slide_animation_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![geo_rectangle(), geo_rectangle()]));
        let original = deck.clone();

        let animation = Animation::new(vec![
            BuildStep::new(0, BuildEffect::Fade, 200),
            BuildStep::new(1, BuildEffect::Appear, 100),
        ]);
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetSlideAnimation::new("s1", Some(animation.clone()))),
            &mut deck,
        )
        .expect("set");
        assert_eq!(deck.slides[0].animation, Some(animation));

        // Clearing is also reversible.
        bus.apply(Box::new(SetSlideAnimation::new("s1", None)), &mut deck)
            .expect("clear");
        assert_eq!(deck.slides[0].animation, None);

        assert!(bus.undo(&mut deck).is_some());
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_slide_animation_rejects_bad_shape_index_and_missing_slide() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()])); // one shape, index 0
        let mut bus = CommandBus::default();
        // shape_index 1 is out of range.
        let bad = Animation::new(vec![BuildStep::new(1, BuildEffect::Fade, 100)]);
        assert_eq!(
            bus.apply(Box::new(SetSlideAnimation::new("s1", Some(bad))), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        // Missing slide.
        assert_eq!(
            bus.apply(Box::new(SetSlideAnimation::new("missing", None)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        // A valid animation is accepted; one bad step rejects the whole sequence.
        let mixed = Animation::new(vec![
            BuildStep::new(0, BuildEffect::Fade, 100),
            BuildStep::new(9, BuildEffect::Appear, 100),
        ]);
        assert_eq!(
            bus.apply(
                Box::new(SetSlideAnimation::new("s1", Some(mixed))),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
        assert_eq!(deck.slides[0].animation, None);
    }

    #[test]
    fn add_build_step_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![geo_rectangle(), geo_rectangle()]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(0, BuildEffect::Fade, 200),
            )),
            &mut deck,
        )
        .expect("add");
        assert_eq!(
            deck.slides[0]
                .animation
                .as_ref()
                .expect("animation")
                .steps
                .len(),
            1
        );
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(1, BuildEffect::Appear, 100),
            )),
            &mut deck,
        )
        .expect("add 2");
        let steps = &deck.slides[0].animation.as_ref().expect("animation").steps;
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].shape_index, 0);
        assert_eq!(steps[1].shape_index, 1);

        // Undoing both restores None (no leftover empty Some).
        assert!(bus.undo(&mut deck).is_some());
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slides[0].animation, None);
        assert_eq!(deck, original);
    }

    #[test]
    fn add_build_step_rejects_bad_shape_index_and_missing_slide() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(AddBuildStep::new(
                    "s1",
                    BuildStep::new(5, BuildEffect::Appear, 100)
                )),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(AddBuildStep::new(
                    "missing",
                    BuildStep::new(0, BuildEffect::Appear, 100)
                )),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
        assert_eq!(deck.slides[0].animation, None);
    }

    #[test]
    fn remove_build_step_at_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![geo_rectangle(), geo_rectangle()]));
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(0, BuildEffect::Fade, 200),
            )),
            &mut deck,
        )
        .expect("seed 0");
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(1, BuildEffect::Appear, 100),
            )),
            &mut deck,
        )
        .expect("seed 1");
        let original = deck.clone();

        // Remove the first step; the remaining one is shape_index 1.
        bus.apply(Box::new(RemoveBuildStepAt::new("s1", 0)), &mut deck)
            .expect("remove");
        let steps = &deck.slides[0].animation.as_ref().expect("animation").steps;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].shape_index, 1);

        // Undo restores the removed step at position 0.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn remove_build_step_at_clears_animation_when_empty() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(0, BuildEffect::Fade, 200),
            )),
            &mut deck,
        )
        .expect("seed");
        assert!(deck.slides[0].animation.is_some());

        // Removing the only step resets animation to None.
        bus.apply(Box::new(RemoveBuildStepAt::new("s1", 0)), &mut deck)
            .expect("remove");
        assert_eq!(deck.slides[0].animation, None);
        // Undo restores it.
        assert!(bus.undo(&mut deck).is_some());
        assert!(deck.slides[0].animation.is_some());
    }

    #[test]
    fn insert_build_step_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides
            .push(slide_with("s1", vec![geo_rectangle(), geo_rectangle()]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        // Inserting creates the animation if absent.
        bus.apply(
            Box::new(InsertBuildStepAt::new(
                "s1",
                0,
                BuildStep::new(1, BuildEffect::Fade, 200),
            )),
            &mut deck,
        )
        .expect("insert");
        let steps = &deck.slides[0].animation.as_ref().expect("animation").steps;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].shape_index, 1);

        // Undo removes the (only) step and clears animation back to None.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn move_build_step_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with(
            "s1",
            vec![geo_rectangle(), geo_rectangle(), geo_rectangle()],
        ));
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(0, BuildEffect::Fade, 100),
            )),
            &mut deck,
        )
        .expect("seed 0");
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(1, BuildEffect::Appear, 100),
            )),
            &mut deck,
        )
        .expect("seed 1");
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(2, BuildEffect::Disappear, 100),
            )),
            &mut deck,
        )
        .expect("seed 2");
        let original = deck.clone();

        // Move the first step (shape_index 0) to the end.
        bus.apply(Box::new(MoveBuildStep::new("s1", 0, 2)), &mut deck)
            .expect("move");
        let steps = &deck.slides[0].animation.as_ref().expect("animation").steps;
        assert_eq!(
            steps.iter().map(|s| s.shape_index).collect::<Vec<_>>(),
            vec![1, 2, 0]
        );

        // Undo restores the original order.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn remove_and_move_build_step_reject_bad_index_and_missing_slide() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(0, BuildEffect::Fade, 100),
            )),
            &mut deck,
        )
        .expect("seed 0");
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(0, BuildEffect::Appear, 100),
            )),
            &mut deck,
        )
        .expect("seed 1");
        // Two steps now.

        let mut rejections = CommandBus::default();
        // Remove out of range.
        assert_eq!(
            rejections.apply(Box::new(RemoveBuildStepAt::new("s1", 9)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        // Remove from a missing slide.
        assert_eq!(
            rejections.apply(Box::new(RemoveBuildStepAt::new("missing", 0)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        // Move out of range (both ends).
        assert_eq!(
            rejections.apply(Box::new(MoveBuildStep::new("s1", 0, 9)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            rejections.apply(Box::new(MoveBuildStep::new("s1", 9, 0)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        // Move on a missing slide.
        assert_eq!(
            rejections.apply(Box::new(MoveBuildStep::new("missing", 0, 1)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        // A slide with no animation rejects remove/move.
        deck.slides.push(slide_with("s2", vec![geo_rectangle()]));
        assert_eq!(
            rejections.apply(Box::new(RemoveBuildStepAt::new("s2", 0)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            rejections.apply(Box::new(MoveBuildStep::new("s2", 0, 0)), &mut deck),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(rejections.undo_len(), 0);
        // Seed data untouched.
        assert_eq!(
            deck.slides[0]
                .animation
                .as_ref()
                .expect("animation")
                .steps
                .len(),
            2
        );
    }

    #[test]
    fn animation_and_transition_serialize_and_deserialize() {
        let transition = Transition::new(TransitionKind::Fade, 750);
        let tj = serde_json::to_string(&transition).expect("serialize transition");
        let tr: Transition = serde_json::from_str(&tj).expect("deserialize transition");
        assert_eq!(transition, tr);
        assert!(tj.contains("\"kind\":\"fade\""));
        assert!(tj.contains("\"duration_ms\":750"));

        let animation = Animation::new(vec![
            BuildStep::new(2, BuildEffect::SlideInLeft, 300),
            BuildStep::new(0, BuildEffect::Disappear, 150),
        ]);
        let aj = serde_json::to_string(&animation).expect("serialize animation");
        let ar: Animation = serde_json::from_str(&aj).expect("deserialize animation");
        assert_eq!(animation, ar);
        assert!(aj.contains("\"slide_in_left\""));
        assert!(aj.contains("\"disappear\""));
        assert!(aj.contains("\"shape_index\""));
        assert!(aj.contains("\"effect\""));
        assert!(aj.contains("\"duration_ms\""));

        // Every enum variant round-trips through its snake_case tag.
        for effect in [
            BuildEffect::Fade,
            BuildEffect::SlideInLeft,
            BuildEffect::SlideInRight,
            BuildEffect::SlideInTop,
            BuildEffect::SlideInBottom,
            BuildEffect::Appear,
            BuildEffect::Disappear,
        ] {
            let j = serde_json::to_string(&effect).expect("serialize effect");
            let r: BuildEffect = serde_json::from_str(&j).expect("deserialize effect");
            assert_eq!(effect, r);
        }
        for kind in [
            TransitionKind::None,
            TransitionKind::Fade,
            TransitionKind::Slide,
            TransitionKind::Push,
            TransitionKind::Wipe,
        ] {
            let j = serde_json::to_string(&kind).expect("serialize kind");
            let r: TransitionKind = serde_json::from_str(&j).expect("deserialize kind");
            assert_eq!(kind, r);
        }
        // The TransitionKind::Slide tag is "slide"; BuildEffect's is "slide_in_left".
        assert_eq!(
            serde_json::to_string(&TransitionKind::Slide).expect("serialize"),
            "\"slide\""
        );
        assert_eq!(
            serde_json::to_string(&BuildEffect::SlideInLeft).expect("serialize"),
            "\"slide_in_left\""
        );
    }

    #[test]
    fn deck_with_animation_and_transition_round_trips() {
        let mut deck = Deck::new();
        let mut slide = slide_with("s1", vec![geo_rectangle(), geo_rectangle()]);
        slide.animation = Some(Animation::new(vec![
            BuildStep::new(0, BuildEffect::Fade, 250),
            BuildStep::new(1, BuildEffect::Appear, 120),
        ]));
        slide.transition = Some(Transition::new(TransitionKind::Push, 900));
        deck.slides.push(slide);

        let json = serde_json::to_string(&deck).expect("serialize deck");
        let restored: Deck = serde_json::from_str(&json).expect("deserialize deck");
        assert_eq!(deck, restored);
        assert_eq!(restored.schema_version, SCHEMA_VERSION);
        assert_eq!(
            restored.slides[0].transition,
            Some(Transition::new(TransitionKind::Push, 900))
        );
        assert_eq!(
            restored.slides[0]
                .animation
                .as_ref()
                .expect("animation")
                .steps
                .len(),
            2
        );
        assert!(json.contains("\"animation\""));
        assert!(json.contains("\"transition\""));
        assert!(json.contains("\"push\""));
        assert!(json.contains("\"fade\""));
    }

    #[test]
    fn duplicate_shape_indices_are_allowed() {
        // The same shape can have multiple steps (e.g. appear then disappear).
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(0, BuildEffect::Appear, 100),
            )),
            &mut deck,
        )
        .expect("appear");
        bus.apply(
            Box::new(AddBuildStep::new(
                "s1",
                BuildStep::new(0, BuildEffect::Disappear, 100),
            )),
            &mut deck,
        )
        .expect("disappear");
        let steps = &deck.slides[0].animation.as_ref().expect("animation").steps;
        assert_eq!(steps.len(), 2);
        assert!(steps.iter().all(|s| s.shape_index == 0));
    }

    #[test]
    fn old_deck_without_wave7_fields_deserializes() {
        // A deck serialized by an older library (before Wave 7) carries none
        // of the new fields. Every new field is additive (#[serde(default)])
        // and must round-trip into its default value.
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        deck.slides.push(slide_with("s2", vec![geo_rectangle()]));

        // Start from the current JSON, then strip every Wave 7 field to mimic
        // a snapshot produced before they existed.
        let mut value = serde_json::to_value(&deck).expect("serialize to value");
        let object = value.as_object_mut().expect("deck is an object");
        object.remove("slide_size");
        object.remove("sections");
        object
            .get_mut("theme")
            .expect("theme")
            .as_object_mut()
            .expect("theme object")
            .remove("high_contrast");
        for slide in object
            .get_mut("slides")
            .expect("slides")
            .as_array_mut()
            .expect("slides array")
        {
            slide
                .as_object_mut()
                .expect("slide object")
                .remove("rich_notes");
        }

        let old_json = serde_json::to_string(&value).expect("reserialize without wave7 fields");
        let restored: Deck = serde_json::from_str(&old_json).expect("old deck must load");

        assert_eq!(restored.slide_size, None);
        assert!(restored.sections.is_empty());
        assert!(!restored.theme.high_contrast);
        assert!(restored.slides.iter().all(|s| s.rich_notes.is_none()));
        assert_eq!(restored.schema_version, SCHEMA_VERSION);
        // The defaults match a freshly-built deck's defaults.
        assert_eq!(restored.slide_size, deck.slide_size);
        assert_eq!(restored.sections, deck.sections);
        assert_eq!(restored.theme.high_contrast, deck.theme.high_contrast);
    }

    #[test]
    fn presenter_settings_default_is_all_off() {
        let settings = PresenterSettings::default();
        assert!(!settings.laser_pointer);
        assert!(!settings.highlighter);
        assert_eq!(settings.laser_color, "#ff0000");
        assert_eq!(settings.highlighter_color, "#ffff00");
    }

    #[test]
    fn set_presenter_settings_applies_and_undoes() {
        let mut deck = Deck::new();
        let original = deck.clone();
        let settings = PresenterSettings {
            laser_pointer: true,
            laser_color: "#00ff00".to_string(),
            highlighter: true,
            highlighter_color: "#0000ff".to_string(),
            projector_filters: ProjectorFilters::default(),
        };
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetPresenterSettings::new(settings.clone())),
            &mut deck,
        )
        .expect("apply");
        assert_eq!(deck.presenter_settings, settings);
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn old_deck_without_presenter_settings_deserializes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let mut value = serde_json::to_value(&deck).expect("serialize");
        value
            .as_object_mut()
            .expect("deck object")
            .remove("presenter_settings");
        let old_json = serde_json::to_string(&value).expect("reserialize");
        let restored: Deck = serde_json::from_str(&old_json).expect("old deck must load");
        assert_eq!(restored.presenter_settings, PresenterSettings::default());
    }

    #[test]
    fn slide_size_constructors() {
        assert_eq!(
            SlideSize::widescreen_16_9(),
            SlideSize {
                width_emu: 12_192_000.0,
                height_emu: 6_858_000.0,
            }
        );
        assert_eq!(
            SlideSize::standard_4_3(),
            SlideSize {
                width_emu: 9_144_000.0,
                height_emu: 6_858_000.0,
            }
        );
        assert_eq!(
            SlideSize::widescreen_16_10(),
            SlideSize {
                width_emu: 12_149_333.0,
                height_emu: 7_593_333.0,
            }
        );
    }

    #[test]
    fn set_slide_size_applies_and_undoes() {
        let mut deck = Deck::new();
        let original = deck.clone();

        let mut bus = CommandBus::default();
        // None -> Some; undo restores None.
        bus.apply(
            Box::new(SetSlideSize::new(Some(SlideSize::widescreen_16_9()))),
            &mut deck,
        )
        .expect("set 16:9");
        assert_eq!(deck.slide_size, Some(SlideSize::widescreen_16_9()));
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slide_size, None);
        assert_eq!(deck, original);

        // Some -> different Some; undo restores the first Some, then None.
        bus.apply(
            Box::new(SetSlideSize::new(Some(SlideSize::standard_4_3()))),
            &mut deck,
        )
        .expect("set 4:3");
        bus.apply(
            Box::new(SetSlideSize::new(Some(SlideSize::widescreen_16_10()))),
            &mut deck,
        )
        .expect("set 16:10");
        assert_eq!(deck.slide_size, Some(SlideSize::widescreen_16_10()));
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slide_size, Some(SlideSize::standard_4_3()));
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slide_size, None);
        assert_eq!(deck, original);

        // Clearing (Some -> None) is reversible too.
        bus.apply(
            Box::new(SetSlideSize::new(Some(SlideSize::widescreen_16_9()))),
            &mut deck,
        )
        .expect("set before clear");
        bus.apply(Box::new(SetSlideSize::new(None)), &mut deck)
            .expect("clear");
        assert_eq!(deck.slide_size, None);
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slide_size, Some(SlideSize::widescreen_16_9()));
    }

    #[test]
    fn set_sections_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        deck.slides.push(slide_with("s2", vec![geo_rectangle()]));
        let original = deck.clone();

        let intro = vec![SlideSection {
            name: "Intro".to_string(),
            start_slide_id: "s1".to_string(),
        }];
        let demo = vec![
            SlideSection {
                name: "Intro".to_string(),
                start_slide_id: "s1".to_string(),
            },
            SlideSection {
                name: "Demo".to_string(),
                start_slide_id: "s2".to_string(),
            },
        ];

        let mut bus = CommandBus::default();
        bus.apply(Box::new(SetSections::new(intro.clone())), &mut deck)
            .expect("set intro");
        assert_eq!(deck.sections, intro);

        bus.apply(Box::new(SetSections::new(demo.clone())), &mut deck)
            .expect("set demo");
        assert_eq!(deck.sections, demo);

        // Undo restores the prior list, then the empty default.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.sections, intro);
        assert!(bus.undo(&mut deck).is_some());
        assert!(deck.sections.is_empty());
        assert_eq!(deck, original);

        // Clearing to empty is reversible.
        bus.apply(Box::new(SetSections::new(demo.clone())), &mut deck)
            .expect("set before clear");
        bus.apply(Box::new(SetSections::new(Vec::new())), &mut deck)
            .expect("clear");
        assert!(deck.sections.is_empty());
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.sections, demo);
    }

    #[test]
    fn set_rich_notes_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let original = deck.clone();

        let first = vec![Paragraph {
            runs: vec![Run::new("note one").bold()],
            list_style: ListStyle::None,
            ..Default::default()
        }];
        let second = vec![
            Paragraph {
                runs: vec![Run::new("note two")],
                list_style: ListStyle::Ordered,
                ..Default::default()
            },
            Paragraph {
                runs: vec![Run::new("more")],
                list_style: ListStyle::Unordered,
                ..Default::default()
            },
        ];

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetRichNotes::new("s1", Some(first.clone()))),
            &mut deck,
        )
        .expect("set first");
        assert_eq!(deck.slides[0].rich_notes.as_ref(), Some(&first));

        bus.apply(
            Box::new(SetRichNotes::new("s1", Some(second.clone()))),
            &mut deck,
        )
        .expect("set second");
        assert_eq!(deck.slides[0].rich_notes.as_ref(), Some(&second));

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slides[0].rich_notes.as_ref(), Some(&first));
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slides[0].rich_notes, None);
        assert_eq!(deck, original);

        // Clearing (Some -> None) is reversible.
        bus.apply(
            Box::new(SetRichNotes::new("s1", Some(first.clone()))),
            &mut deck,
        )
        .expect("set before clear");
        bus.apply(Box::new(SetRichNotes::new("s1", None)), &mut deck)
            .expect("clear");
        assert_eq!(deck.slides[0].rich_notes, None);
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slides[0].rich_notes.as_ref(), Some(&first));
    }

    #[test]
    fn set_rich_notes_rejects_missing_slide() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));

        let notes = Some(vec![Paragraph {
            runs: vec![Run::new("x")],
            list_style: ListStyle::None,
            ..Default::default()
        }]);

        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(SetRichNotes::new("missing", notes.clone())),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
        assert_eq!(deck.slides[0].rich_notes, None);
    }

    #[test]
    fn set_high_contrast_applies_and_undoes() {
        let mut deck = Deck::new();
        let original = deck.clone();

        let mut bus = CommandBus::default();
        // false -> true; undo restores false.
        bus.apply(Box::new(SetHighContrast::new(true)), &mut deck)
            .expect("enable");
        assert!(deck.theme.high_contrast);
        assert!(bus.undo(&mut deck).is_some());
        assert!(!deck.theme.high_contrast);
        assert_eq!(deck, original);

        // true -> false -> true; undo restores each prior state.
        bus.apply(Box::new(SetHighContrast::new(true)), &mut deck)
            .expect("enable 2");
        bus.apply(Box::new(SetHighContrast::new(false)), &mut deck)
            .expect("disable");
        assert!(!deck.theme.high_contrast);
        assert!(bus.undo(&mut deck).is_some());
        assert!(deck.theme.high_contrast);
        assert!(bus.undo(&mut deck).is_some());
        assert!(!deck.theme.high_contrast);
        assert_eq!(deck, original);
    }

    #[test]
    fn deck_with_all_wave7_fields_round_trips() {
        let mut deck = Deck::new();
        deck.slide_size = Some(SlideSize::widescreen_16_9());
        deck.sections = vec![
            SlideSection {
                name: "Intro".to_string(),
                start_slide_id: "s1".to_string(),
            },
            SlideSection {
                name: "Details".to_string(),
                start_slide_id: "s2".to_string(),
            },
        ];
        deck.theme.high_contrast = true;
        let mut slide = slide_with("s1", vec![geo_rectangle()]);
        slide.rich_notes = Some(vec![Paragraph {
            runs: vec![Run::new("rich note").italic()],
            list_style: ListStyle::None,
            ..Default::default()
        }]);
        deck.slides.push(slide);
        deck.slides.push(slide_with("s2", vec![geo_rectangle()]));

        let json = serde_json::to_string(&deck).expect("serialize deck");
        let restored: Deck = serde_json::from_str(&json).expect("deserialize deck");
        assert_eq!(deck, restored);
        assert_eq!(restored.slide_size, Some(SlideSize::widescreen_16_9()));
        assert_eq!(restored.sections.len(), 2);
        assert!(restored.theme.high_contrast);
        assert_eq!(
            restored.slides[0].rich_notes.as_ref().map(Vec::len),
            Some(1)
        );
        assert_eq!(restored.schema_version, SCHEMA_VERSION);
        assert!(json.contains("\"slide_size\""));
        assert!(json.contains("\"sections\""));
        assert!(json.contains("\"high_contrast\":true"));
        assert!(json.contains("\"rich_notes\""));
    }

    #[test]
    fn slide_size_and_sections_and_rich_notes_serialize_and_deserialize() {
        let size = SlideSize::widescreen_16_10();
        let sj = serde_json::to_string(&size).expect("serialize slide size");
        let sr: SlideSize = serde_json::from_str(&sj).expect("deserialize slide size");
        assert_eq!(size, sr);
        assert!(sj.contains("\"width_emu\""));
        assert!(sj.contains("\"height_emu\""));

        let sections = vec![
            SlideSection {
                name: "A".to_string(),
                start_slide_id: "s1".to_string(),
            },
            SlideSection {
                name: "B".to_string(),
                start_slide_id: "s4".to_string(),
            },
        ];
        let secj = serde_json::to_string(&sections).expect("serialize sections");
        let secr: Vec<SlideSection> = serde_json::from_str(&secj).expect("deserialize sections");
        assert_eq!(sections, secr);
        assert!(secj.contains("\"name\""));
        assert!(secj.contains("\"start_slide_id\""));

        let rich = vec![Paragraph {
            runs: vec![Run::new("speaker").bold()],
            list_style: ListStyle::Unordered,
            ..Default::default()
        }];
        let rj = serde_json::to_string(&rich).expect("serialize rich notes");
        let rr: Vec<Paragraph> = serde_json::from_str(&rj).expect("deserialize rich notes");
        assert_eq!(rich, rr);

        // The new commands round-trip too.
        let cmd_size = SetSlideSize::new(Some(SlideSize::standard_4_3()));
        let csj = serde_json::to_string(&cmd_size).expect("serialize set-slide-size");
        let csr: SetSlideSize = serde_json::from_str(&csj).expect("deserialize set-slide-size");
        assert_eq!(cmd_size, csr);

        let cmd_sec = SetSections::new(sections);
        let csecj = serde_json::to_string(&cmd_sec).expect("serialize set-sections");
        let csecr: SetSections = serde_json::from_str(&csecj).expect("deserialize set-sections");
        assert_eq!(cmd_sec, csecr);

        let cmd_hc = SetHighContrast::new(true);
        let chj = serde_json::to_string(&cmd_hc).expect("serialize set-high-contrast");
        let chr: SetHighContrast =
            serde_json::from_str(&chj).expect("deserialize set-high-contrast");
        assert_eq!(cmd_hc, chr);
    }

    #[test]
    fn old_deck_without_template_fields_deserializes() {
        // A deck serialized before the Wave 9 template fields existed (no
        // `template`, `layouts`, `master`, or `layout_ref`) must round-trip
        // unchanged, with the new fields defaulting in.
        let old_json = r#"{
            "schema_version": 1,
            "id": "old-deck",
            "theme": {
                "background": {"r": 255, "g": 255, "b": 255, "a": 255},
                "heading_font": "Calibri",
                "body_font": "Calibri",
                "accent_color": {"r": 0, "g": 112, "b": 192, "a": 255}
            },
            "slides": [{"id": "s1", "notes": "", "shapes": []}]
        }"#;
        let deck: Deck = serde_json::from_str(old_json).expect("old deck deserializes");
        assert_eq!(deck.template, None);
        assert!(deck.layouts.is_empty());
        assert_eq!(deck.master, Master::default());
        assert_eq!(deck.slides[0].layout_ref, None);
        assert_eq!(deck.schema_version, SCHEMA_VERSION);

        let round_json = serde_json::to_string(&deck).expect("serialize deck");
        let restored: Deck = serde_json::from_str(&round_json).expect("deserialize deck");
        assert_eq!(deck, restored);
    }

    #[test]
    fn template_registry_has_six_templates() {
        let names = TemplateRegistry::names();
        assert_eq!(
            names,
            vec![
                "default",
                "educator",
                "pitch",
                "conference_talk",
                "community_update",
                "photo_essay",
            ]
        );
        for name in &names {
            assert!(
                TemplateRegistry::get(name).is_some(),
                "{name} should resolve"
            );
        }
        assert!(TemplateRegistry::get("does-not-exist").is_none());

        let display_names: Vec<&str> = names
            .iter()
            .filter_map(|name| TemplateRegistry::get(name).map(|d| d.display_name))
            .collect();
        assert_eq!(display_names.len(), 6);
        let unique: std::collections::HashSet<&str> = display_names.iter().copied().collect();
        assert_eq!(unique.len(), 6, "display names should be distinct");
    }

    #[test]
    fn template_registry_get_returns_distinct_themes() {
        let default = TemplateRegistry::get("default").expect("default template");
        assert_eq!(default.theme, Theme::default());

        // Pitch is dark.
        let pitch = TemplateRegistry::get("pitch").expect("pitch template");
        assert_eq!(pitch.theme.background, Color::rgb(26, 26, 46));
        assert_ne!(pitch.theme.accent_color, default.theme.accent_color);
        assert_ne!(pitch.theme.heading_font, default.theme.heading_font);

        // Educator is warm: cream background, orange accent.
        let educator = TemplateRegistry::get("educator").expect("educator template");
        assert!(educator.theme.background.r > 240);
        assert!(educator.theme.background.g > 230);
        assert_eq!(educator.theme.accent_color, Color::rgb(232, 122, 48));

        // Photo Essay is black.
        let photo = TemplateRegistry::get("photo_essay").expect("photo essay template");
        assert_eq!(photo.theme.background, Color::rgb(0, 0, 0));

        // All six themes are distinct.
        let mut themes: Vec<Theme> = TemplateRegistry::names()
            .into_iter()
            .map(|name| TemplateRegistry::get(name).unwrap().theme)
            .collect();
        let total = themes.len();
        themes.dedup();
        assert_eq!(
            themes.len(),
            total,
            "all six template themes should be distinct"
        );
    }

    #[test]
    fn set_template_applies_and_undoes() {
        let mut deck = Deck::new();
        let original = deck.clone();
        assert_ne!(deck.theme.background, Color::rgb(26, 26, 46));

        let mut bus = CommandBus::default();
        let cmd = Box::new(SetTemplate::new("pitch"));
        assert!(cmd.validate(&deck));
        bus.apply(cmd, &mut deck).expect("apply should succeed");

        let pitch = TemplateRegistry::get("pitch").unwrap();
        assert_eq!(deck.theme, pitch.theme, "deck theme should be pitch");
        assert_eq!(deck.master, pitch.master);
        assert_eq!(deck.layouts.len(), pitch.layouts.len());
        assert_eq!(deck.template.as_deref(), Some("pitch"));

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(
            deck, original,
            "undo should restore the original deck exactly"
        );
    }

    #[test]
    fn set_template_rejects_unknown_name() {
        let mut deck = Deck::new();
        let mut bus = CommandBus::default();
        let cmd = Box::new(SetTemplate::new("nope"));
        assert!(!cmd.validate(&deck));
        assert_eq!(bus.apply(cmd, &mut deck), Err(CommandError::InvalidCommand));
        assert!(deck.template.is_none());
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn set_slide_layout_applies_and_undoes() {
        let mut deck = Deck::new();
        // Use the default template's layouts so a layout name validates.
        let def = TemplateRegistry::get("default").unwrap();
        deck.layouts = def.layouts.clone();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        assert!(deck.slides[0].layout_ref.is_none());

        let mut bus = CommandBus::default();
        let cmd = Box::new(SetSlideLayout::new(
            "s1",
            Some("Title and Content".to_string()),
        ));
        bus.apply(cmd, &mut deck).expect("apply should succeed");
        assert_eq!(
            deck.slides[0].layout_ref.as_deref(),
            Some("Title and Content")
        );

        assert!(bus.undo(&mut deck).is_some());
        assert!(deck.slides[0].layout_ref.is_none());

        // Clearing via None also round-trips.
        deck.slides[0].layout_ref = Some("Blank".to_string());
        let cmd = Box::new(SetSlideLayout::new("s1", None));
        bus.apply(cmd, &mut deck)
            .expect("apply clear should succeed");
        assert!(deck.slides[0].layout_ref.is_none());
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slides[0].layout_ref.as_deref(), Some("Blank"));
    }

    #[test]
    fn set_slide_layout_rejects_missing_slide_and_unknown_layout() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));

        let mut bus = CommandBus::default();

        // Missing slide.
        let missing = Box::new(SetSlideLayout::new("ghost", Some("Blank".to_string())));
        assert!(!missing.validate(&deck));
        assert_eq!(
            bus.apply(missing, &mut deck),
            Err(CommandError::InvalidCommand)
        );

        // Unknown layout name (deck has no layouts).
        let unknown = Box::new(SetSlideLayout::new("s1", Some("Nope".to_string())));
        assert!(!unknown.validate(&deck));
        assert_eq!(
            bus.apply(unknown, &mut deck),
            Err(CommandError::InvalidCommand)
        );

        assert!(deck.slides[0].layout_ref.is_none());
        assert_eq!(bus.undo_len(), 0);
    }

    #[test]
    fn deck_with_template_and_layouts_round_trips() {
        let mut deck = Deck::new();
        let pitch = TemplateRegistry::get("pitch").unwrap();
        deck.theme = pitch.theme.clone();
        deck.master = pitch.master.clone();
        deck.layouts = pitch.layouts.clone();
        deck.template = Some("pitch".to_string());
        let mut slide = slide_with("s1", vec![geo_rectangle()]);
        slide.layout_ref = Some("Cover".to_string());
        deck.slides.push(slide);

        let json = serde_json::to_string(&deck).expect("serialize deck");
        let restored: Deck = serde_json::from_str(&json).expect("deserialize deck");
        assert_eq!(deck, restored);
        assert_eq!(restored.template.as_deref(), Some("pitch"));
        assert_eq!(restored.layouts.len(), pitch.layouts.len());
        assert!(!restored.master.background_shapes.is_empty());
        assert_eq!(restored.slides[0].layout_ref.as_deref(), Some("Cover"));
        assert!(json.contains("\"template\""));
        assert!(json.contains("\"layouts\""));
        assert!(json.contains("\"layout_ref\""));
    }

    #[test]
    fn master_and_layout_serialize_and_deserialize() {
        let master = Master {
            background_shapes: vec![BackgroundShape {
                geometry: Geometry::Rectangle,
                style: Style {
                    fill: Some(Fill::Solid(Color::rgb(26, 26, 46))),
                    outline: None,
                    shadow: None,
                },
                transform: Transform {
                    frame: Rect::new(0.0, 0.0, 12_192_000.0, 6_858_000.0),
                    rotation: 0.0,
                },
            }],
            placeholders: vec![
                PlaceholderDef {
                    name: "title".to_string(),
                    frame: Rect::new(457_200.0, 457_200.0, 11_277_600.0, 1_143_000.0),
                },
                PlaceholderDef {
                    name: "content".to_string(),
                    frame: Rect::new(457_200.0, 1_828_800.0, 11_277_600.0, 4_343_400.0),
                },
            ],
        };
        let mj = serde_json::to_string(&master).expect("serialize master");
        let mr: Master = serde_json::from_str(&mj).expect("deserialize master");
        assert_eq!(master, mr);
        assert!(mj.contains("\"background_shapes\""));
        assert!(mj.contains("\"placeholders\""));

        // Empty master default-constructs and round-trips.
        let empty = Master::default();
        assert!(empty.background_shapes.is_empty());
        let ej = serde_json::to_string(&empty).expect("serialize empty master");
        let er: Master = serde_json::from_str(&ej).expect("deserialize empty master");
        assert_eq!(empty, er);

        let layout = Layout {
            name: "Title and Content".to_string(),
            placeholders: vec![PlaceholderDef {
                name: "content".to_string(),
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
            }],
        };
        let lj = serde_json::to_string(&layout).expect("serialize layout");
        let lr: Layout = serde_json::from_str(&lj).expect("deserialize layout");
        assert_eq!(layout, lr);
        assert_eq!(Layout::default().name, "Blank");
        assert_eq!(Master::default_16_9(), Master::default());
    }

    #[test]
    fn shape_id_accessor_returns_id() {
        let shape = Shape::TextBox(TextBox {
            id: "test-1".to_string(),
            frame: Rect::new(0.0, 0.0, 100.0, 100.0),
            paragraphs: Vec::new(),
        });
        assert_eq!(shape.id(), "test-1");

        // Passthrough exposes its own id through the same accessor.
        let passthrough = Shape::Passthrough(PassthroughObject {
            id: "p-7".to_string(),
            label: "sp".to_string(),
            source_part: String::new(),
            raw_bytes: Vec::new(),
            frame: None,
        });
        assert_eq!(passthrough.id(), "p-7");
    }

    #[test]
    fn shape_set_id_updates() {
        let mut shape = Shape::Geometric(GeometricShape {
            id: String::new(),
            transform: Transform::default(),
            geometry: Geometry::Rectangle,
            style: Style::default(),
        });
        assert_eq!(shape.id(), "");
        shape.set_id("geo-42".to_string());
        assert_eq!(shape.id(), "geo-42");
    }

    #[test]
    fn shape_generate_id_returns_unique() {
        let a = Shape::generate_id();
        let b = Shape::generate_id();
        assert!(!a.is_empty());
        assert_ne!(a, b);
    }

    #[test]
    fn morph_transition_kind_serializes() {
        let json = serde_json::to_string(&TransitionKind::Morph).expect("serialize Morph");
        assert_eq!(json, "\"morph\"");
        let back: TransitionKind = serde_json::from_str(&json).expect("deserialize Morph");
        assert_eq!(back, TransitionKind::Morph);
    }

    #[test]
    fn transition_with_morph_round_trips() {
        let slide = Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: Vec::new(),
            animation: None,
            transition: Some(Transition::new(TransitionKind::Morph, 800)),
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        };
        let json = serde_json::to_string(&slide).expect("serialize slide");
        let restored: Slide = serde_json::from_str(&json).expect("deserialize slide");
        assert_eq!(slide, restored);
        let transition = restored.transition.expect("transition present");
        assert_eq!(transition.kind, TransitionKind::Morph);
        assert_eq!(transition.duration_ms, 800);
    }

    #[test]
    fn old_deck_without_shape_ids_deserializes() {
        // A deck whose shapes carry stable ids.
        let slide = Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![
                Shape::TextBox(TextBox {
                    id: "tb-1".to_string(),
                    frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                    paragraphs: Vec::new(),
                }),
                Shape::Geometric(GeometricShape {
                    id: "geo-1".to_string(),
                    transform: Transform::default(),
                    geometry: Geometry::Rectangle,
                    style: Style::default(),
                }),
                Shape::Image(ImageShape {
                    id: "img-1".to_string(),
                    transform: Transform::default(),
                    media_ref: "key".to_string(),
                    crop: None,
                    alt_text: None,
                }),
            ],
            animation: None,
            transition: None,
            rich_notes: None,
            layout_ref: None,
            reduce_motion: None,
            rehearsed_duration_ms: None,
        };
        let mut deck = Deck::new();
        deck.slides.push(slide);

        let mut value = serde_json::to_value(&deck).expect("serialize to value");
        // Strip every shape's `id` from its content object, simulating an old
        // deck serialized before the id field existed.
        if let Some(slides) = value.get_mut("slides").and_then(|s| s.as_array_mut()) {
            for slide in slides {
                if let Some(shapes) = slide.get_mut("shapes").and_then(|s| s.as_array_mut()) {
                    for shape in shapes {
                        if let Some(content) =
                            shape.get_mut("value").and_then(|v| v.as_object_mut())
                        {
                            content.remove("id");
                        }
                    }
                }
            }
        }
        let stripped = serde_json::to_string(&value).expect("re-serialize stripped deck");
        let restored: Deck = serde_json::from_str(&stripped).expect("deserialize stripped deck");
        for shape in &restored.slides[0].shapes {
            assert!(
                shape.id().is_empty(),
                "old deck shape should deserialize with empty id, got {:?}",
                shape.id()
            );
        }
        // The deck is otherwise valid.
        assert_eq!(restored.slides[0].shapes.len(), 3);
    }

    // ===== Comment tests (Wave 17) ===========================================

    fn slide_with_shape(slide_id: &str, shape_id: &str) -> Slide {
        slide_with(
            slide_id,
            vec![Shape::Geometric(GeometricShape {
                id: shape_id.to_string(),
                transform: Transform::default(),
                geometry: Geometry::Rectangle,
                style: Style::default(),
            })],
        )
    }

    fn root_comment(body: &str) -> Comment {
        Comment {
            id: Shape::generate_id(),
            author: "Alice".to_string(),
            body: body.to_string(),
            timestamp: "2026-07-29T00:00:00Z".to_string(),
            resolved: false,
        }
    }

    fn thread_on_slide(slide_id: &str) -> CommentThread {
        CommentThread {
            id: Shape::generate_id(),
            anchor: CommentAnchor::Slide {
                slide_id: slide_id.to_string(),
            },
            comments: vec![root_comment("first")],
            assigned_to: None,
            resolved: false,
        }
    }

    #[test]
    fn old_deck_without_comments_deserializes() {
        // A deck serialized before comments existed has no `comments` key.
        // Build a valid deck in code, serialize it (the empty `comments` field
        // is skipped), then prove the payload round-trips into a deck with an
        // empty comments list.
        let mut deck = Deck::new();
        deck.id = "legacy".to_string();
        deck.slides.push(slide_with("s1", Vec::new()));

        let json = serde_json::to_string(&deck).expect("serialize deck");
        assert!(
            !json.contains("\"comments\""),
            "an empty-comments deck must not emit a `comments` key"
        );

        let restored: Deck = serde_json::from_str(&json).expect("deserialize legacy deck");
        assert!(restored.comments.is_empty());
        assert_eq!(restored.id, "legacy");
        assert_eq!(restored, deck);
    }

    #[test]
    fn add_comment_creates_thread() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(AddComment::new(
                CommentAnchor::Slide {
                    slide_id: "s1".to_string(),
                },
                "Alice",
                "Hello world",
            )),
            &mut deck,
        )
        .expect("apply");

        assert_eq!(deck.comments.len(), 1);
        let thread = &deck.comments[0];
        assert_eq!(thread.anchor.slide_id(), "s1");
        assert_eq!(thread.comments.len(), 1);
        assert_eq!(thread.comments[0].author, "Alice");
        assert_eq!(thread.comments[0].body, "Hello world");
        assert!(!thread.comments[0].resolved);
        assert!(!thread.resolved);
        assert!(thread.assigned_to.is_none());

        assert_eq!(
            AddComment::new(
                CommentAnchor::Slide {
                    slide_id: "s1".to_string(),
                },
                "Alice",
                "Hello world",
            )
            .affected_slide_ids(),
            vec!["s1".to_string()],
        );

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn add_comment_validates_slide_and_shape() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with_shape("s1", "shape-1"));
        let mut bus = CommandBus::default();

        // Unknown slide → rejected.
        assert_eq!(
            bus.apply(
                Box::new(AddComment::new(
                    CommentAnchor::Slide {
                        slide_id: "nope".to_string(),
                    },
                    "Alice",
                    "x",
                )),
                &mut deck,
            ),
            Err(CommandError::InvalidCommand)
        );

        // Shape anchor referencing a missing shape → rejected.
        assert_eq!(
            bus.apply(
                Box::new(AddComment::new(
                    CommentAnchor::Shape {
                        slide_id: "s1".to_string(),
                        shape_id: "missing".to_string(),
                    },
                    "Alice",
                    "x",
                )),
                &mut deck,
            ),
            Err(CommandError::InvalidCommand)
        );

        // Shape anchor with a real shape id → accepted.
        bus.apply(
            Box::new(AddComment::new(
                CommentAnchor::Shape {
                    slide_id: "s1".to_string(),
                    shape_id: "shape-1".to_string(),
                },
                "Alice",
                "on the shape",
            )),
            &mut deck,
        )
        .expect("valid shape anchor");
        assert_eq!(deck.comments.len(), 1);
    }

    #[test]
    fn add_comment_text_range_validates_offsets() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with_shape("s1", "shape-1"));
        let mut bus = CommandBus::default();

        // start > end → rejected.
        assert_eq!(
            bus.apply(
                Box::new(AddComment::new(
                    CommentAnchor::TextRange {
                        slide_id: "s1".to_string(),
                        shape_id: "shape-1".to_string(),
                        start: 5,
                        end: 2,
                    },
                    "Alice",
                    "bad range",
                )),
                &mut deck,
            ),
            Err(CommandError::InvalidCommand)
        );

        // start <= end with a real shape → accepted.
        bus.apply(
            Box::new(AddComment::new(
                CommentAnchor::TextRange {
                    slide_id: "s1".to_string(),
                    shape_id: "shape-1".to_string(),
                    start: 2,
                    end: 5,
                },
                "Alice",
                "good range",
            )),
            &mut deck,
        )
        .expect("valid text range");
        assert_eq!(deck.comments.len(), 1);
    }

    #[test]
    fn reply_to_comment_appends_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let thread = thread_on_slide("s1");
        let thread_id = thread.id.clone();
        deck.comments.push(thread);
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(ReplyToComment::new(
                thread_id.clone(),
                "s1",
                "Bob",
                "a reply",
            )),
            &mut deck,
        )
        .expect("apply");

        let thread = deck.comment_thread(&thread_id).expect("thread exists");
        assert_eq!(thread.comments.len(), 2);
        assert_eq!(thread.comments[1].author, "Bob");
        assert_eq!(thread.comments[1].body, "a reply");

        // Replying to an unknown thread is rejected.
        assert_eq!(
            bus.apply(
                Box::new(ReplyToComment::new("missing", "s1", "Bob", "x")),
                &mut deck,
            ),
            Err(CommandError::InvalidCommand)
        );

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_comment_resolved_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let thread = thread_on_slide("s1");
        let thread_id = thread.id.clone();
        deck.comments.push(thread);
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetCommentResolved::new(thread_id.clone(), "s1", true)),
            &mut deck,
        )
        .expect("apply");
        assert!(deck.comment_thread(&thread_id).expect("thread").resolved);

        assert!(bus.undo(&mut deck).is_some());
        assert!(!deck.comment_thread(&thread_id).expect("thread").resolved);
        assert_eq!(deck, original);
    }

    #[test]
    fn assign_comment_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let thread = thread_on_slide("s1");
        let thread_id = thread.id.clone();
        deck.comments.push(thread);
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(AssignComment::new(
                thread_id.clone(),
                "s1",
                Some("Carol".to_string()),
            )),
            &mut deck,
        )
        .expect("apply");
        assert_eq!(
            deck.comment_thread(&thread_id).expect("thread").assigned_to,
            Some("Carol".to_string())
        );

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);

        // Clearing an existing assignee is also reversible.
        bus.apply(
            Box::new(AssignComment::new(
                thread_id.clone(),
                "s1",
                Some("Dan".to_string()),
            )),
            &mut deck,
        )
        .expect("assign");
        bus.apply(
            Box::new(AssignComment::new(thread_id.clone(), "s1", None)),
            &mut deck,
        )
        .expect("clear");
        assert!(deck
            .comment_thread(&thread_id)
            .expect("thread")
            .assigned_to
            .is_none());
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(
            deck.comment_thread(&thread_id).expect("thread").assigned_to,
            Some("Dan".to_string())
        );
    }

    #[test]
    fn delete_comment_thread_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        let thread = thread_on_slide("s1");
        let thread_id = thread.id.clone();
        deck.comments.push(thread);
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(DeleteCommentThread::new(thread_id.clone(), "s1")),
            &mut deck,
        )
        .expect("apply");
        assert!(deck.comments.is_empty());

        // Deleting an unknown thread is rejected.
        assert_eq!(
            bus.apply(
                Box::new(DeleteCommentThread::new("missing", "s1")),
                &mut deck,
            ),
            Err(CommandError::InvalidCommand)
        );

        // Undo re-inserts the removed thread in full.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
        assert_eq!(deck.comments.len(), 1);
        assert_eq!(deck.comments[0].id, thread_id);
        assert_eq!(deck.comments[0].comments.len(), 1);
    }

    #[test]
    fn comment_thread_serialize_and_deserialize() {
        let thread = CommentThread {
            id: "t1".to_string(),
            anchor: CommentAnchor::Shape {
                slide_id: "s1".to_string(),
                shape_id: "sh1".to_string(),
            },
            comments: vec![Comment {
                id: "c1".to_string(),
                author: "Alice".to_string(),
                body: "hi".to_string(),
                timestamp: "2026-07-29T12:00:00Z".to_string(),
                resolved: false,
            }],
            assigned_to: Some("Bob".to_string()),
            resolved: true,
        };

        let json = serde_json::to_string(&thread).expect("serialize thread");
        let restored: CommentThread = serde_json::from_str(&json).expect("deserialize thread");
        assert_eq!(thread, restored);

        // The assigned_to field round-trips and resolved flags are kept.
        assert!(json.contains("\"assigned_to\":\"Bob\""));
        assert!(json.contains("\"resolved\":true"));
    }

    #[test]
    fn comment_anchor_all_variants_serialize_correctly() {
        let slide = CommentAnchor::Slide {
            slide_id: "s1".to_string(),
        };
        let shape = CommentAnchor::Shape {
            slide_id: "s1".to_string(),
            shape_id: "sh1".to_string(),
        };
        let range = CommentAnchor::TextRange {
            slide_id: "s1".to_string(),
            shape_id: "sh1".to_string(),
            start: 3,
            end: 9,
        };

        // Tagged with `kind`; internally-tagged enums have no `value` wrapper.
        let slide_json = serde_json::to_value(&slide).expect("serialize slide anchor");
        assert_eq!(slide_json["kind"], "slide");
        assert_eq!(slide_json["slide_id"], "s1");

        let shape_json = serde_json::to_value(&shape).expect("serialize shape anchor");
        assert_eq!(shape_json["kind"], "shape");
        assert_eq!(shape_json["shape_id"], "sh1");

        let range_json = serde_json::to_value(&range).expect("serialize range anchor");
        assert_eq!(range_json["kind"], "text_range");
        assert_eq!(range_json["start"], 3);
        assert_eq!(range_json["end"], 9);

        // Each variant round-trips.
        assert_eq!(
            serde_json::from_str::<CommentAnchor>(&serde_json::to_string(&slide).unwrap()).unwrap(),
            slide
        );
        assert_eq!(
            serde_json::from_str::<CommentAnchor>(&serde_json::to_string(&shape).unwrap()).unwrap(),
            shape
        );
        assert_eq!(
            serde_json::from_str::<CommentAnchor>(&serde_json::to_string(&range).unwrap()).unwrap(),
            range
        );
    }

    #[test]
    fn deck_with_comments_round_trips() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", Vec::new()));
        deck.comments.push(CommentThread {
            id: "t1".to_string(),
            anchor: CommentAnchor::Slide {
                slide_id: "s1".to_string(),
            },
            comments: vec![Comment {
                id: "c1".to_string(),
                author: "Alice".to_string(),
                body: "top-level".to_string(),
                timestamp: "2026-07-29T08:30:00Z".to_string(),
                resolved: false,
            }],
            assigned_to: None,
            resolved: false,
        });

        let json = serde_json::to_string(&deck).expect("serialize deck");
        let restored: Deck = serde_json::from_str(&json).expect("deserialize deck");
        assert_eq!(deck, restored);
        assert_eq!(restored.comments.len(), 1);
        assert_eq!(restored.comments[0].id, "t1");

        // An empty-comments deck serializes without a `comments` key.
        let mut empty = deck.clone();
        empty.comments.clear();
        let empty_json = serde_json::to_string(&empty).expect("serialize empty");
        assert!(!empty_json.contains("\"comments\""));
    }

    fn slide_with_animation(id: &str, shapes: Vec<Shape>, steps: Vec<BuildStep>) -> Slide {
        let mut slide = slide_with(id, shapes);
        slide.animation = Some(Animation::new(steps));
        slide
    }

    #[test]
    fn trigger_default_is_on_click() {
        let step = BuildStep::new(0, BuildEffect::Fade, 100);
        assert_eq!(Trigger::default(), Trigger::OnClick);
        assert_eq!(step.trigger, Trigger::OnClick);
        assert_eq!(step.delay_ms, 0);
        assert_eq!(step.motion_path, None);
    }

    #[test]
    fn trigger_enum_serializes_all_variants() {
        assert_eq!(
            serde_json::to_string(&Trigger::OnClick).unwrap(),
            "\"on_click\""
        );
        assert_eq!(
            serde_json::to_string(&Trigger::WithPrevious).unwrap(),
            "\"with_previous\""
        );
        assert_eq!(
            serde_json::to_string(&Trigger::AfterPrevious).unwrap(),
            "\"after_previous\""
        );

        for (text, expected) in [
            ("\"on_click\"", Trigger::OnClick),
            ("\"with_previous\"", Trigger::WithPrevious),
            ("\"after_previous\"", Trigger::AfterPrevious),
        ] {
            let parsed: Trigger = serde_json::from_str(text).expect("parse trigger");
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn old_deck_without_trigger_fields_deserializes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with_animation(
            "s1",
            vec![geo_rectangle()],
            vec![BuildStep::new(0, BuildEffect::Fade, 200)],
        ));

        let mut value = serde_json::to_value(&deck).expect("serialize to value");
        // Strip the new BuildStep fields (trigger, delay_ms, motion_path) and
        // the new Slide field (reduce_motion), simulating a pre-Wave-19 deck.
        let slide = value.get_mut("slides").unwrap().get_mut(0).unwrap();
        slide.as_object_mut().unwrap().remove("reduce_motion");
        let step = slide
            .get_mut("animation")
            .unwrap()
            .get_mut("steps")
            .unwrap()
            .get_mut(0)
            .unwrap();
        let step_obj = step.as_object_mut().unwrap();
        step_obj.remove("trigger");
        step_obj.remove("delay_ms");
        step_obj.remove("motion_path");

        let old_json = serde_json::to_string(&value).expect("reserialize old deck");
        let restored: Deck = serde_json::from_str(&old_json).expect("old deck must load");

        let step = &restored.slides[0]
            .animation
            .as_ref()
            .expect("animation")
            .steps[0];
        assert_eq!(step.trigger, Trigger::OnClick);
        assert_eq!(step.delay_ms, 0);
        assert_eq!(step.motion_path, None);
        assert_eq!(restored.slides[0].reduce_motion, None);
    }

    #[test]
    fn build_step_with_motion_path_serializes() {
        let mut step = BuildStep::new(0, BuildEffect::MotionPath, 500);
        step.motion_path = Some(vec![
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(1_000_000.0, 0.0, 100.0, 100.0),
        ]);
        let json = serde_json::to_string(&step).expect("serialize step");
        assert!(
            json.contains("\"motion_path\""),
            "motion_path must serialize"
        );
        assert!(json.contains("\"motion_path\""));
        let restored: BuildStep = serde_json::from_str(&json).expect("deserialize step");
        assert_eq!(step, restored);
        assert_eq!(restored.effect, BuildEffect::MotionPath);
        assert_eq!(restored.motion_path.expect("path").len(), 2);

        // A step without a motion path omits the field entirely.
        let plain = BuildStep::new(0, BuildEffect::Fade, 100);
        let plain_json = serde_json::to_string(&plain).expect("serialize plain");
        assert!(!plain_json.contains("\"motion_path\""));
    }

    #[test]
    fn set_build_step_trigger_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with_animation(
            "s1",
            vec![geo_rectangle(), geo_rectangle()],
            vec![
                BuildStep::new(0, BuildEffect::Fade, 200),
                BuildStep::new(1, BuildEffect::Appear, 100),
            ],
        ));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetBuildStepTrigger::new("s1", 1, Trigger::WithPrevious)),
            &mut deck,
        )
        .expect("set with_previous");
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[1].trigger,
            Trigger::WithPrevious
        );
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[0].trigger,
            Trigger::OnClick,
            "untouched step keeps its default"
        );

        bus.apply(
            Box::new(SetBuildStepTrigger::new("s1", 1, Trigger::AfterPrevious)),
            &mut deck,
        )
        .expect("set after_previous");
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[1].trigger,
            Trigger::AfterPrevious
        );

        // Undo restores the prior value at each step.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[1].trigger,
            Trigger::WithPrevious
        );
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_build_step_trigger_rejects_bad_index_and_missing_slide() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with_animation(
            "s1",
            vec![geo_rectangle()],
            vec![BuildStep::new(0, BuildEffect::Fade, 100)],
        ));
        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(SetBuildStepTrigger::new("s1", 9, Trigger::WithPrevious)),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(
            bus.apply(
                Box::new(SetBuildStepTrigger::new(
                    "missing",
                    0,
                    Trigger::WithPrevious
                )),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[0].trigger,
            Trigger::OnClick
        );
    }

    #[test]
    fn set_build_step_delay_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with_animation(
            "s1",
            vec![geo_rectangle()],
            vec![BuildStep::new(0, BuildEffect::Fade, 200)],
        ));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(Box::new(SetBuildStepDelay::new("s1", 0, 350)), &mut deck)
            .expect("set delay");
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[0].delay_ms,
            350
        );
        // A second set changes it; undo restores each prior value.
        bus.apply(Box::new(SetBuildStepDelay::new("s1", 0, 0)), &mut deck)
            .expect("reset delay");
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[0].delay_ms,
            0
        );
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[0].delay_ms,
            350
        );
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_build_step_motion_path_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with_animation(
            "s1",
            vec![geo_rectangle()],
            vec![BuildStep::new(0, BuildEffect::MotionPath, 500)],
        ));
        let original = deck.clone();

        let path = Some(vec![
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(500.0, 0.0, 100.0, 100.0),
        ]);
        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetBuildStepMotionPath::new("s1", 0, path.clone())),
            &mut deck,
        )
        .expect("set path");
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[0].motion_path,
            path
        );

        // Clearing is reversible.
        bus.apply(
            Box::new(SetBuildStepMotionPath::new("s1", 0, None)),
            &mut deck,
        )
        .expect("clear path");
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[0].motion_path,
            None
        );
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(
            deck.slides[0].animation.as_ref().expect("animation").steps[0].motion_path,
            path
        );
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_slide_reduce_motion_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetSlideReduceMotion::new("s1", Some(true))),
            &mut deck,
        )
        .expect("enable reduce motion");
        assert_eq!(deck.slides[0].reduce_motion, Some(true));
        bus.apply(
            Box::new(SetSlideReduceMotion::new("s1", Some(false))),
            &mut deck,
        )
        .expect("explicitly disable reduce motion");
        assert_eq!(deck.slides[0].reduce_motion, Some(false));
        bus.apply(Box::new(SetSlideReduceMotion::new("s1", None)), &mut deck)
            .expect("clear reduce motion");
        assert_eq!(deck.slides[0].reduce_motion, None);

        // Undo walks back through each prior value.
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slides[0].reduce_motion, Some(false));
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slides[0].reduce_motion, Some(true));
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_slide_rehearsed_duration_applies_and_undoes() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let original = deck.clone();

        let mut bus = CommandBus::default();
        bus.apply(
            Box::new(SetSlideRehearsedDuration::new("s1", Some(5000))),
            &mut deck,
        )
        .expect("set duration");
        assert_eq!(deck.slides[0].rehearsed_duration_ms, Some(5000));

        bus.apply(
            Box::new(SetSlideRehearsedDuration::new("s1", None)),
            &mut deck,
        )
        .expect("clear duration");
        assert_eq!(deck.slides[0].rehearsed_duration_ms, None);

        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck.slides[0].rehearsed_duration_ms, Some(5000));
        assert!(bus.undo(&mut deck).is_some());
        assert_eq!(deck, original);
    }

    #[test]
    fn set_slide_reduce_motion_rejects_missing_slide() {
        let mut deck = Deck::new();
        deck.slides.push(slide_with("s1", vec![geo_rectangle()]));
        let mut bus = CommandBus::default();
        assert_eq!(
            bus.apply(
                Box::new(SetSlideReduceMotion::new("missing", Some(true))),
                &mut deck
            ),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(bus.undo_len(), 0);
        assert_eq!(deck.slides[0].reduce_motion, None);
    }
}
