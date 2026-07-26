//! Tauri commands for the 900Slides desktop application.
//!
//! This module exposes the v0.1.0 command surface: deck creation, opening,
//! saving, text editing, undo, presenter mode, and recovery snapshots. Every
//! mutation is applied transactionally in Rust, and the frontend always
//! re-renders from the returned deck snapshot.

use std::collections::BTreeMap;
use std::fs;
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
        let dir = dirs::data_dir()
            .unwrap_or_default()
            .join("900Slides")
            .join("recovery");
        fs::create_dir_all(&dir).ok();
        Self {
            session: Mutex::new(None),
            recovery: Mutex::new(RecoveryTracker {
                dir,
                pending_token: 0,
                pending_bytes: None,
                pending_deck_id: None,
            }),
            presenter_index: Mutex::new(0),
            recovery_token: AtomicU64::new(0),
            media_cache: Mutex::new(MediaCache::default()),
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
            let mut cache = self
                .media_cache
                .lock()
                .expect("media cache mutex poisoned");
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
            slides: deck.slides.iter().map(slide_to_dto).collect(),
            media,
            warnings: Vec::new(),
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
    /// Ordered slides in the deck.
    pub slides: Vec<SlideSnapshot>,
    /// Media store: image bytes keyed by their media reference, base64-encoded
    /// so the frontend can render images directly from the snapshot.
    #[serde(default)]
    pub media: BTreeMap<String, MediaEntryDto>,
    /// Warnings from the last load (empty for most commands).
    pub warnings: Vec<WarningDto>,
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

/// Paragraph data transfer object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphDto {
    /// Inline text runs.
    pub runs: Vec<RunDto>,
    /// List style of the paragraph.
    pub list_style: String,
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
    *state
        .session
        .lock()
        .map_err(|e| e.to_string())? = Some(session);
    *state
        .presenter_index
        .lock()
        .map_err(|e| e.to_string())? = 0;
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
    *state
        .session
        .lock()
        .map_err(|e| e.to_string())? = Some(session);
    *state
        .presenter_index
        .lock()
        .map_err(|e| e.to_string())? = 0;
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

/// Renders a single slide to a deterministic SVG string.
#[tauri::command]
pub fn render_slide_svg(slide_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let guard = state.session.lock().map_err(|e| e.to_string())?;
    let session = guard.as_ref().ok_or("no deck is open")?;
    let deck = session.deck();
    let slide = deck.slide(&slide_id).ok_or("slide not found")?;
    let rendered = slides_render::render_slide(
        slide,
        &deck.theme,
        &deck.media,
        &slides_render::RenderOptions::default(),
    );
    Ok(rendered.svg)
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

/// Opens a fullscreen presenter window showing the current deck.
#[tauri::command]
pub fn start_presenter(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    {
        let guard = state.session.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            return Err("no deck is open".to_string());
        }
    }
    *state
        .presenter_index
        .lock()
        .map_err(|e| e.to_string())? = 0;
    tauri::WebviewWindowBuilder::new(
        &app,
        "presenter",
        tauri::WebviewUrl::App("index.html#/presenter".into()),
    )
    .title("Presenter")
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
        guard
            .as_ref()
            .map(|s| s.deck().slides.len())
            .unwrap_or(0)
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
pub fn restore_recovery(
    id: String,
    state: State<'_, AppState>,
) -> Result<DeckSnapshot, String> {
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
    *state
        .session
        .lock()
        .map_err(|e| e.to_string())? = Some(session);
    *state
        .presenter_index
        .lock()
        .map_err(|e| e.to_string())? = 0;
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

fn paragraph_to_dto(paragraph: &slides_core::Paragraph) -> ParagraphDto {
    ParagraphDto {
        runs: paragraph.runs.iter().map(run_to_dto).collect(),
        list_style: match paragraph.list_style {
            slides_core::ListStyle::None => "none".to_string(),
            slides_core::ListStyle::Ordered => "ordered".to_string(),
            slides_core::ListStyle::Unordered => "unordered".to_string(),
        },
    }
}

fn run_to_dto(run: &slides_core::Run) -> RunDto {
    RunDto {
        text: run.text.clone(),
        bold: run.bold,
        italic: run.italic,
        underline: run.underline,
    }
}

fn run_from_dto(run: &RunDto) -> slides_core::Run {
    slides_core::Run {
        text: run.text.clone(),
        bold: run.bold,
        italic: run.italic,
        underline: run.underline,
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
    use super::{sanitize_recovery_id, AppState};
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
}
