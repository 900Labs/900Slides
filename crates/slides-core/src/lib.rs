//! Deck model, commands, undo / redo, theme.

/// A deck: the root of a presentation document.
#[derive(Debug, Default, Clone)]
pub struct Deck;

/// A single slide within a deck.
#[derive(Debug, Default, Clone)]
pub struct Slide;

/// A shape or content object placed on a slide.
#[derive(Debug, Default, Clone)]
pub struct Shape;

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[test]
fn placeholders_exist() {
    let _deck = Deck;
    let _slide = Slide;
    let _shape = Shape;
    assert!(!version().is_empty());
}
