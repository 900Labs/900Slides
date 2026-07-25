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

/// An inline run of text with formatting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    /// Text content.
    pub text: String,
    /// Bold formatting.
    pub bold: bool,
    /// Italic formatting.
    pub italic: bool,
    /// Underline formatting.
    pub underline: bool,
}

impl Run {
    /// Creates a plain run of text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            underline: false,
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
        serde_json::to_string(self).map_or(0, |s| s.len())
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
        serde_json::to_string(self).map_or(0, |s| s.len())
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
                    },
                    Paragraph {
                        runs: vec![Run::new("World").italic()],
                        list_style: ListStyle::None,
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
                },
                Paragraph {
                    runs: vec![Run::new("Moon")],
                    list_style: ListStyle::None,
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
}
