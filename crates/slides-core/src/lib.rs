//! Deck model, commands, undo, theme.

use std::collections::BTreeMap;

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
    /// Key-addressed store of image bytes referenced by image shapes.
    #[serde(default)]
    pub media: MediaStore,
}

impl Default for Deck {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: String::new(),
            theme: Theme::default(),
            slides: Vec::new(),
            media: MediaStore::default(),
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
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::white(),
            heading_font: String::from("Calibri"),
            body_font: String::from("Calibri"),
            accent_color: Color::rgb(0, 112, 192),
        }
    }
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
    /// Reserved animation field for future use.
    pub animation: Option<Animation>,
    /// Reserved transition field for future use.
    pub transition: Option<Transition>,
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
}

/// An editable text box shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBox {
    /// Bounding rectangle of the text box, in EMU.
    pub frame: Rect,
    /// Paragraphs of text inside the box.
    pub paragraphs: Vec<Paragraph>,
}

/// An image placed on a slide, referencing bytes in the deck's [`MediaStore`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageShape {
    /// Position, size, and rotation of the image.
    pub transform: Transform,
    /// Key of this image's bytes in the deck's [`MediaStore`].
    pub media_ref: String,
    /// Optional crop applied to the image.
    pub crop: Option<Crop>,
}

/// A geometric shape placed on a slide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricShape {
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
    /// - `javascript:`, `vbscript:`, `mocha:`, `livescript:`, `http:`,
    ///   `https:`, `file:`, and `data:` schemes (case-insensitive prefix),
    /// - any value with a colon before the first slash (i.e. an unknown scheme),
    /// - any control character (U+0000..=U+001F or U+007F).
    ///
    /// Allowed: `mailto:`, `tel:`, `#fragment`, and schemeless relative paths.
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
            "http:",
            "https:",
            "file:",
            "data:",
        ];
        if let Some(scheme) = DANGEROUS_SCHEMES.iter().find(|s| lowered.starts_with(*s)) {
            return Err(LinkError::DisallowedScheme(
                scheme.trim_end_matches(':').to_string(),
            ));
        }

        const ALLOWED_SCHEMES: &[&str] = &["mailto:", "tel:"];
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

/// Reserved animation field for future use.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Animation;

/// Reserved transition field for future use.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transition;

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

/// Transactional command bus with bounded undo history.
///
/// The bus does not own the [`Deck`]; it is passed in on each call so the same
/// model can be shared with other layers.
#[derive(Debug, Default)]
pub struct CommandBus {
    undo_stack: Vec<Box<dyn Command>>,
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
    /// onto the undo stack.
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
        Ok(())
    }

    /// Pops the most recent transaction and applies its inverse.
    ///
    /// Returns the affected slide ids if a command was undone, or `None` if the
    /// history was empty.
    pub fn undo(&mut self, deck: &mut Deck) -> Option<Vec<String>> {
        let inverse = self.undo_stack.pop()?;
        let affected = inverse.affected_slide_ids();
        self.total_size = self.total_size.saturating_sub(inverse.serialized_size());
        inverse.apply(deck);
        Some(affected)
    }

    /// Returns the number of transactions that can currently be undone.
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns the total serialized size of the stored inverse transactions.
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
                transform: self.transform,
                media_ref: self.media_key.clone(),
                crop: self.crop.clone(),
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

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_exist() {
        let _deck = Deck::new();
        let _slide = Slide::default();
        let _shape = Shape::TextBox(TextBox {
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
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("before")],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
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
    fn command_bus_rejects_oversized_transaction() {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            notes: String::new(),
            shapes: vec![Shape::TextBox(TextBox {
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("seed")],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
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
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("seed")],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
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
                transform: Transform::default(),
                media_ref: "only".to_string(),
                crop: None,
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
                transform: Transform::default(),
                media_ref: "shared".to_string(),
                crop: None,
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
                    frame: Rect::new(0.0, 0.0, 10.0, 10.0),
                    paragraphs: Vec::new(),
                }),
                Shape::Geometric(GeometricShape {
                    transform: Transform {
                        frame: Rect::new(0.0, 0.0, 10.0, 10.0),
                        rotation: 0.0,
                    },
                    geometry: Geometry::Rectangle,
                    style: Style::default(),
                }),
                Shape::Image(ImageShape {
                    transform: Transform {
                        frame: Rect::new(0.0, 0.0, 10.0, 10.0),
                        rotation: 0.0,
                    },
                    media_ref: "x".to_string(),
                    crop: None,
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
                    frame: Rect::new(0.0, 0.0, 1.0, 1.0),
                    paragraphs: Vec::new(),
                }),
                Shape::Image(ImageShape {
                    transform: Transform::default(),
                    media_ref: "m".to_string(),
                    crop: None,
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
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("legacy").bold().italic()],
                    list_style: ListStyle::Ordered,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
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

        assert_eq!(
            Link::new("http://example.com"),
            Err(LinkError::DisallowedScheme("http".to_string()))
        );
        assert_eq!(
            Link::new("https://example.com"),
            Err(LinkError::DisallowedScheme("https".to_string()))
        );
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
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("seed").bold().strikethrough().superscript()],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
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
                frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("seed").bold()],
                    list_style: ListStyle::None,
                    ..Default::default()
                }],
            })],
            animation: None,
            transition: None,
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
                    frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                    paragraphs: vec![Paragraph {
                        runs: vec![Run::new("seed")],
                        list_style: ListStyle::None,
                        ..Default::default()
                    }],
                }),
                Shape::Geometric(GeometricShape {
                    transform: Transform::default(),
                    geometry: Geometry::Rectangle,
                    style: Style::default(),
                }),
            ],
            animation: None,
            transition: None,
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
                    frame: Rect::new(0.0, 0.0, 100.0, 100.0),
                    paragraphs: Vec::new(),
                }),
                Shape::Image(ImageShape {
                    transform: Transform::default(),
                    media_ref: "m".to_string(),
                    crop: None,
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
}
