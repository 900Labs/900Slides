//! Integration tests for the ODP writer (`slides_odp::save`).
//!
//! Each test builds a `slides_core::Deck` directly from the model types and
//! asserts on the produced `.odp` bytes. The writer is never round-tripped
//! through the reader (which is implemented in a parallel task), so these tests
//! stay isolated from the load path.

use std::io::Read;

use slides_core::{
    Deck, Fill, GeometricShape, Geometry, ImageShape, MediaEntry, Paragraph, Rect, Run, Shape,
    Slide, Style, TableShape, TextBox, Transform,
};
use slides_odp::save;
use zip::ZipArchive;

const ODP_MIMETYPE: &str = "application/vnd.oasis.opendocument.presentation";

/// Reads a single decompressed entry from the archive as a UTF-8 string.
fn read_entry(bytes: &[u8], name: &str) -> String {
    let mut zip = ZipArchive::new(std::io::Cursor::new(bytes)).expect("valid zip");
    let mut text = String::new();
    zip.by_name(name)
        .expect("entry exists")
        .read_to_string(&mut text)
        .expect("utf8");
    text
}

/// Builds a deck with one slide holding a single text box.
fn one_text_box_deck() -> Deck {
    let mut deck = Deck::new();
    deck.slides.push(Slide {
        id: "slide-1".to_string(),
        shapes: vec![Shape::TextBox(TextBox {
            id: "tb-1".to_string(),
            frame: Rect::new(457_200.0, 457_200.0, 9_144_000.0, 1_371_600.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("Hello ODP")],
                ..Paragraph::default()
            }],
        })],
        ..Slide::default()
    });
    deck
}

#[test]
fn save_simple_deck_produces_valid_odp() {
    let bytes = save(&one_text_box_deck()).expect("save ok");

    // ZIP magic.
    assert_eq!(&bytes[..2], b"PK", "output must start with the ZIP magic");

    let mut zip = ZipArchive::new(std::io::Cursor::new(&bytes)).expect("valid zip");

    // The mimetype entry must be the first file in the archive.
    let first = zip
        .by_index(0)
        .expect("first entry exists")
        .name()
        .to_string();
    assert_eq!(first, "mimetype", "mimetype must be the first entry");

    // Its content must be the ODP presentation MIME type.
    let mut mime = String::new();
    zip.by_name("mimetype")
        .expect("mimetype entry exists")
        .read_to_string(&mut mime)
        .expect("utf8");
    assert_eq!(mime, ODP_MIMETYPE);
}

#[test]
fn save_text_box_content() {
    let bytes = save(&one_text_box_deck()).expect("save ok");
    let content = read_entry(&bytes, "content.xml");
    assert!(
        content.contains("Hello ODP"),
        "content.xml must contain the text-box text, got: {content}"
    );
}

#[test]
fn save_text_box_formats_runs() {
    let mut deck = Deck::new();
    deck.slides.push(Slide {
        id: "slide-1".to_string(),
        shapes: vec![Shape::TextBox(TextBox {
            id: "tb-1".to_string(),
            frame: Rect::new(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("bold").bold()],
                ..Paragraph::default()
            }],
        })],
        ..Slide::default()
    });

    let bytes = save(&deck).expect("save ok");
    let content = read_entry(&bytes, "content.xml");
    assert!(
        content.contains(r#"fo:font-weight="bold""#),
        "bold run must emit fo:font-weight, got: {content}"
    );
    assert!(
        content.contains(r#"text:style-name="T0""#),
        "formatted run must reference an automatic style, got: {content}"
    );
}

#[test]
fn save_image_writes_picture() {
    let mut deck = Deck::new();
    deck.media.insert(
        "img1",
        MediaEntry {
            mime: "image/png".to_string(),
            bytes: vec![0x89, b'P', b'N', b'G'],
            width: 10,
            height: 10,
        },
    );
    deck.slides.push(Slide {
        id: "slide-1".to_string(),
        shapes: vec![Shape::Image(ImageShape {
            id: "img-1".to_string(),
            transform: Transform {
                frame: Rect::new(0.0, 0.0, 1_000_000.0, 1_000_000.0),
                rotation: 0.0,
            },
            media_ref: "img1".to_string(),
            crop: None,
            alt_text: None,
        })],
        ..Slide::default()
    });

    let bytes = save(&deck).expect("save ok");
    let zip = ZipArchive::new(std::io::Cursor::new(&bytes)).expect("valid zip");
    let has_picture = zip.file_names().any(|name| name.starts_with("Pictures/"));
    assert!(has_picture, "archive must contain a Pictures/ entry");

    let content = read_entry(&bytes, "content.xml");
    assert!(
        content.contains("Pictures/img1"),
        "content.xml must reference the embedded image, got: {content}"
    );
}

#[test]
fn save_deterministic() {
    let deck = one_text_box_deck();
    let first = save(&deck).expect("save ok");
    let second = save(&deck).expect("save ok");
    assert_eq!(
        first, second,
        "saving the same deck twice must produce identical bytes"
    );
}

#[test]
fn save_multiple_slides() {
    let mut deck = Deck::new();
    for i in 0..3 {
        deck.slides.push(Slide {
            id: format!("slide-{i}"),
            shapes: vec![Shape::TextBox(TextBox {
                id: format!("tb-{i}"),
                frame: Rect::new(0.0, 0.0, 1_000_000.0, 500_000.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new(format!("Slide {i}"))],
                    ..Paragraph::default()
                }],
            })],
            ..Slide::default()
        });
    }

    let bytes = save(&deck).expect("save ok");
    let content = read_entry(&bytes, "content.xml");
    let page_count = content.matches("<draw:page").count();
    assert_eq!(
        page_count, 3,
        "expected 3 draw:page elements, got {page_count}"
    );
}

#[test]
fn save_geometric_shape() {
    let mut deck = Deck::new();
    deck.slides.push(Slide {
        id: "slide-1".to_string(),
        shapes: vec![Shape::Geometric(GeometricShape {
            id: "geo-1".to_string(),
            transform: Transform {
                frame: Rect::new(914_400.0, 914_400.0, 2_000_000.0, 1_000_000.0),
                rotation: 0.0,
            },
            geometry: Geometry::Rectangle,
            style: Style {
                fill: Some(Fill::Solid(slides_core::Color::rgb(0, 0, 255))),
                outline: None,
                shadow: None,
            },
        })],
        ..Slide::default()
    });

    let bytes = save(&deck).expect("save ok");
    let content = read_entry(&bytes, "content.xml");
    assert!(
        content.contains("<draw:rect"),
        "rectangle geometry must emit draw:rect, got: {content}"
    );
    assert!(
        content.contains("draw:fill-color=\"#0000FF\""),
        "rectangle fill color must be emitted, got: {content}"
    );
}

#[test]
fn save_table_shape() {
    let mut deck = Deck::new();
    let table = TableShape::default_grid(2, 2, Rect::new(0.0, 0.0, 4_000_000.0, 2_000_000.0));
    deck.slides.push(Slide {
        id: "slide-1".to_string(),
        shapes: vec![Shape::Table(table)],
        ..Slide::default()
    });

    let bytes = save(&deck).expect("save ok");
    let content = read_entry(&bytes, "content.xml");
    assert!(
        content.contains("<table:table>"),
        "table shape must emit table:table, got: {content}"
    );
}

#[test]
fn save_validates_as_zip_and_xml() {
    // Sanity: every declared entry can be read, and the manifest references
    // the core parts.
    let bytes = save(&one_text_box_deck()).expect("save ok");
    let zip = ZipArchive::new(std::io::Cursor::new(&bytes)).expect("valid zip");

    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    for required in [
        "mimetype",
        "content.xml",
        "styles.xml",
        "meta.xml",
        "META-INF/manifest.xml",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "archive must contain {required}, got {names:?}"
        );
    }

    let manifest = read_entry(&bytes, "META-INF/manifest.xml");
    assert!(manifest.contains(ODP_MIMETYPE));
    assert!(manifest.contains("content.xml"));
}
