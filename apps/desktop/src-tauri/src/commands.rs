//! Tauri commands for the 900Slides desktop application.
//!
//! This module exposes the v0.1.0 command surface: deck creation, opening,
//! saving, text editing, undo, presenter mode, and recovery snapshots. Every
//! mutation is applied transactionally in Rust, and the frontend always
//! re-renders from the returned deck snapshot.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

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
        }
    }
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
    let snapshot = deck_to_dto(session.deck());
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
    let mut snapshot = deck_to_dto(session.deck());
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
    guard.as_ref().map(|s| deck_to_dto(s.deck()))
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
    let snapshot = deck_to_dto(session.deck());
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
    let snapshot = deck_to_dto(session.deck());
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
    let snapshot = deck_to_dto(session.deck());
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
    let mut snapshot = deck_to_dto(session.deck());
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

fn deck_to_dto(deck: &slides_core::Deck) -> DeckSnapshot {
    DeckSnapshot {
        id: deck.id.clone(),
        schema_version: deck.schema_version,
        theme: theme_to_dto(&deck.theme),
        slides: deck.slides.iter().map(slide_to_dto).collect(),
        warnings: Vec::new(),
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
