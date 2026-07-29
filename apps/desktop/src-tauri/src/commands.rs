//! Tauri commands for the 900Slides desktop application.
//!
//! This module exposes the v0.1.0 command surface: deck creation, opening,
//! saving, text editing, undo, presenter mode, and recovery snapshots. Every
//! mutation is applied transactionally in Rust, and the frontend always
//! re-renders from the returned deck snapshot.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

/// Default PPTX 16:9 slide dimensions, in EMU.
const SLIDE_WIDTH_EMU: f64 = 12_192_000.0;
/// Default PPTX 16:9 slide dimensions, in EMU.
const SLIDE_HEIGHT_EMU: f64 = 6_858_000.0;

/// Global application state held by Tauri.
///
/// Holds the active PPTX editing session, presenter navigation index, and
/// recovery tracking. The session is the canonical source of truth for the
/// deck; the Svelte frontend is a read-and-command projection.
pub struct AppState {
    /// The active PPTX editing session, if any.
    pub session: Mutex<Option<slides_pptx::Session>>,
    /// Recovery tracking and directory.
    pub recovery: Mutex<RecoveryTracker>,
    /// Index of the slide currently shown in presenter mode.
    pub presenter_index: Mutex<usize>,
    /// Monotonic token for debounced recovery writes.
    pub recovery_token: AtomicU64,
    /// Cache of the base64-encoded media DTO, reused across snapshots whose
    /// media store has not changed so that non-media commands (text edits,
    /// moves, style changes) do not re-encode every image on every keystroke.
    pub media_cache: Mutex<MediaCache>,
    /// Offline en-US spell checker (bundled dictionary + learned user words).
    /// Held behind a Mutex because learning a word needs `&mut self`.
    pub spell: Mutex<slides_spell::SpellChecker>,
    /// Path to the newline-delimited user dictionary file. `spell_add_word`
    /// appends learned words here; on startup the checker reloads them.
    pub user_dictionary_path: PathBuf,
}

/// Cached media snapshot, keyed by a content fingerprint of the deck's media.
#[derive(Debug, Default)]
pub struct MediaCache {
    /// Fingerprint the cached `dto` corresponds to (`u64::MAX` forces a rebuild).
    pub fingerprint: u64,
    /// Last-encoded media DTO map.
    pub dto: BTreeMap<String, MediaEntryDto>,
}

/// Tracks pending recovery writes and the directory they are stored in.
pub struct RecoveryTracker {
    /// Directory where recovery snapshots are written.
    pub dir: PathBuf,
    /// Token of the most recently scheduled recovery write.
    pub pending_token: u64,
    /// Serialized PPTX bytes waiting to be written.
    pub pending_bytes: Option<Vec<u8>>,
    /// Deck id for the pending recovery write.
    pub pending_deck_id: Option<String>,
}

impl AppState {
    /// Creates a new application state with the default recovery directory.
    pub fn new() -> Self {
        let app_dir = dirs::data_dir().unwrap_or_default().join("900Slides");
        fs::create_dir_all(&app_dir).ok();
        let recovery_dir = app_dir.join("recovery");
        fs::create_dir_all(&recovery_dir).ok();
        let user_dictionary_path = app_dir.join("user-dictionary.txt");
        Self {
            session: Mutex::new(None),
            recovery: Mutex::new(RecoveryTracker {
                dir: recovery_dir,
                pending_token: 0,
                pending_bytes: None,
                pending_deck_id: None,
            }),
            presenter_index: Mutex::new(0),
            recovery_token: AtomicU64::new(0),
            media_cache: Mutex::new(MediaCache::default()),
            spell: Mutex::new(slides_spell::SpellChecker::new()),
            user_dictionary_path,
        }
    }
}

impl AppState {
    /// Builds a [`DeckSnapshot`], reusing the cached media DTO when the deck's
    /// media store is unchanged so that non-media commands do not re-encode
    /// every image's bytes to base64 on each keystroke.
    pub fn snapshot(&self, deck: &slides_core::Deck) -> DeckSnapshot {
        let fingerprint = media_fingerprint(deck);
        let media = {
            let mut cache = self.media_cache.lock().expect("media cache mutex poisoned");
            if cache.fingerprint != fingerprint {
                cache.dto = media_to_dto(&deck.media);
                cache.fingerprint = fingerprint;
            }
            cache.dto.clone()
        };
        DeckSnapshot {
            id: deck.id.clone(),
            schema_version: deck.schema_version,
            theme: theme_to_dto(&deck.theme),
            slide_size: deck.slide_size.as_ref().map(slide_size_to_dto),
            sections: deck.sections.iter().map(section_to_dto).collect(),
            slides: deck.slides.iter().map(slide_to_dto).collect(),
            media,
            presenter_settings: presenter_settings_to_dto(&deck.presenter_settings),
            warnings: Vec::new(),
        }
    }

    /// Loads the persisted user dictionary (if any) into the spell checker.
    ///
    /// Called once at startup. Read errors are ignored gracefully: a missing
    /// or unreadable file simply yields an empty user dictionary, so spell
    /// checking still works from the bundled en-US word list.
    pub fn load_user_dictionary(&self) {
        let Ok(content) = fs::read_to_string(&self.user_dictionary_path) else {
            return;
        };
        let Ok(mut checker) = self.spell.lock() else {
            return;
        };
        for line in content.lines() {
            let word = line.trim();
            if !word.is_empty() {
                checker.add_user_word(word);
            }
        }
    }
}

/// Computes a runtime fingerprint of a deck's media store. Only used to decide
/// whether the cached base64 DTO is still valid; not persisted, so a
/// non-stable hasher is acceptable.
fn media_fingerprint(deck: &slides_core::Deck) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    let mut hasher = DefaultHasher::new();
    hasher.write(deck.id.as_bytes());
    hasher.write_u64(deck.media.len() as u64);
    for (key, entry) in deck.media.iter() {
        hasher.write(key.as_bytes());
        hasher.write(entry.mime.as_bytes());
        hasher.write(&entry.bytes);
        hasher.write_u32(entry.width);
        hasher.write_u32(entry.height);
    }
    hasher.finish()
}

/// Snapshot of a deck sent to the frontend after every command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckSnapshot {
    /// Stable deck identifier.
    pub id: String,
    /// Deck model schema version.
    pub schema_version: u32,
    /// Theme applied to the whole deck.
    pub theme: ThemeSnapshot,
    /// Fixed slide dimensions (aspect ratio), when set. When `None`, the deck
    /// renders at the default 16:9.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide_size: Option<SlideSizeDto>,
    /// Named slide sections, in slide order.
    #[serde(default)]
    pub sections: Vec<SlideSectionDto>,
    /// Ordered slides in the deck.
    pub slides: Vec<SlideSnapshot>,
    /// Media store: image bytes keyed by their media reference, base64-encoded
    /// so the frontend can render images directly from the snapshot.
    #[serde(default)]
    pub media: BTreeMap<String, MediaEntryDto>,
    /// Presenter settings (laser pointer, highlighter).
    #[serde(default)]
    pub presenter_settings: PresenterSettingsDto,
    /// Warnings from the last load (empty for most commands).
    pub warnings: Vec<WarningDto>,
}

/// Fixed slide dimensions, in EMU, used to pin the deck's aspect ratio.
/// Mirrors [`slides_core::SlideSize`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideSizeDto {
    /// Slide width, in EMU.
    pub width_emu: f64,
    /// Slide height, in EMU.
    pub height_emu: f64,
}

/// A named slide section that starts at a given slide. Mirrors
/// [`slides_core::SlideSection`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideSectionDto {
    /// Human-readable section name.
    pub name: String,
    /// Id of the first slide in this section.
    pub start_slide_id: String,
}

/// Snapshot of a theme for the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSnapshot {
    /// Background color.
    pub background: ColorDto,
    /// Heading font family.
    pub heading_font: String,
    /// Body font family.
    pub body_font: String,
    /// Accent color.
    pub accent_color: ColorDto,
    /// High-contrast accessibility mode.
    #[serde(default)]
    pub high_contrast: bool,
}

/// RGBA color sent to the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorDto {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel.
    pub a: u8,
}

/// Presenter settings (laser pointer + highlighter), mirroring
/// [`slides_core::PresenterSettings`]. Laser and highlighter colors are stored
/// as CSS hex strings (e.g. `#ff0000`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenterSettingsDto {
    /// Whether the laser pointer is enabled by default.
    #[serde(default)]
    pub laser_pointer: bool,
    /// Laser pointer color as a CSS hex string. Defaults to red.
    #[serde(default = "default_laser_color_dto")]
    pub laser_color: String,
    /// Whether the highlighter tool is enabled by default.
    #[serde(default)]
    pub highlighter: bool,
    /// Highlighter color as a CSS hex string. Defaults to yellow.
    #[serde(default = "default_highlighter_color_dto")]
    pub highlighter_color: String,
}

impl Default for PresenterSettingsDto {
    fn default() -> Self {
        Self {
            laser_pointer: false,
            laser_color: default_laser_color_dto(),
            highlighter: false,
            highlighter_color: default_highlighter_color_dto(),
        }
    }
}

/// Default laser pointer color, matching the model default.
fn default_laser_color_dto() -> String {
    String::from("#ff0000")
}

/// Default highlighter color, matching the model default.
fn default_highlighter_color_dto() -> String {
    String::from("#ffff00")
}

/// Converts a model [`slides_core::PresenterSettings`] into its DTO.
fn presenter_settings_to_dto(settings: &slides_core::PresenterSettings) -> PresenterSettingsDto {
    PresenterSettingsDto {
        laser_pointer: settings.laser_pointer,
        laser_color: settings.laser_color.clone(),
        highlighter: settings.highlighter,
        highlighter_color: settings.highlighter_color.clone(),
    }
}

/// Converts a [`PresenterSettingsDto`] into the model type.
fn presenter_settings_from_dto(dto: &PresenterSettingsDto) -> slides_core::PresenterSettings {
    slides_core::PresenterSettings {
        laser_pointer: dto.laser_pointer,
        laser_color: dto.laser_color.clone(),
        highlighter: dto.highlighter,
        highlighter_color: dto.highlighter_color.clone(),
    }
}

/// Snapshot of a single slide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideSnapshot {
    /// Stable slide identifier.
    pub id: String,
    /// Plain-text speaker notes.
    pub notes: String,
    /// Shapes on this slide.
    pub shapes: Vec<ShapeSnapshot>,
    /// Slide-to-slide transition played when advancing to this slide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionDto>,
    /// Ordered build-in animation sequence for this slide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<AnimationDto>,
    /// Rich-text speaker notes, when present. When `None`, the plain
    /// [`SlideSnapshot::notes`] field is used instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rich_notes: Option<Vec<ParagraphDto>>,
}

/// Snapshot of a slide-to-slide transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionDto {
    /// Kind of transition.
    pub kind: TransitionKindDto,
    /// Duration in milliseconds.
    pub duration_ms: u32,
}

/// The kind of slide-to-slide transition, mirroring [`slides_core::TransitionKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKindDto {
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
}

/// Snapshot of an ordered build-in animation sequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationDto {
    /// Ordered build steps.
    pub steps: Vec<BuildStepDto>,
}

/// Snapshot of a single build-in step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildStepDto {
    /// Index into `slide.shapes` of the shape this step reveals (or hides).
    pub shape_index: usize,
    /// The reveal or hide effect.
    pub effect: BuildEffectDto,
    /// Duration of the effect in milliseconds.
    pub duration_ms: u32,
}

/// The reveal or hide effect for a build step, mirroring [`slides_core::BuildEffect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildEffectDto {
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
}

/// Snapshot of a shape for the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ShapeSnapshot {
    /// An editable text box.
    TextBox(TextBoxSnapshot),
    /// An opaque, byte-for-byte preserved object.
    Passthrough(PassthroughSnapshot),
    /// An image referencing bytes in the deck media store.
    Image(ImageShapeSnapshot),
    /// A geometric shape.
    Geometric(GeometricShapeSnapshot),
    /// A table: a grid of editable cells.
    Table(TableShapeSnapshot),
    /// A chart: a data visualization rendered as SVG.
    Chart(ChartShapeSnapshot),
}

/// Snapshot of a chart shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartShapeSnapshot {
    /// Position, size, and rotation of the chart.
    pub transform: TransformDto,
    /// Kind of chart.
    pub chart_type: ChartTypeDto,
    /// Data plotted by the chart.
    pub data: ChartDataDto,
    /// Optional chart title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// The kind of chart, mirroring [`slides_core::ChartType`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartTypeDto {
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

/// Data backing a chart, mirroring [`slides_core::ChartData`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ChartDataDto {
    /// Category-aligned data for bar, column, line, area, and pie charts.
    Category {
        /// Category labels, shared across every series.
        categories: Vec<String>,
        /// One or more value series, each aligned with `categories`.
        series: Vec<CategorySeriesDto>,
    },
    /// XY (scatter) data.
    #[serde(rename = "xy")]
    XY {
        /// One or more point series.
        series: Vec<XYSeriesDto>,
    },
}

/// A value series aligned with a set of categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategorySeriesDto {
    /// Series name, shown in the legend.
    #[serde(default)]
    pub name: String,
    /// One numeric value per category.
    pub values: Vec<f64>,
}

/// A series of (x, y) points for scatter charts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XYSeriesDto {
    /// Series name, shown in the legend.
    #[serde(default)]
    pub name: String,
    /// Ordered (x, y) pairs.
    pub points: Vec<XYPointDto>,
}

/// A single (x, y) point in an [`XYSeriesDto`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XYPointDto {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

/// Snapshot of an image shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageShapeSnapshot {
    /// Position, size, and rotation of the image.
    pub transform: TransformDto,
    /// Key of this image's bytes in the deck media store.
    pub media_ref: String,
    /// Optional crop applied to the image.
    pub crop: Option<CropDto>,
}

/// Snapshot of a geometric shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometricShapeSnapshot {
    /// Position, size, and rotation of the shape.
    pub transform: TransformDto,
    /// Primitive geometry of the shape.
    pub geometry: GeometryDto,
    /// Visual style of the shape.
    pub style: StyleDto,
}

/// Horizontal alignment of cell text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CellAlignDto {
    /// Left-aligned text.
    #[default]
    Left,
    /// Centered text.
    Center,
    /// Right-aligned text.
    Right,
}

/// A single border edge: color, width, and dash style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorderEdgeDto {
    /// Edge color.
    pub color: ColorDto,
    /// Width in EMU.
    pub width_emu: f64,
    /// Dash pattern of the edge.
    #[serde(default)]
    pub dash: DashStyleDto,
}

/// The four borders of a cell (or the table default).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableBordersDto {
    /// Top edge, if any.
    #[serde(default)]
    pub top: Option<BorderEdgeDto>,
    /// Bottom edge, if any.
    #[serde(default)]
    pub bottom: Option<BorderEdgeDto>,
    /// Left edge, if any.
    #[serde(default)]
    pub left: Option<BorderEdgeDto>,
    /// Right edge, if any.
    #[serde(default)]
    pub right: Option<BorderEdgeDto>,
}

/// A single cell in a table.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCellDto {
    /// Plain-text content of the cell.
    #[serde(default)]
    pub text: String,
    /// Cell fill, or `None` to inherit the table default.
    #[serde(default)]
    pub fill: Option<FillDto>,
    /// Cell-level border overrides. When `None`, inherit the table default.
    #[serde(default)]
    pub borders: Option<TableBordersDto>,
    /// Horizontal alignment of the cell text.
    #[serde(default)]
    pub align: CellAlignDto,
}

/// A single row of cells in a table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRowDto {
    /// Row height in EMU.
    pub height: f64,
    /// Cells, left to right.
    pub cells: Vec<TableCellDto>,
}

/// Snapshot of a table shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableShapeSnapshot {
    /// Position, size, and rotation of the table.
    pub transform: TransformDto,
    /// Rows, top to bottom.
    pub rows: Vec<TableRowDto>,
    /// Per-column width in EMU.
    pub column_widths: Vec<f64>,
    /// Default cell borders applied when a cell has no explicit border.
    #[serde(default)]
    pub default_borders: TableBordersDto,
    /// Whether the first row is rendered as a header (bold, distinct fill).
    #[serde(default)]
    pub header_row: bool,
}

/// Placement of a shape: a bounding frame plus a rotation around its center.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformDto {
    /// Bounding rectangle, in EMU.
    pub frame: RectDto,
    /// Rotation around the frame center, in degrees.
    pub rotation: f64,
}

/// The geometric primitive a shape is built from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryDto {
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

/// Visual style applied to a geometric shape.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleDto {
    /// Interior fill, if any.
    pub fill: Option<FillDto>,
    /// Outline (stroke), if any.
    pub outline: Option<OutlineDto>,
    /// Drop shadow, if any.
    pub shadow: Option<ShadowDto>,
}

/// Fill applied to a shape's interior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillDto {
    /// A single solid color.
    Solid(ColorDto),
}

/// Outline (stroke) of a shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineDto {
    /// Stroke color.
    pub color: ColorDto,
    /// Stroke width, in EMU.
    pub width_emu: f64,
    /// Dash pattern of the stroke.
    pub dash: DashStyleDto,
}

/// Dash pattern for an outline.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashStyleDto {
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

/// Drop shadow drawn behind a shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowDto {
    /// Horizontal offset, in EMU.
    pub offset_x: f64,
    /// Vertical offset, in EMU.
    pub offset_y: f64,
    /// Blur radius, in EMU.
    pub blur: f64,
    /// Shadow color.
    pub color: ColorDto,
    /// Shadow opacity, in the range `0.0..=1.0`.
    pub opacity: f64,
}

/// Crop applied to an image, as fractions of its native size in `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropDto {
    /// Fraction cropped from the left edge.
    pub left: f64,
    /// Fraction cropped from the top edge.
    pub top: f64,
    /// Fraction cropped from the right edge.
    pub right: f64,
    /// Fraction cropped from the bottom edge.
    pub bottom: f64,
}

/// Media entry sent to the frontend, with bytes base64-encoded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaEntryDto {
    /// MIME type of the stored bytes (e.g. `image/png`).
    pub mime: String,
    /// Raw media bytes, base64-encoded.
    pub bytes: String,
    /// Native pixel width of the media.
    pub width: u32,
    /// Native pixel height of the media.
    pub height: u32,
}

/// Snapshot of a text box shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextBoxSnapshot {
    /// Bounding rectangle in EMU.
    pub frame: RectDto,
    /// Paragraphs inside the text box.
    pub paragraphs: Vec<ParagraphDto>,
}

/// Bounding rectangle in EMU.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectDto {
    /// Horizontal position, in EMU.
    pub x: f64,
    /// Vertical position, in EMU.
    pub y: f64,
    /// Width, in EMU.
    pub width: f64,
    /// Height, in EMU.
    pub height: f64,
}

/// Snapshot of an opaque passthrough shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PassthroughSnapshot {
    /// Identifier from the source object.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Source part path.
    pub source_part: String,
    /// Bounding rectangle in EMU, if it could be parsed.
    pub frame: Option<RectDto>,
}

/// Vertical alignment of a run.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerticalAlignDto {
    /// Normal baseline alignment.
    #[default]
    Baseline,
    /// Superscript text.
    Superscript,
    /// Subscript text.
    Subscript,
}

/// Hyperlink attached to a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkDto {
    /// URL target of the link.
    pub url: String,
    /// Optional display text override.
    pub display: Option<String>,
}

/// Heading level for a paragraph style.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadingLevelDto {
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

/// Paragraph-level style data transfer object.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphStyleDto {
    /// Heading level, if this paragraph is a heading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<HeadingLevelDto>,
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

/// Paragraph data transfer object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphDto {
    /// Inline text runs.
    pub runs: Vec<RunDto>,
    /// List style of the paragraph.
    pub list_style: String,
    /// Paragraph-level style.
    #[serde(default)]
    pub style: ParagraphStyleDto,
}

/// Inline text run data transfer object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDto {
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
    /// Vertical alignment of the run.
    #[serde(default)]
    pub vertical_align: VerticalAlignDto,
    /// Hyperlink attached to this run.
    #[serde(default)]
    pub link: Option<LinkDto>,
    /// Inline code flag.
    #[serde(default)]
    pub code: bool,
    /// Run-level font family override.
    #[serde(default)]
    pub font_family: Option<String>,
}

/// Loss ledger warning data transfer object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningDto {
    /// Identifier of the affected slide.
    pub slide_id: String,
    /// Human-readable warning message.
    pub message: String,
}

/// A misspelled word and its byte span within the checked text. Mirrors
/// [`slides_spell::Misspelling`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MisspellingDto {
    /// The misspelled token exactly as it appeared in the source text.
    pub word: String,
    /// Inclusive byte offset of the token within the checked text.
    pub byte_start: usize,
    /// Exclusive byte offset of the token within the checked text.
    pub byte_end: usize,
}

/// State returned to the presenter window.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenterState {
    /// Current slide snapshot.
    pub current_slide: SlideSnapshot,
    /// Next slide snapshot, if any.
    pub next_slide: Option<SlideSnapshot>,
    /// One-based index of the current slide.
    pub slide_number: usize,
    /// Total number of slides.
    pub total: usize,
    /// Plain-text notes for the current slide.
    pub notes: String,
    /// Media store, base64-encoded, so presenter slides can render images.
    #[serde(default)]
    pub media: BTreeMap<String, MediaEntryDto>,
    /// Deck slide size (aspect ratio), when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide_size: Option<SlideSizeDto>,
    /// Whether the deck is rendered in high-contrast mode.
    #[serde(default)]
    pub high_contrast: bool,
    /// Presenter settings (laser pointer, highlighter colors and defaults).
    #[serde(default)]
    pub presenter_settings: PresenterSettingsDto,
}

/// Recovery snapshot metadata returned to the frontend.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySnapshot {
    /// Recovery file identifier (filename including `.pptx`).
    pub id: String,
    /// Timestamp string from the filename.
    pub timestamp: String,
    /// Deck identifier from the filename.
    pub deck_id: String,
}

/// Creates a new blank deck and returns its snapshot.
#[tauri::command]
pub fn new_deck(state: State<'_, AppState>) -> Result<DeckSnapshot, String> {
    let bytes = slides_pptx::create_blank_pptx();
    let mut session = slides_pptx::load(&bytes).map_err(|e| e.to_string())?;
    let fresh = slides_core::Deck::new();
    session.deck_mut().id = fresh.id;
    let snapshot = state.snapshot(session.deck());
    *state.session.lock().map_err(|e| e.to_string())? = Some(session);
    *state.presenter_index.lock().map_err(|e| e.to_string())? = 0;
    Ok(snapshot)
}

/// Opens a PPTX file from the given path and returns its deck snapshot.
#[tauri::command]
pub fn open_deck(path: String, state: State<'_, AppState>) -> Result<DeckSnapshot, String> {
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let session = slides_pptx::load(&bytes).map_err(|e| e.to_string())?;
    let mut snapshot = state.snapshot(session.deck());
    snapshot.warnings = session
        .loss_ledger()
        .warnings()
        .iter()
        .map(warning_to_dto)
        .collect();
    *state.session.lock().map_err(|e| e.to_string())? = Some(session);
    *state.presenter_index.lock().map_err(|e| e.to_string())? = 0;
    Ok(snapshot)
}

/// Saves the current deck to the given path.
#[tauri::command]
pub fn save_deck(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let bytes = slides_pptx::save(session).map_err(|e| e.to_string())?;
    session.commit_save(bytes.clone());
    let deck_id = session.deck().id.clone();
    drop(guard);
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    retire_recovery(&state, &deck_id);
    Ok(())
}

/// Internal command that atomically replaces a chart's data and type. Used
/// when the data-table editor sends data whose kind does not match the chart's
/// current type (e.g. switching to/from scatter).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SetChartDataAndType {
    slide_id: String,
    shape_index: usize,
    data: slides_core::ChartData,
    chart_type: slides_core::ChartType,
}

impl slides_core::Command for SetChartDataAndType {
    fn apply(&self, deck: &mut slides_core::Deck) {
        let Some(slide) = deck.slide_mut(&self.slide_id) else {
            return;
        };
        let Some(shape) = slide.shapes.get_mut(self.shape_index) else {
            return;
        };
        if let slides_core::Shape::Chart(chart) = shape {
            chart.data = self.data.clone();
            chart.chart_type = self.chart_type;
        }
    }

    fn inverse(&self, deck: &slides_core::Deck) -> Box<dyn slides_core::Command> {
        let (prior_data, prior_type) = deck
            .slide(&self.slide_id)
            .and_then(|slide| slide.shapes.get(self.shape_index))
            .and_then(|shape| match shape {
                slides_core::Shape::Chart(chart) => Some((chart.data.clone(), chart.chart_type)),
                _ => None,
            })
            .unwrap_or_else(|| (self.data.clone(), self.chart_type));
        Box::new(Self {
            slide_id: self.slide_id.clone(),
            shape_index: self.shape_index,
            data: prior_data,
            chart_type: prior_type,
        })
    }

    fn serialized_size(&self) -> usize {
        serde_json::to_string(self).map_or(0, |s| s.len())
    }

    fn affected_slide_ids(&self) -> Vec<String> {
        vec![self.slide_id.clone()]
    }

    fn validate(&self, deck: &slides_core::Deck) -> bool {
        let Some(slide) = deck.slide(&self.slide_id) else {
            return false;
        };
        let Some(shape) = slide.shapes.get(self.shape_index) else {
            return false;
        };
        let slides_core::Shape::Chart(chart) = shape else {
            return false;
        };
        slides_core::ChartShape::new(
            chart.transform,
            self.chart_type,
            self.data.clone(),
            chart.title.clone(),
        )
        .is_ok()
    }
}

/// Returns the current deck snapshot for re-rendering, or `None` if no deck is open.
#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> Option<DeckSnapshot> {
    let guard = state.session.lock().ok()?;
    guard.as_ref().map(|s| state.snapshot(s.deck()))
}

/// Edits a paragraph inside a text box and returns the updated deck snapshot.
#[tauri::command]
pub fn edit_text(
    slide_id: String,
    shape_index: usize,
    paragraph_index: usize,
    runs: Vec<RunDto>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let core_runs: Vec<slides_core::Run> = runs.iter().map(run_from_dto).collect();
    let command = Box::new(slides_core::EditText::new(
        slide_id,
        shape_index,
        paragraph_index,
        core_runs,
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Replaces all paragraphs in a text box and returns the updated deck snapshot.
#[tauri::command]
pub fn edit_text_box(
    slide_id: String,
    shape_index: usize,
    paragraphs: Vec<ParagraphDto>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let core_paragraphs: Vec<slides_core::Paragraph> =
        paragraphs.iter().map(paragraph_from_dto).collect();
    let command = Box::new(slides_core::EditTextBox::new(
        slide_id,
        shape_index,
        core_paragraphs,
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Merge-patches a run's style flags and returns the updated deck snapshot.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn set_run_style(
    slide_id: String,
    shape_index: usize,
    paragraph_index: usize,
    run_index: usize,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strikethrough: Option<bool>,
    vertical_align: Option<VerticalAlignDto>,
    code: Option<bool>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let mut command =
        slides_core::SetRunStyle::new(slide_id, shape_index, paragraph_index, run_index);
    if let Some(bold) = bold {
        command = command.bold(bold);
    }
    if let Some(italic) = italic {
        command = command.italic(italic);
    }
    if let Some(underline) = underline {
        command = command.underline(underline);
    }
    if let Some(strikethrough) = strikethrough {
        command = command.strikethrough(strikethrough);
    }
    if let Some(vertical_align) = vertical_align {
        command = match vertical_align {
            VerticalAlignDto::Baseline => command.baseline(),
            VerticalAlignDto::Superscript => command.superscript(),
            VerticalAlignDto::Subscript => command.subscript(),
        };
    }
    if let Some(code) = code {
        command = command.code(code);
    }
    session
        .execute(Box::new(command))
        .map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Replaces a paragraph's style and returns the updated deck snapshot.
#[tauri::command]
pub fn set_paragraph_style(
    slide_id: String,
    shape_index: usize,
    paragraph_index: usize,
    style: ParagraphStyleDto,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let core_style = paragraph_style_to_core(&style);
    let command = Box::new(slides_core::SetParagraphStyle::new(
        slide_id,
        shape_index,
        paragraph_index,
        core_style,
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Ingests image bytes, stores them in the deck media store, appends an image
/// shape to the slide, and returns the updated deck snapshot.
#[tauri::command]
pub fn insert_image(
    slide_id: String,
    bytes: Vec<u8>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let ingested = slides_media::ingest(&bytes, &slides_media::IngestOptions::default())
        .map_err(|e| e.to_string())?;
    let entry = slides_core::MediaEntry {
        mime: ingested.mime.to_string(),
        bytes: ingested.bytes.clone(),
        width: ingested.width,
        height: ingested.height,
    };
    // Use the same content-addressed key the PPTX loader uses, so inserting an
    // image that already exists (or re-inserting a loaded image) dedups to a
    // single media entry instead of creating duplicate package parts.
    let media_key = slides_pptx::media_key(&ingested.bytes);
    let transform = centered_transform(ingested.width, ingested.height);

    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::InsertImage::new(
        slide_id, media_key, entry, transform, None,
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Appends a geometric shape to a slide and returns the updated deck snapshot.
#[tauri::command]
pub fn add_shape(
    slide_id: String,
    geometry_kind: String,
    style: Option<StyleDto>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let geometry = geometry_from_kind(&geometry_kind);

    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let core_style = style
        .map(style_to_core)
        .unwrap_or_else(|| default_shape_style(&session.deck().theme));
    let shape = slides_core::Shape::Geometric(slides_core::GeometricShape {
        transform: centered_transform(0, 0),
        geometry,
        style: core_style,
    });
    let command = Box::new(slides_core::AddShape::new(slide_id, shape));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Updates a shape's transform (position/size/rotation) and returns the updated
/// deck snapshot.
#[tauri::command]
pub fn update_shape_transform(
    slide_id: String,
    shape_index: usize,
    transform: TransformDto,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::MoveShape::new(
        slide_id,
        shape_index,
        transform_to_core(transform),
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Updates a geometric shape's style and returns the updated deck snapshot.
#[tauri::command]
pub fn update_shape_style(
    slide_id: String,
    shape_index: usize,
    style: StyleDto,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::SetShapeStyle::new(
        slide_id,
        shape_index,
        style_to_core(style),
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Removes a shape from a slide and returns the updated deck snapshot.
#[tauri::command]
pub fn delete_shape(
    slide_id: String,
    shape_index: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::DeleteShape::new(slide_id, shape_index));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Appends a new `rows` x `cols` table to a slide and returns the updated deck
/// snapshot.
#[tauri::command]
pub fn add_table(
    slide_id: String,
    rows: usize,
    cols: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let table = slides_core::TableShape::default_grid(rows, cols, centered_table_frame());
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::AddTable::new(slide_id, table));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Sets the text of a single cell in a table shape and returns the updated
/// deck snapshot.
#[tauri::command]
pub fn set_cell_text(
    slide_id: String,
    shape_index: usize,
    row: usize,
    col: usize,
    text: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::SetCellText::new(
        slide_id,
        shape_index,
        row,
        col,
        text,
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Merge-patches a cell's style (fill, borders, and/or alignment) and returns
/// the updated deck snapshot. Pass `clear_fill: true` / `clear_borders: true`
/// to remove an existing fill / borders.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn set_cell_style(
    slide_id: String,
    shape_index: usize,
    row: usize,
    col: usize,
    fill: Option<FillDto>,
    clear_fill: Option<bool>,
    borders: Option<TableBordersDto>,
    clear_borders: Option<bool>,
    align: Option<CellAlignDto>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let mut command = slides_core::SetCellStyle::new(slide_id, shape_index, row, col);
    if clear_fill.unwrap_or(false) {
        command = command.fill(None);
    } else if let Some(fill) = fill {
        command = command.fill(Some(fill_to_core(fill)));
    }
    if clear_borders.unwrap_or(false) {
        command = command.borders(None);
    } else if let Some(borders) = borders {
        command = command.borders(Some(table_borders_to_core(borders)));
    }
    if let Some(align) = align {
        command = command.align(cell_align_to_core(align));
    }
    session
        .execute(Box::new(command))
        .map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Inserts an empty row into a table at `index` and returns the updated deck
/// snapshot. The new row matches the table's column count and the height of
/// the last row.
#[tauri::command]
pub fn insert_row(
    slide_id: String,
    shape_index: usize,
    index: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let (col_count, row_height) = table_grid_metrics(session.deck(), &slide_id, shape_index)?;
    let row = slides_core::TableRow {
        height: row_height,
        cells: (0..col_count)
            .map(|_| slides_core::TableCell::default())
            .collect(),
    };
    let command = Box::new(slides_core::InsertRow::new(
        slide_id,
        shape_index,
        index,
        row,
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Inserts an empty column into a table at `index` and returns the updated
/// deck snapshot. The new column's width is the table's average column width.
#[tauri::command]
pub fn insert_column(
    slide_id: String,
    shape_index: usize,
    index: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let table = lookup_table(session.deck(), &slide_id, shape_index)?;
    let width = average_column_width(table);
    let command = Box::new(slides_core::InsertColumn::new(
        slide_id,
        shape_index,
        index,
        width,
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Removes a row from a table at `index` and returns the updated deck snapshot.
#[tauri::command]
pub fn delete_row(
    slide_id: String,
    shape_index: usize,
    index: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::DeleteRow::new(slide_id, shape_index, index));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Removes a column from a table at `index` and returns the updated deck
/// snapshot.
#[tauri::command]
pub fn delete_column(
    slide_id: String,
    shape_index: usize,
    index: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::DeleteColumn::new(slide_id, shape_index, index));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Appends a chart shape with default sample data to a slide and returns the
/// updated deck snapshot.
#[tauri::command]
pub fn add_chart(
    slide_id: String,
    chart_type: ChartTypeDto,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let chart_type = chart_type_from_dto(chart_type);
    let data = sample_chart_data(chart_type);
    let chart = slides_core::ChartShape::new(
        slides_core::Transform {
            frame: centered_chart_frame(),
            rotation: 0.0,
        },
        chart_type,
        data,
        None,
    )
    .map_err(|e| e.to_string())?;
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::AddChart::new(slide_id, chart));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Sets the chart type of a chart shape and returns the updated deck snapshot.
///
/// Switching between category-based types and scatter is rejected by the model
/// when the existing data does not match the new type; the error is returned
/// to the frontend.
#[tauri::command]
pub fn set_chart_type(
    slide_id: String,
    shape_index: usize,
    chart_type: ChartTypeDto,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let chart_type = chart_type_from_dto(chart_type);
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::SetChartType::new(
        slide_id,
        shape_index,
        chart_type,
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Replaces the full data set of a chart shape and returns the updated deck
/// snapshot.
///
/// If the provided data kind does not match the chart's current type (for
/// example, sending XY data to a category chart), the command also updates the
/// type to a sensible default so the change can be applied atomically.
#[tauri::command]
pub fn set_chart_data(
    slide_id: String,
    shape_index: usize,
    data: ChartDataDto,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let data = chart_data_from_dto(data);
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;

    let needs_type_update = session
        .deck()
        .slide(&slide_id)
        .and_then(|slide| slide.shapes.get(shape_index))
        .and_then(|shape| match shape {
            slides_core::Shape::Chart(chart) => {
                let data_kind_matches = match &data {
                    slides_core::ChartData::Category { .. } => chart.chart_type.is_category(),
                    slides_core::ChartData::XY { .. } => chart.chart_type.is_xy(),
                };
                Some(!data_kind_matches)
            }
            _ => None,
        })
        .unwrap_or(false);

    let command: Box<dyn slides_core::Command> = if needs_type_update {
        let chart_type = match &data {
            slides_core::ChartData::Category { .. } => slides_core::ChartType::Column,
            slides_core::ChartData::XY { .. } => slides_core::ChartType::Scatter,
        };
        Box::new(SetChartDataAndType {
            slide_id,
            shape_index,
            data,
            chart_type,
        })
    } else {
        Box::new(slides_core::SetChartData::new(slide_id, shape_index, data))
    };

    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Sets or clears the title of a chart shape and returns the updated deck
/// snapshot. Pass an empty string to clear the title.
#[tauri::command]
pub fn set_chart_title(
    slide_id: String,
    shape_index: usize,
    title: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let title = if title.is_empty() { None } else { Some(title) };
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::SetChartTitle::new(
        slide_id,
        shape_index,
        title,
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Sets or clears the transition for a slide and returns the updated deck snapshot.
///
/// Pass `kind: None` (or omit it) to clear the transition.
#[tauri::command]
pub fn set_transition(
    slide_id: String,
    kind: Option<TransitionKindDto>,
    duration_ms: u32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let transition =
        kind.map(|k| slides_core::Transition::new(transition_kind_from_dto(k), duration_ms));
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::SetTransition::new(slide_id, transition));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Replaces the full build-in animation sequence for a slide and returns the
/// updated deck snapshot. Pass an empty step list to clear the animation.
#[tauri::command]
pub fn set_slide_animation(
    slide_id: String,
    steps: Vec<BuildStepDto>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let animation = if steps.is_empty() {
        None
    } else {
        Some(slides_core::Animation::new(
            steps.into_iter().map(build_step_from_dto).collect(),
        ))
    };
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::SetSlideAnimation::new(slide_id, animation));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Appends a build step to a slide's animation sequence and returns the updated
/// deck snapshot.
#[tauri::command]
pub fn add_build_step(
    slide_id: String,
    shape_index: usize,
    effect: BuildEffectDto,
    duration_ms: u32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let step = slides_core::BuildStep::new(shape_index, build_effect_from_dto(effect), duration_ms);
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::AddBuildStep::new(slide_id, step));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Removes a build step from a slide's animation sequence by position and
/// returns the updated deck snapshot.
#[tauri::command]
pub fn remove_build_step(
    slide_id: String,
    step_index: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::RemoveBuildStepAt::new(slide_id, step_index));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Reorders a build step in a slide's animation sequence and returns the
/// updated deck snapshot.
#[tauri::command]
pub fn move_build_step(
    slide_id: String,
    from: usize,
    to: usize,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::MoveBuildStep::new(slide_id, from, to));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Renders a single slide to a deterministic SVG string.
///
/// The slide's dimensions come from the deck's `slide_size` (aspect ratio),
/// falling back to the default 16:9. When the deck theme has high-contrast
/// enabled, a high-contrast palette (black background, white text, yellow
/// accents) overrides the theme before rendering.
#[tauri::command]
pub fn render_slide_svg(slide_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_ref().ok_or("no deck is open")?;
    let deck = session.deck();
    let slide = deck.slide(&slide_id).ok_or("slide not found")?;
    let opts = render_options(deck);
    let theme = high_contrast_theme(deck);
    let rendered = slides_render::render_slide(slide, &theme, &deck.media, &opts);
    Ok(rendered.svg)
}

/// Builds render dimensions for a deck: the deck's `slide_size` when set, else
/// the default 16:9.
fn render_options(deck: &slides_core::Deck) -> slides_render::RenderOptions {
    if let Some(size) = &deck.slide_size {
        slides_render::RenderOptions {
            width_emu: size.width_emu,
            height_emu: size.height_emu,
        }
    } else {
        slides_render::RenderOptions::default()
    }
}

/// Returns the theme to render with. When high-contrast is enabled, returns an
/// overridden palette (black background, yellow accent) without mutating the
/// deck; otherwise returns the deck's theme by reference.
fn high_contrast_theme(deck: &slides_core::Deck) -> std::borrow::Cow<'_, slides_core::Theme> {
    if deck.theme.high_contrast {
        let mut theme = deck.theme.clone();
        theme.background = slides_core::Color::black();
        theme.accent_color = slides_core::Color::rgb(255, 215, 0);
        std::borrow::Cow::Owned(theme)
    } else {
        std::borrow::Cow::Borrowed(&deck.theme)
    }
}

/// Sets or clears the deck's slide size (aspect ratio) and returns the updated
/// deck snapshot. Pass `None` to revert to the default 16:9.
#[tauri::command]
pub fn set_slide_size(
    slide_size: Option<SlideSizeDto>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let core_size = slide_size.map(slide_size_from_dto);
    let command = Box::new(slides_core::SetSlideSize::new(core_size));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Replaces the deck's entire slide-section list and returns the updated deck
/// snapshot.
#[tauri::command]
pub fn set_sections(
    sections: Vec<SlideSectionDto>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let core_sections = sections.iter().map(section_from_dto).collect();
    let command = Box::new(slides_core::SetSections::new(core_sections));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Sets or clears the rich-text speaker notes for a slide and returns the
/// updated deck snapshot. Pass `None` to clear rich notes (falling back to the
/// plain `notes` field).
#[tauri::command]
pub fn set_rich_notes(
    slide_id: String,
    rich_notes: Option<Vec<ParagraphDto>>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let core_notes = rich_notes
        .map(|paragraphs| paragraphs.iter().map(paragraph_from_dto).collect());
    let command = Box::new(slides_core::SetRichNotes::new(slide_id, core_notes));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Sets the deck theme's high-contrast accessibility mode and returns the
/// updated deck snapshot.
#[tauri::command]
pub fn set_high_contrast(
    high_contrast: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::SetHighContrast::new(high_contrast));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Sets the deck's presenter settings (laser pointer and highlighter colors and
/// defaults) and returns the updated deck snapshot. The change is applied via
/// the verified [`slides_core::SetPresenterSettings`] command so it is undoable.
#[tauri::command]
pub fn set_presenter_settings(
    settings: PresenterSettingsDto,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    let command = Box::new(slides_core::SetPresenterSettings::new(
        presenter_settings_from_dto(&settings),
    ));
    session.execute(command).map_err(|e| e.to_string())?;
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Undoes the most recent command and returns the updated deck snapshot.
#[tauri::command]
pub fn undo(app: AppHandle, state: State<'_, AppState>) -> Result<DeckSnapshot, String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_mut().ok_or("no deck is open")?;
    if !session.undo() {
        return Err("nothing to undo".to_string());
    }
    let snapshot = state.snapshot(session.deck());
    drop(guard);
    schedule_recovery(&app, &state);
    Ok(snapshot)
}

/// Returns the current loss ledger warnings, if any.
#[tauri::command]
pub fn get_loss_ledger(state: State<'_, AppState>) -> Option<Vec<WarningDto>> {
    let guard = state.session.lock().ok()?;
    guard.as_ref().map(|session| {
        session
            .loss_ledger()
            .warnings()
            .iter()
            .map(warning_to_dto)
            .collect()
    })
}

/// Opens the dual-display presenter: a presenter control window and a
/// separate fullscreen audience window, both driven from the current deck.
///
/// If the presenter windows are already open they are focused instead of being
/// recreated (window labels must be unique). Navigation is owned by the
/// presenter window; the audience window mirrors it over Tauri events.
#[tauri::command]
pub fn start_presenter(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    {
        let guard = state.session.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            return Err("no deck is open".to_string());
        }
    }
    *state.presenter_index.lock().map_err(|e| e.to_string())? = 0;

    // If the presenter is already open, focus the existing windows instead of
    // erroring on a duplicate window label.
    if let Some(existing) = app.get_webview_window("presenter") {
        let _ = existing.set_focus();
        if let Some(audience) = app.get_webview_window("audience") {
            let _ = audience.set_focus();
        }
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
        &app,
        "presenter",
        tauri::WebviewUrl::App("index.html#/presenter".into()),
    )
    .title("Presenter")
    .inner_size(1100.0, 700.0)
    .build()
    .map_err(|e| e.to_string())?;

    tauri::WebviewWindowBuilder::new(
        &app,
        "audience",
        tauri::WebviewUrl::App("index.html#/audience".into()),
    )
    .title("900Slides — Audience")
    .fullscreen(true)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Returns the current presenter view state.
#[tauri::command]
pub fn get_presenter_state(state: State<'_, AppState>) -> Result<PresenterState, String> {
    presenter_state_at(&state)
}

/// Advances the presenter to the next slide.
#[tauri::command]
pub fn presenter_next(state: State<'_, AppState>) -> Result<PresenterState, String> {
    let len = {
        let guard = state.session.lock().map_err(|e| e.to_string())?;
        guard.as_ref().map(|s| s.deck().slides.len()).unwrap_or(0)
    };
    let mut idx = state.presenter_index.lock().map_err(|e| e.to_string())?;
    if *idx + 1 < len {
        *idx += 1;
    }
    drop(idx);
    presenter_state_at(&state)
}

/// Goes back one slide in presenter mode.
#[tauri::command]
pub fn presenter_previous(state: State<'_, AppState>) -> Result<PresenterState, String> {
    let mut idx = state.presenter_index.lock().map_err(|e| e.to_string())?;
    if *idx > 0 {
        *idx -= 1;
    }
    drop(idx);
    presenter_state_at(&state)
}

/// Lists available recovery snapshots, newest first.
#[tauri::command]
pub fn list_recovery_snapshots(
    state: State<'_, AppState>,
) -> Result<Vec<RecoverySnapshot>, String> {
    let tracker = state.recovery.lock().map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&tracker.dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pptx") {
            continue;
        }
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(pos) = name.rfind('_') {
            let deck_id = name[..pos].to_string();
            let timestamp = name[pos + 1..].to_string();
            entries.push(RecoverySnapshot {
                id: format!("{name}.pptx"),
                timestamp,
                deck_id,
            });
        }
    }
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(entries)
}

/// Restores a recovery snapshot as the current deck.
#[tauri::command]
pub fn restore_recovery(id: String, state: State<'_, AppState>) -> Result<DeckSnapshot, String> {
    let path = {
        let tracker = state.recovery.lock().map_err(|e| e.to_string())?;
        sanitize_recovery_id(&tracker.dir, &id)?
    };
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let session = slides_pptx::load(&bytes).map_err(|e| e.to_string())?;
    let mut snapshot = state.snapshot(session.deck());
    snapshot.warnings = session
        .loss_ledger()
        .warnings()
        .iter()
        .map(warning_to_dto)
        .collect();
    *state.session.lock().map_err(|e| e.to_string())? = Some(session);
    *state.presenter_index.lock().map_err(|e| e.to_string())? = 0;
    Ok(snapshot)
}

/// Discards a recovery snapshot file.
#[tauri::command]
pub fn discard_recovery(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = {
        let tracker = state.recovery.lock().map_err(|e| e.to_string())?;
        sanitize_recovery_id(&tracker.dir, &id)?
    };
    fs::remove_file(&path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Checks `text` for en-US misspellings, returning each flagged word with its
/// byte span. Runs against the bundled dictionary plus the session's learned
/// user words. Runs on demand from the frontend (debounced by the caller).
#[tauri::command]
pub fn spell_check(
    text: String,
    state: State<'_, AppState>,
) -> Result<Vec<MisspellingDto>, String> {
    let checker = state.spell.lock().map_err(|e| e.to_string())?;
    Ok(checker
        .check(&text)
        .into_iter()
        .map(|m| MisspellingDto {
            word: m.word,
            byte_start: m.byte_start,
            byte_end: m.byte_end,
        })
        .collect())
}

/// Returns up to `max` correction suggestions for `word`, ranked by edit
/// distance then alphabetical. An empty list is returned for known words.
#[tauri::command]
pub fn spell_suggest(
    word: String,
    max: usize,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let checker = state.spell.lock().map_err(|e| e.to_string())?;
    Ok(checker.suggest(&word, max))
}

/// Learns `word` into the in-memory user dictionary and appends it to the
/// persisted user dictionary file so it survives restarts. A no-op for empty
/// input or words already present in the file.
#[tauri::command]
pub fn spell_add_word(word: String, state: State<'_, AppState>) -> Result<(), String> {
    let trimmed = word.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    {
        let mut checker = state.spell.lock().map_err(|e| e.to_string())?;
        checker.add_user_word(trimmed);
    }
    let path = state.user_dictionary_path.clone();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Avoid duplicating an entry that was learned earlier (case-insensitive).
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let already_known = existing
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case(trimmed));
    if !already_known {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        writeln!(file, "{trimmed}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn presenter_state_at(state: &AppState) -> Result<PresenterState, String> {
    let guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_ref().ok_or("no deck is open")?;
    let slides = &session.deck().slides;
    if slides.is_empty() {
        return Err("deck has no slides".to_string());
    }
    let mut idx = *state.presenter_index.lock().map_err(|e| e.to_string())?;
    if idx >= slides.len() {
        idx = 0;
    }
    let current = slide_to_dto(&slides[idx]);
    let next = slides.get(idx + 1).map(slide_to_dto);
    let notes = slides.get(idx).map(|s| s.notes.clone()).unwrap_or_default();
    Ok(PresenterState {
        current_slide: current,
        next_slide: next,
        slide_number: idx + 1,
        total: slides.len(),
        notes,
        media: media_to_dto(&session.deck().media),
        slide_size: session.deck().slide_size.as_ref().map(slide_size_to_dto),
        high_contrast: session.deck().theme.high_contrast,
        presenter_settings: presenter_settings_to_dto(&session.deck().presenter_settings),
    })
}

fn schedule_recovery(app: &AppHandle, state: &State<'_, AppState>) {
    let Some(mut guard) = state.session.lock().ok() else {
        return;
    };
    let Some(session) = guard.as_mut() else {
        return;
    };
    let Some(bytes) = slides_pptx::save(session).ok() else {
        return;
    };
    let deck_id = session.deck().id.clone();
    drop(guard);

    let token = state.recovery_token.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut tracker) = state.recovery.lock() {
        tracker.pending_token = token;
        tracker.pending_bytes = Some(bytes);
        tracker.pending_deck_id = Some(deck_id);
    }

    let app_handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(750));
        let Some(state) = app_handle.try_state::<AppState>() else {
            return;
        };
        let (bytes, deck_id, dir) = {
            let Ok(tracker) = state.recovery.lock() else {
                return;
            };
            if tracker.pending_token != token {
                return;
            }
            (
                tracker.pending_bytes.clone(),
                tracker.pending_deck_id.clone(),
                tracker.dir.clone(),
            )
        };
        if let (Some(bytes), Some(deck_id)) = (bytes, deck_id) {
            let _ = write_recovery_snapshot(&dir, &deck_id, &bytes);
        }
    });
}

fn sanitize_recovery_id(dir: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() {
        return Err("recovery id is empty".to_string());
    }
    if id.contains('/') || id.contains('\\') || id.starts_with('.') || id.contains('\0') {
        return Err("recovery id contains invalid characters".to_string());
    }

    let canonical_dir = dir.canonicalize().map_err(|e| e.to_string())?;
    let joined = dir.join(id);
    let canonical_joined = joined.canonicalize().unwrap_or_else(|_| joined.clone());

    if !canonical_joined.starts_with(&canonical_dir) {
        return Err("recovery id escapes recovery directory".to_string());
    }
    Ok(canonical_joined)
}

fn write_recovery_snapshot(dir: &Path, deck_id: &str, bytes: &[u8]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as u64;
    let path = dir.join(format!("{deck_id}_{timestamp}.pptx"));
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let other = entry.path();
        if other == path {
            continue;
        }
        if let Some(stem) = other.file_stem().and_then(|s| s.to_str()) {
            if let Some(pos) = stem.rfind('_') {
                if &stem[..pos] == deck_id {
                    fs::remove_file(&other).ok();
                }
            }
        }
    }
    Ok(())
}

fn retire_recovery(state: &State<'_, AppState>, deck_id: &str) {
    if let Ok(tracker) = state.recovery.lock() {
        if let Ok(entries) = fs::read_dir(&tracker.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(pos) = stem.rfind('_') {
                        if &stem[..pos] == deck_id {
                            fs::remove_file(&path).ok();
                        }
                    }
                }
            }
        }
    }
}

fn theme_to_dto(theme: &slides_core::Theme) -> ThemeSnapshot {
    ThemeSnapshot {
        background: color_to_dto(theme.background),
        heading_font: theme.heading_font.clone(),
        body_font: theme.body_font.clone(),
        accent_color: color_to_dto(theme.accent_color),
        high_contrast: theme.high_contrast,
    }
}

fn slide_size_to_dto(size: &slides_core::SlideSize) -> SlideSizeDto {
    SlideSizeDto {
        width_emu: size.width_emu,
        height_emu: size.height_emu,
    }
}

fn slide_size_from_dto(dto: SlideSizeDto) -> slides_core::SlideSize {
    slides_core::SlideSize {
        width_emu: dto.width_emu,
        height_emu: dto.height_emu,
    }
}

fn section_to_dto(section: &slides_core::SlideSection) -> SlideSectionDto {
    SlideSectionDto {
        name: section.name.clone(),
        start_slide_id: section.start_slide_id.clone(),
    }
}

fn section_from_dto(dto: &SlideSectionDto) -> slides_core::SlideSection {
    slides_core::SlideSection {
        name: dto.name.clone(),
        start_slide_id: dto.start_slide_id.clone(),
    }
}

fn color_to_dto(color: slides_core::Color) -> ColorDto {
    ColorDto {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

fn slide_to_dto(slide: &slides_core::Slide) -> SlideSnapshot {
    SlideSnapshot {
        id: slide.id.clone(),
        notes: slide.notes.clone(),
        shapes: slide.shapes.iter().map(shape_to_dto).collect(),
        transition: slide.transition.as_ref().map(transition_to_dto),
        animation: slide.animation.as_ref().map(animation_to_dto),
        rich_notes: slide
            .rich_notes
            .as_ref()
            .map(|paragraphs| paragraphs.iter().map(paragraph_to_dto).collect()),
    }
}

fn shape_to_dto(shape: &slides_core::Shape) -> ShapeSnapshot {
    match shape {
        slides_core::Shape::TextBox(tb) => ShapeSnapshot::TextBox(TextBoxSnapshot {
            frame: rect_to_dto(tb.frame),
            paragraphs: tb.paragraphs.iter().map(paragraph_to_dto).collect(),
        }),
        slides_core::Shape::Passthrough(obj) => ShapeSnapshot::Passthrough(PassthroughSnapshot {
            id: obj.id.clone(),
            label: obj.label.clone(),
            source_part: obj.source_part.clone(),
            frame: obj.frame.map(rect_to_dto),
        }),
        slides_core::Shape::Image(image) => ShapeSnapshot::Image(ImageShapeSnapshot {
            transform: transform_to_dto(image.transform),
            media_ref: image.media_ref.clone(),
            crop: image.crop.as_ref().map(crop_to_dto),
        }),
        slides_core::Shape::Geometric(geometric) => {
            ShapeSnapshot::Geometric(GeometricShapeSnapshot {
                transform: transform_to_dto(geometric.transform),
                geometry: geometry_to_dto(geometric.geometry),
                style: style_to_dto(&geometric.style),
            })
        }
        slides_core::Shape::Table(table) => ShapeSnapshot::Table(table_to_dto(table)),
        slides_core::Shape::Chart(chart) => ShapeSnapshot::Chart(chart_to_dto(chart)),
    }
}

fn rect_to_dto(rect: slides_core::Rect) -> RectDto {
    RectDto {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn rect_to_core(rect: RectDto) -> slides_core::Rect {
    slides_core::Rect::new(rect.x, rect.y, rect.width, rect.height)
}

fn transform_to_dto(transform: slides_core::Transform) -> TransformDto {
    TransformDto {
        frame: rect_to_dto(transform.frame),
        rotation: transform.rotation,
    }
}

fn transform_to_core(transform: TransformDto) -> slides_core::Transform {
    slides_core::Transform {
        frame: rect_to_core(transform.frame),
        rotation: transform.rotation,
    }
}

fn color_to_core(color: ColorDto) -> slides_core::Color {
    slides_core::Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

fn geometry_to_dto(geometry: slides_core::Geometry) -> GeometryDto {
    match geometry {
        slides_core::Geometry::Rectangle => GeometryDto::Rectangle,
        slides_core::Geometry::RoundedRectangle { radius } => {
            GeometryDto::RoundedRectangle { radius }
        }
        slides_core::Geometry::Ellipse => GeometryDto::Ellipse,
        slides_core::Geometry::Triangle => GeometryDto::Triangle,
        slides_core::Geometry::Line => GeometryDto::Line,
        slides_core::Geometry::Arrow => GeometryDto::Arrow,
        slides_core::Geometry::RightArrowCallout => GeometryDto::RightArrowCallout,
        slides_core::Geometry::Star5 => GeometryDto::Star5,
    }
}

/// Maps a frontend geometry kind string to the model geometry.
fn geometry_from_kind(kind: &str) -> slides_core::Geometry {
    match kind {
        "rounded_rectangle" => slides_core::Geometry::RoundedRectangle { radius: 100_000.0 },
        "ellipse" => slides_core::Geometry::Ellipse,
        "triangle" => slides_core::Geometry::Triangle,
        "line" => slides_core::Geometry::Line,
        "arrow" => slides_core::Geometry::Arrow,
        "right_arrow_callout" => slides_core::Geometry::RightArrowCallout,
        "star5" => slides_core::Geometry::Star5,
        _ => slides_core::Geometry::Rectangle,
    }
}

fn fill_to_dto(fill: &slides_core::Fill) -> FillDto {
    match fill {
        slides_core::Fill::Solid(color) => FillDto::Solid(color_to_dto(*color)),
    }
}

fn fill_to_core(fill: FillDto) -> slides_core::Fill {
    match fill {
        FillDto::Solid(color) => slides_core::Fill::Solid(color_to_core(color)),
    }
}

fn outline_to_dto(outline: &slides_core::Outline) -> OutlineDto {
    OutlineDto {
        color: color_to_dto(outline.color),
        width_emu: outline.width_emu,
        dash: dash_to_dto(&outline.dash),
    }
}

fn outline_to_core(outline: OutlineDto) -> slides_core::Outline {
    slides_core::Outline {
        color: color_to_core(outline.color),
        width_emu: outline.width_emu,
        dash: dash_to_core(outline.dash),
    }
}

fn dash_to_dto(dash: &slides_core::DashStyle) -> DashStyleDto {
    match dash {
        slides_core::DashStyle::Solid => DashStyleDto::Solid,
        slides_core::DashStyle::Dash => DashStyleDto::Dash,
        slides_core::DashStyle::Dot => DashStyleDto::Dot,
        slides_core::DashStyle::DashDot => DashStyleDto::DashDot,
    }
}

fn dash_to_core(dash: DashStyleDto) -> slides_core::DashStyle {
    match dash {
        DashStyleDto::Solid => slides_core::DashStyle::Solid,
        DashStyleDto::Dash => slides_core::DashStyle::Dash,
        DashStyleDto::Dot => slides_core::DashStyle::Dot,
        DashStyleDto::DashDot => slides_core::DashStyle::DashDot,
    }
}

fn shadow_to_dto(shadow: &slides_core::Shadow) -> ShadowDto {
    ShadowDto {
        offset_x: shadow.offset_x,
        offset_y: shadow.offset_y,
        blur: shadow.blur,
        color: color_to_dto(shadow.color),
        opacity: shadow.opacity,
    }
}

fn shadow_to_core(shadow: ShadowDto) -> slides_core::Shadow {
    slides_core::Shadow {
        offset_x: shadow.offset_x,
        offset_y: shadow.offset_y,
        blur: shadow.blur,
        color: color_to_core(shadow.color),
        opacity: shadow.opacity,
    }
}

fn style_to_dto(style: &slides_core::Style) -> StyleDto {
    StyleDto {
        fill: style.fill.as_ref().map(fill_to_dto),
        outline: style.outline.as_ref().map(outline_to_dto),
        shadow: style.shadow.as_ref().map(shadow_to_dto),
    }
}

fn style_to_core(style: StyleDto) -> slides_core::Style {
    slides_core::Style {
        fill: style.fill.map(fill_to_core),
        outline: style.outline.map(outline_to_core),
        shadow: style.shadow.map(shadow_to_core),
    }
}

fn crop_to_dto(crop: &slides_core::Crop) -> CropDto {
    CropDto {
        left: crop.left,
        top: crop.top,
        right: crop.right,
        bottom: crop.bottom,
    }
}

/// Default style for newly added shapes: a solid accent fill with a thin dark
/// outline, so the shape is visible without an explicit style from the caller.
fn default_shape_style(theme: &slides_core::Theme) -> slides_core::Style {
    slides_core::Style {
        fill: Some(slides_core::Fill::Solid(theme.accent_color)),
        outline: Some(slides_core::Outline {
            color: slides_core::Color::black(),
            width_emu: 9_525.0,
            dash: slides_core::DashStyle::Solid,
        }),
        shadow: None,
    }
}

/// Builds a transform centered on the slide, scaling `native_w` x `native_h`
/// pixels to fit within 60% of the slide. A zero dimension falls back to a
/// default 4:3 box.
fn centered_transform(native_w: u32, native_h: u32) -> slides_core::Transform {
    let max_w = SLIDE_WIDTH_EMU * 0.6;
    let max_h = SLIDE_HEIGHT_EMU * 0.6;
    let (width, height) = if native_w == 0 || native_h == 0 {
        (max_w, max_w * 0.75)
    } else {
        let nw = native_w as f64;
        let nh = native_h as f64;
        let scale = (max_w / nw).min(max_h / nh);
        (nw * scale, nh * scale)
    };
    let x = (SLIDE_WIDTH_EMU - width) / 2.0;
    let y = (SLIDE_HEIGHT_EMU - height) / 2.0;
    slides_core::Transform {
        frame: slides_core::Rect::new(x, y, width, height),
        rotation: 0.0,
    }
}

/// Returns a centered frame for a new table, sized to ~70% of the slide width
/// and ~60% of the slide height.
fn centered_table_frame() -> slides_core::Rect {
    let width = SLIDE_WIDTH_EMU * 0.7;
    let height = SLIDE_HEIGHT_EMU * 0.6;
    let x = (SLIDE_WIDTH_EMU - width) / 2.0;
    let y = (SLIDE_HEIGHT_EMU - height) / 2.0;
    slides_core::Rect::new(x, y, width, height)
}

/// Returns a centered frame for a new chart, sized to ~60% of the slide width
/// and ~40% of the slide height.
fn centered_chart_frame() -> slides_core::Rect {
    let width = SLIDE_WIDTH_EMU * 0.6;
    let height = SLIDE_HEIGHT_EMU * 0.4;
    let x = (SLIDE_WIDTH_EMU - width) / 2.0;
    let y = (SLIDE_HEIGHT_EMU - height) / 2.0;
    slides_core::Rect::new(x, y, width, height)
}

/// Builds default sample data for a newly inserted chart.
fn sample_chart_data(chart_type: slides_core::ChartType) -> slides_core::ChartData {
    if chart_type.is_category() {
        slides_core::ChartData::Category {
            categories: vec!["Q1".to_string(), "Q2".to_string(), "Q3".to_string()],
            series: vec![
                slides_core::CategorySeries {
                    name: "2023".to_string(),
                    values: vec![10.0, 20.0, 30.0],
                },
                slides_core::CategorySeries {
                    name: "2024".to_string(),
                    values: vec![15.0, 25.0, 35.0],
                },
            ],
        }
    } else {
        slides_core::ChartData::XY {
            series: vec![slides_core::XYSeries {
                name: "Run A".to_string(),
                points: vec![
                    slides_core::XYPoint::new(0.0, 1.0),
                    slides_core::XYPoint::new(1.0, 2.0),
                    slides_core::XYPoint::new(2.0, 3.0),
                ],
            }],
        }
    }
}

/// Finds a table shape on a slide, returning `Err` if the slide or shape is
/// missing or is not a table.
fn lookup_table<'a>(
    deck: &'a slides_core::Deck,
    slide_id: &str,
    shape_index: usize,
) -> Result<&'a slides_core::TableShape, String> {
    let slide = deck.slide(slide_id).ok_or("slide not found")?;
    let shape = slide.shapes.get(shape_index).ok_or("shape not found")?;
    match shape {
        slides_core::Shape::Table(table) => Ok(table),
        _ => Err("shape is not a table".to_string()),
    }
}

/// Returns the column count and a representative row height (from the last
/// row, or a quarter of the slide height when the table is empty) for building
/// a new row that matches the table's grid.
fn table_grid_metrics(
    deck: &slides_core::Deck,
    slide_id: &str,
    shape_index: usize,
) -> Result<(usize, f64), String> {
    let table = lookup_table(deck, slide_id, shape_index)?;
    let col_count = table.col_count();
    let row_height = table
        .rows
        .last()
        .map(|row| row.height)
        .unwrap_or(SLIDE_HEIGHT_EMU / 4.0);
    Ok((col_count, row_height))
}

/// Returns the average column width of a table, used as the width of a newly
/// inserted column.
fn average_column_width(table: &slides_core::TableShape) -> f64 {
    let count = table.col_count();
    if count == 0 {
        return SLIDE_WIDTH_EMU / 4.0;
    }
    table.column_widths.iter().sum::<f64>() / count as f64
}

fn media_entry_to_dto(entry: &slides_core::MediaEntry) -> MediaEntryDto {
    let bytes = base64::engine::general_purpose::STANDARD.encode(&entry.bytes);
    MediaEntryDto {
        mime: entry.mime.clone(),
        bytes,
        width: entry.width,
        height: entry.height,
    }
}

fn media_to_dto(media: &slides_core::MediaStore) -> BTreeMap<String, MediaEntryDto> {
    media
        .iter()
        .map(|(key, entry)| (key.clone(), media_entry_to_dto(entry)))
        .collect()
}

fn cell_align_to_dto(align: slides_core::CellAlign) -> CellAlignDto {
    match align {
        slides_core::CellAlign::Left => CellAlignDto::Left,
        slides_core::CellAlign::Center => CellAlignDto::Center,
        slides_core::CellAlign::Right => CellAlignDto::Right,
    }
}

fn cell_align_to_core(align: CellAlignDto) -> slides_core::CellAlign {
    match align {
        CellAlignDto::Left => slides_core::CellAlign::Left,
        CellAlignDto::Center => slides_core::CellAlign::Center,
        CellAlignDto::Right => slides_core::CellAlign::Right,
    }
}

fn border_edge_to_dto(edge: &slides_core::BorderEdge) -> BorderEdgeDto {
    BorderEdgeDto {
        color: color_to_dto(edge.color),
        width_emu: edge.width_emu,
        dash: dash_to_dto(&edge.dash),
    }
}

fn border_edge_to_core(edge: BorderEdgeDto) -> slides_core::BorderEdge {
    slides_core::BorderEdge {
        color: color_to_core(edge.color),
        width_emu: edge.width_emu,
        dash: dash_to_core(edge.dash),
    }
}

fn table_borders_to_dto(borders: &slides_core::TableBorders) -> TableBordersDto {
    TableBordersDto {
        top: borders.top.as_ref().map(border_edge_to_dto),
        bottom: borders.bottom.as_ref().map(border_edge_to_dto),
        left: borders.left.as_ref().map(border_edge_to_dto),
        right: borders.right.as_ref().map(border_edge_to_dto),
    }
}

fn table_borders_to_core(borders: TableBordersDto) -> slides_core::TableBorders {
    slides_core::TableBorders {
        top: borders.top.map(border_edge_to_core),
        bottom: borders.bottom.map(border_edge_to_core),
        left: borders.left.map(border_edge_to_core),
        right: borders.right.map(border_edge_to_core),
    }
}

fn table_cell_to_dto(cell: &slides_core::TableCell) -> TableCellDto {
    TableCellDto {
        text: cell.text.clone(),
        fill: cell.fill.as_ref().map(fill_to_dto),
        borders: cell.borders.as_ref().map(table_borders_to_dto),
        align: cell_align_to_dto(cell.align),
    }
}

fn table_row_to_dto(row: &slides_core::TableRow) -> TableRowDto {
    TableRowDto {
        height: row.height,
        cells: row.cells.iter().map(table_cell_to_dto).collect(),
    }
}

fn table_to_dto(table: &slides_core::TableShape) -> TableShapeSnapshot {
    TableShapeSnapshot {
        transform: transform_to_dto(table.transform),
        rows: table.rows.iter().map(table_row_to_dto).collect(),
        column_widths: table.column_widths.clone(),
        default_borders: table_borders_to_dto(&table.default_borders),
        header_row: table.header_row,
    }
}

fn chart_to_dto(chart: &slides_core::ChartShape) -> ChartShapeSnapshot {
    ChartShapeSnapshot {
        transform: transform_to_dto(chart.transform),
        chart_type: chart_type_to_dto(chart.chart_type),
        data: chart_data_to_dto(&chart.data),
        title: chart.title.clone(),
    }
}

fn chart_type_to_dto(chart_type: slides_core::ChartType) -> ChartTypeDto {
    match chart_type {
        slides_core::ChartType::Bar => ChartTypeDto::Bar,
        slides_core::ChartType::Column => ChartTypeDto::Column,
        slides_core::ChartType::Line => ChartTypeDto::Line,
        slides_core::ChartType::Area => ChartTypeDto::Area,
        slides_core::ChartType::Pie => ChartTypeDto::Pie,
        slides_core::ChartType::Scatter => ChartTypeDto::Scatter,
    }
}

fn chart_data_to_dto(data: &slides_core::ChartData) -> ChartDataDto {
    match data {
        slides_core::ChartData::Category { categories, series } => ChartDataDto::Category {
            categories: categories.clone(),
            series: series
                .iter()
                .map(|s| CategorySeriesDto {
                    name: s.name.clone(),
                    values: s.values.clone(),
                })
                .collect(),
        },
        slides_core::ChartData::XY { series } => ChartDataDto::XY {
            series: series
                .iter()
                .map(|s| XYSeriesDto {
                    name: s.name.clone(),
                    points: s
                        .points
                        .iter()
                        .map(|p| XYPointDto { x: p.x, y: p.y })
                        .collect(),
                })
                .collect(),
        },
    }
}

fn chart_type_from_dto(dto: ChartTypeDto) -> slides_core::ChartType {
    match dto {
        ChartTypeDto::Bar => slides_core::ChartType::Bar,
        ChartTypeDto::Column => slides_core::ChartType::Column,
        ChartTypeDto::Line => slides_core::ChartType::Line,
        ChartTypeDto::Area => slides_core::ChartType::Area,
        ChartTypeDto::Pie => slides_core::ChartType::Pie,
        ChartTypeDto::Scatter => slides_core::ChartType::Scatter,
    }
}

fn chart_data_from_dto(dto: ChartDataDto) -> slides_core::ChartData {
    match dto {
        ChartDataDto::Category { categories, series } => slides_core::ChartData::Category {
            categories,
            series: series
                .into_iter()
                .map(|s| slides_core::CategorySeries {
                    name: s.name,
                    values: s.values,
                })
                .collect(),
        },
        ChartDataDto::XY { series } => slides_core::ChartData::XY {
            series: series
                .into_iter()
                .map(|s| slides_core::XYSeries {
                    name: s.name,
                    points: s
                        .points
                        .into_iter()
                        .map(|p| slides_core::XYPoint::new(p.x, p.y))
                        .collect(),
                })
                .collect(),
        },
    }
}

fn transition_to_dto(transition: &slides_core::Transition) -> TransitionDto {
    TransitionDto {
        kind: transition_kind_to_dto(transition.kind),
        duration_ms: transition.duration_ms,
    }
}

fn transition_kind_to_dto(kind: slides_core::TransitionKind) -> TransitionKindDto {
    match kind {
        slides_core::TransitionKind::None => TransitionKindDto::None,
        slides_core::TransitionKind::Fade => TransitionKindDto::Fade,
        slides_core::TransitionKind::Slide => TransitionKindDto::Slide,
        slides_core::TransitionKind::Push => TransitionKindDto::Push,
        slides_core::TransitionKind::Wipe => TransitionKindDto::Wipe,
    }
}

fn transition_kind_from_dto(dto: TransitionKindDto) -> slides_core::TransitionKind {
    match dto {
        TransitionKindDto::None => slides_core::TransitionKind::None,
        TransitionKindDto::Fade => slides_core::TransitionKind::Fade,
        TransitionKindDto::Slide => slides_core::TransitionKind::Slide,
        TransitionKindDto::Push => slides_core::TransitionKind::Push,
        TransitionKindDto::Wipe => slides_core::TransitionKind::Wipe,
    }
}

fn animation_to_dto(animation: &slides_core::Animation) -> AnimationDto {
    AnimationDto {
        steps: animation.steps.iter().map(build_step_to_dto).collect(),
    }
}

fn build_step_to_dto(step: &slides_core::BuildStep) -> BuildStepDto {
    BuildStepDto {
        shape_index: step.shape_index,
        effect: build_effect_to_dto(step.effect),
        duration_ms: step.duration_ms,
    }
}

fn build_effect_to_dto(effect: slides_core::BuildEffect) -> BuildEffectDto {
    match effect {
        slides_core::BuildEffect::Fade => BuildEffectDto::Fade,
        slides_core::BuildEffect::SlideInLeft => BuildEffectDto::SlideInLeft,
        slides_core::BuildEffect::SlideInRight => BuildEffectDto::SlideInRight,
        slides_core::BuildEffect::SlideInTop => BuildEffectDto::SlideInTop,
        slides_core::BuildEffect::SlideInBottom => BuildEffectDto::SlideInBottom,
        slides_core::BuildEffect::Appear => BuildEffectDto::Appear,
        slides_core::BuildEffect::Disappear => BuildEffectDto::Disappear,
    }
}

fn build_step_from_dto(dto: BuildStepDto) -> slides_core::BuildStep {
    slides_core::BuildStep::new(
        dto.shape_index,
        build_effect_from_dto(dto.effect),
        dto.duration_ms,
    )
}

fn build_effect_from_dto(dto: BuildEffectDto) -> slides_core::BuildEffect {
    match dto {
        BuildEffectDto::Fade => slides_core::BuildEffect::Fade,
        BuildEffectDto::SlideInLeft => slides_core::BuildEffect::SlideInLeft,
        BuildEffectDto::SlideInRight => slides_core::BuildEffect::SlideInRight,
        BuildEffectDto::SlideInTop => slides_core::BuildEffect::SlideInTop,
        BuildEffectDto::SlideInBottom => slides_core::BuildEffect::SlideInBottom,
        BuildEffectDto::Appear => slides_core::BuildEffect::Appear,
        BuildEffectDto::Disappear => slides_core::BuildEffect::Disappear,
    }
}

fn paragraph_to_dto(paragraph: &slides_core::Paragraph) -> ParagraphDto {
    ParagraphDto {
        runs: paragraph.runs.iter().map(run_to_dto).collect(),
        list_style: match paragraph.list_style {
            slides_core::ListStyle::None => "none".to_string(),
            slides_core::ListStyle::Ordered => "ordered".to_string(),
            slides_core::ListStyle::Unordered => "unordered".to_string(),
        },
        style: paragraph_style_to_dto(&paragraph.style),
    }
}

fn paragraph_style_to_dto(style: &slides_core::ParagraphStyle) -> ParagraphStyleDto {
    ParagraphStyleDto {
        heading: style.heading.map(|level| match level {
            slides_core::HeadingLevel::H1 => HeadingLevelDto::H1,
            slides_core::HeadingLevel::H2 => HeadingLevelDto::H2,
            slides_core::HeadingLevel::H3 => HeadingLevelDto::H3,
            slides_core::HeadingLevel::H4 => HeadingLevelDto::H4,
            slides_core::HeadingLevel::H5 => HeadingLevelDto::H5,
            slides_core::HeadingLevel::H6 => HeadingLevelDto::H6,
        }),
        blockquote: style.blockquote,
        code_block: style.code_block,
        indent_level: style.indent_level,
    }
}

fn run_to_dto(run: &slides_core::Run) -> RunDto {
    RunDto {
        text: run.text.clone(),
        bold: run.bold,
        italic: run.italic,
        underline: run.underline,
        strikethrough: run.strikethrough,
        vertical_align: match run.vertical_align {
            slides_core::VerticalAlign::Baseline => VerticalAlignDto::Baseline,
            slides_core::VerticalAlign::Superscript => VerticalAlignDto::Superscript,
            slides_core::VerticalAlign::Subscript => VerticalAlignDto::Subscript,
        },
        link: run.link.as_ref().map(|link| LinkDto {
            url: link.url.clone(),
            display: link.display.clone(),
        }),
        code: run.code,
        font_family: run.font_family.clone(),
    }
}

fn run_from_dto(run: &RunDto) -> slides_core::Run {
    slides_core::Run {
        text: run.text.clone(),
        bold: run.bold,
        italic: run.italic,
        underline: run.underline,
        strikethrough: run.strikethrough,
        vertical_align: match run.vertical_align {
            VerticalAlignDto::Baseline => slides_core::VerticalAlign::Baseline,
            VerticalAlignDto::Superscript => slides_core::VerticalAlign::Superscript,
            VerticalAlignDto::Subscript => slides_core::VerticalAlign::Subscript,
        },
        link: run
            .link
            .as_ref()
            .map(|link| slides_core::Link::new_unchecked(link.url.clone())),
        code: run.code,
        font_family: run.font_family.clone(),
    }
}

fn paragraph_from_dto(dto: &ParagraphDto) -> slides_core::Paragraph {
    slides_core::Paragraph {
        runs: dto.runs.iter().map(run_from_dto).collect(),
        list_style: match dto.list_style.as_str() {
            "ordered" => slides_core::ListStyle::Ordered,
            "unordered" => slides_core::ListStyle::Unordered,
            _ => slides_core::ListStyle::None,
        },
        style: paragraph_style_to_core(&dto.style),
    }
}

fn paragraph_style_to_core(style: &ParagraphStyleDto) -> slides_core::ParagraphStyle {
    slides_core::ParagraphStyle {
        heading: style.heading.map(|level| match level {
            HeadingLevelDto::H1 => slides_core::HeadingLevel::H1,
            HeadingLevelDto::H2 => slides_core::HeadingLevel::H2,
            HeadingLevelDto::H3 => slides_core::HeadingLevel::H3,
            HeadingLevelDto::H4 => slides_core::HeadingLevel::H4,
            HeadingLevelDto::H5 => slides_core::HeadingLevel::H5,
            HeadingLevelDto::H6 => slides_core::HeadingLevel::H6,
        }),
        blockquote: style.blockquote,
        code_block: style.code_block,
        indent_level: style.indent_level,
    }
}

fn warning_to_dto(warning: &slides_pptx::LossWarning) -> WarningDto {
    WarningDto {
        slide_id: warning.slide_id.clone(),
        message: warning.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        average_column_width, sanitize_recovery_id, table_grid_metrics, table_to_dto, AppState,
        CellAlignDto,
    };
    use std::fs;

    #[test]
    fn sanitize_recovery_id_rejects_path_traversal() {
        let dir = std::env::temp_dir().join("900slides-test-sanitize");
        fs::create_dir_all(&dir).unwrap();
        let canonical_dir = dir.canonicalize().unwrap();

        assert!(sanitize_recovery_id(&canonical_dir, "../evil").is_err());
        assert!(sanitize_recovery_id(&canonical_dir, "./hidden").is_err());
        assert!(sanitize_recovery_id(&canonical_dir, "a/b").is_err());
        assert!(sanitize_recovery_id(&canonical_dir, "a\\b").is_err());
        assert!(sanitize_recovery_id(&canonical_dir, "").is_err());
        assert!(sanitize_recovery_id(&canonical_dir, "a\0b").is_err());

        let ok = sanitize_recovery_id(&canonical_dir, "deck_123.pptx").unwrap();
        assert_eq!(ok, canonical_dir.join("deck_123.pptx"));
    }

    #[test]
    fn presenter_state_at_errors_on_empty_deck() {
        let state = AppState::new();
        let blank = slides_pptx::create_blank_pptx();
        let mut session = slides_pptx::load(&blank).expect("load blank pptx");
        session.deck_mut().slides.clear();
        *state.session.lock().unwrap() = Some(session);

        assert_eq!(
            super::presenter_state_at(&state),
            Err("deck has no slides".to_string())
        );
    }

    fn sample_table() -> slides_core::TableShape {
        let mut table = slides_core::TableShape::default_grid(
            2,
            2,
            slides_core::Rect::new(0.0, 0.0, 200.0, 100.0),
        );
        table.cell_mut(0, 0).unwrap().text = "Name".to_string();
        table.cell_mut(0, 1).unwrap().text = "Value".to_string();
        table.cell_mut(1, 0).unwrap().text = "x".to_string();
        table.cell_mut(1, 1).unwrap().align = slides_core::CellAlign::Right;
        table.header_row = true;
        table
    }

    #[test]
    fn table_to_dto_round_trips_cells_and_header() {
        let table = sample_table();
        let dto = table_to_dto(&table);
        assert_eq!(dto.rows.len(), 2);
        assert_eq!(dto.column_widths, table.column_widths);
        assert!(dto.header_row);
        assert_eq!(dto.rows[0].cells[0].text, "Name");
        assert_eq!(dto.rows[1].cells[1].align, CellAlignDto::Right);
    }

    #[test]
    fn average_column_width_matches_mean() {
        let mut table = slides_core::TableShape::default_grid(
            1,
            2,
            slides_core::Rect::new(0.0, 0.0, 300.0, 100.0),
        );
        table.column_widths = vec![100.0, 200.0];
        assert_eq!(average_column_width(&table), 150.0);
    }

    #[test]
    fn table_grid_metrics_reads_existing_table() {
        let table = sample_table();
        let mut deck = slides_core::Deck::new();
        let slide = slides_core::Slide {
            id: "slide-1".to_string(),
            shapes: vec![slides_core::Shape::Table(table)],
            ..Default::default()
        };
        let slide_id = slide.id.clone();
        deck.slides.push(slide);

        let (cols, height) = table_grid_metrics(&deck, &slide_id, 0).expect("table metrics");
        assert_eq!(cols, 2);
        assert_eq!(height, 50.0);

        assert!(table_grid_metrics(&deck, &slide_id, 1).is_err());
    }
}
