//! Deck model, commands, undo / redo, theme.

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
}

impl Default for Deck {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: String::new(),
            theme: Theme::default(),
            slides: Vec::new(),
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
}

/// An editable text box shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBox {
    /// Bounding rectangle of the text box, in EMU.
    pub frame: Rect,
    /// Paragraphs of text inside the box.
    pub paragraphs: Vec<Paragraph>,
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
    /// returned.
    pub fn apply(
        &mut self,
        command: Box<dyn Command>,
        deck: &mut Deck,
    ) -> Result<(), CommandError> {
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
    /// Returns `false` if there was nothing to undo.
    pub fn undo(&mut self, deck: &mut Deck) -> bool {
        let Some(inverse) = self.undo_stack.pop() else {
            return false;
        };
        self.total_size = self.total_size.saturating_sub(inverse.serialized_size());
        inverse.apply(deck);
        true
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

        assert!(bus.undo(&mut deck));
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
}
