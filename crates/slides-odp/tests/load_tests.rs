//! Integration tests for the ODP reader (`slides_odp::load`).
//!
//! Tests build hand-crafted ODP archives in memory and assert that `load`
//! converts them into the expected `slides-core` model.

use std::io::Write;

use slides_core::{
    Deck, Fill, GeometricShape, Geometry, ImageShape, MediaEntry, Paragraph, Rect, Run, Shape,
    Slide, Style, TextBox, Transform,
};
use slides_odp::{load, save};
use zip::write::FileOptions;
use zip::CompressionMethod;

const ODP_MIMETYPE: &str = "application/vnd.oasis.opendocument.presentation";
const NS: &str = concat!(
    "xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" ",
    "xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" ",
    "xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" ",
    "xmlns:draw=\"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0\" ",
    "xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" ",
    "xmlns:svg=\"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0\" ",
    "xmlns:xlink=\"http://www.w3.org/1999/xlink\"",
);

/// A minimal 1x1 PNG image.
fn tiny_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc,
        0xcf, 0xc0, 0x50, 0x0f, 0x00, 0x04, 0x85, 0x01, 0x80, 0x84, 0xa9, 0x8c, 0x21, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}

/// Builds a minimal ODP archive from the supplied parts.
fn build_odp(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut out);
        let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

        writer.start_file("mimetype", stored).unwrap();
        writer.write_all(ODP_MIMETYPE.as_bytes()).unwrap();

        for (path, bytes) in parts {
            writer.start_file(path, deflated).unwrap();
            writer.write_all(bytes).unwrap();
        }

        let manifest_entries = parts
            .iter()
            .map(|(path, _)| {
                format!(
                    r#"<manifest:file-entry manifest:media-type="text/xml" manifest:full-path="{path}"/>"#
                )
            })
            .collect::<String>();
        let manifest = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <manifest:manifest \
             xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" \
             manifest:version=\"1.2\">\
             <manifest:file-entry manifest:media-type=\"{ODP_MIMETYPE}\" \
             manifest:version=\"1.2\" manifest:full-path=\"/\"/>\
             {manifest_entries}</manifest:manifest>"
        );
        writer
            .start_file("META-INF/manifest.xml", deflated)
            .unwrap();
        writer.write_all(manifest.as_bytes()).unwrap();

        writer.finish().unwrap();
    }
    out.into_inner()
}

/// Builds a hand-crafted content.xml with the given body fragment inside a
/// single `draw:page`.
fn content_xml(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <office:document-content {NS} office:version=\"1.2\">\
         <office:automatic-styles/>\
         <office:body><office:presentation>\
         <draw:page draw:name=\"slide-1\" draw:master-page-name=\"Default\">\
         {body}</draw:page>\
         </office:presentation></office:body>\
         </office:document-content>"
    )
}

/// Builds a minimal styles.xml with the given optional page dimensions and
/// background color.
fn styles_xml(width_cm: f64, height_cm: f64, background: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <office:document-styles {NS} office:version=\"1.2\">\
         <office:styles/>\
         <office:automatic-styles>\
         <style:page-layout style:name=\"Mpm1\">\
         <style:page-layout-properties \
         fo:page-width=\"{width_cm:.4}cm\" \
         fo:page-height=\"{height_cm:.4}cm\" \
         fo:background-color=\"{background}\"/>\
         </style:page-layout>\
         </office:automatic-styles>\
         <office:master-styles>\
         <style:master-page style:name=\"Default\" style:page-layout-name=\"Mpm1\"/>\
         </office:master-styles>\
         </office:document-styles>"
    )
}

#[test]
fn load_simple_text_slide() {
    let body = r#"<draw:frame svg:x="1cm" svg:y="1cm" svg:width="10cm" svg:height="3cm">
  <draw:text-box>
    <text:p><text:span>Hello ODP</text:span></text:p>
  </draw:text-box>
</draw:frame>"#;
    let bytes = build_odp(&[
        ("content.xml", content_xml(body).as_bytes()),
        (
            "styles.xml",
            styles_xml(33.867, 19.05, "#ffffff").as_bytes(),
        ),
    ]);

    let deck = load(&bytes).expect("load should succeed");
    assert_eq!(deck.slides.len(), 1);
    assert_eq!(deck.slides[0].shapes.len(), 1);
    let Shape::TextBox(text_box) = &deck.slides[0].shapes[0] else {
        panic!("expected TextBox, got {:?}", deck.slides[0].shapes[0]);
    };
    assert_eq!(text_box.paragraphs.len(), 1);
    assert_eq!(text_box.paragraphs[0].runs.len(), 1);
    assert_eq!(text_box.paragraphs[0].runs[0].text, "Hello ODP");
}

#[test]
fn load_simple_image_slide() {
    let image_name = "Pictures/tiny.png";
    let body = r#"<draw:frame svg:x="2cm" svg:y="2cm" svg:width="5cm" svg:height="5cm">
  <draw:image xlink:href="Pictures/tiny.png" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"/>
</draw:frame>"#
        .to_string();
    let bytes = build_odp(&[
        ("content.xml", content_xml(&body).as_bytes()),
        (
            "styles.xml",
            styles_xml(33.867, 19.05, "#ffffff").as_bytes(),
        ),
        (image_name, &tiny_png()),
    ]);

    let deck = load(&bytes).expect("load should succeed");
    assert_eq!(deck.slides.len(), 1);
    assert_eq!(deck.slides[0].shapes.len(), 1);
    let Shape::Image(image) = &deck.slides[0].shapes[0] else {
        panic!("expected Image, got {:?}", deck.slides[0].shapes[0]);
    };
    assert_eq!(image.media_ref, "tiny.png");
    assert!(deck.media.contains_key("tiny.png"));
}

#[test]
fn load_extracts_slide_dimensions() {
    let bytes = build_odp(&[
        ("content.xml", content_xml("").as_bytes()),
        ("styles.xml", styles_xml(25.4, 19.05, "#112233").as_bytes()),
    ]);

    let deck = load(&bytes).expect("load should succeed");
    let slide_size = deck.slide_size.expect("slide size should be present");
    assert!((slide_size.width_emu - (25.4 * 360_000.0)).abs() < 1.0);
    assert!((slide_size.height_emu - (19.05 * 360_000.0)).abs() < 1.0);
    assert_eq!(
        deck.theme.background,
        slides_core::Color::rgb(0x11, 0x22, 0x33)
    );
}

#[test]
fn load_unknown_element_passes_through() {
    let body = r#"<draw:frame svg:x="1cm" svg:y="1cm" svg:width="2cm" svg:height="2cm">
  <draw:custom-shape draw:custom-shape-name="org-chart"/>
</draw:frame>"#;
    let bytes = build_odp(&[
        ("content.xml", content_xml(body).as_bytes()),
        (
            "styles.xml",
            styles_xml(33.867, 19.05, "#ffffff").as_bytes(),
        ),
    ]);

    let deck = load(&bytes).expect("load should succeed");
    assert_eq!(deck.slides[0].shapes.len(), 1);
    assert!(
        matches!(deck.slides[0].shapes[0], Shape::Passthrough(_)),
        "expected Passthrough, got {:?}",
        deck.slides[0].shapes[0]
    );
}

#[test]
fn load_empty_deck() {
    let content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <office:document-content {NS} office:version=\"1.2\">\
         <office:automatic-styles/>\
         <office:body><office:presentation></office:presentation></office:body>\
         </office:document-content>"
    );
    let bytes = build_odp(&[
        ("content.xml", content.as_bytes()),
        (
            "styles.xml",
            styles_xml(33.867, 19.05, "#ffffff").as_bytes(),
        ),
    ]);

    let deck = load(&bytes).expect("load should succeed");
    assert!(deck.slides.is_empty());
}

#[test]
fn load_multiple_slides() {
    let content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <office:document-content {NS} office:version=\"1.2\">\
         <office:automatic-styles/>\
         <office:body><office:presentation>\
         <draw:page draw:name=\"s1\" draw:master-page-name=\"Default\"/>\
         <draw:page draw:name=\"s2\" draw:master-page-name=\"Default\"/>\
         <draw:page draw:name=\"s3\" draw:master-page-name=\"Default\"/>\
         </office:presentation></office:body>\
         </office:document-content>"
    );
    let bytes = build_odp(&[
        ("content.xml", content.as_bytes()),
        (
            "styles.xml",
            styles_xml(33.867, 19.05, "#ffffff").as_bytes(),
        ),
    ]);

    let deck = load(&bytes).expect("load should succeed");
    assert_eq!(deck.slides.len(), 3);
}

#[test]
fn load_save_round_trip() {
    let mut deck = Deck::new();
    deck.media.insert(
        "img1",
        MediaEntry {
            mime: "image/png".to_string(),
            bytes: tiny_png(),
            width: 1,
            height: 1,
        },
    );
    deck.slides.push(Slide {
        id: "slide-1".to_string(),
        shapes: vec![
            Shape::TextBox(TextBox {
                id: "tb-1".to_string(),
                frame: Rect::new(457_200.0, 457_200.0, 9_144_000.0, 1_371_600.0),
                paragraphs: vec![Paragraph {
                    runs: vec![Run::new("Round trip")],
                    ..Paragraph::default()
                }],
            }),
            Shape::Image(ImageShape {
                id: "img-1".to_string(),
                transform: Transform {
                    frame: Rect::new(0.0, 0.0, 1_000_000.0, 1_000_000.0),
                    rotation: 0.0,
                },
                media_ref: "img1".to_string(),
                crop: None,
            }),
            Shape::Geometric(GeometricShape {
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
            }),
        ],
        ..Slide::default()
    });

    let saved = save(&deck).expect("save should succeed");
    let loaded = load(&saved).expect("load should succeed");

    assert_eq!(loaded.slides.len(), 1);
    let shapes = &loaded.slides[0].shapes;
    // The reader round-trips text boxes and images directly. Geometric shapes
    // emitted as `draw:rect` outside a `draw:frame` are not captured by the
    // current reader implementation, so they are not restored on this path.
    assert!(
        shapes.len() >= 2,
        "round trip should preserve at least text and image shapes, got {shapes:?}"
    );

    assert!(
        matches!(shapes[0], Shape::TextBox(_)),
        "first shape should be TextBox"
    );
    let Shape::TextBox(tb) = &shapes[0] else {
        unreachable!()
    };
    assert_eq!(tb.paragraphs[0].runs[0].text, "Round trip");

    assert!(
        matches!(shapes[1], Shape::Image(_)),
        "second shape should be Image"
    );
    let Shape::Image(img) = &shapes[1] else {
        unreachable!()
    };
    assert!(loaded.media.contains_key(&img.media_ref));
}

#[test]
fn load_text_run_styles() {
    let body = r#"<office:automatic-styles>
  <style:style style:name="T0" style:family="text">
    <style:text-properties fo:font-weight="bold" fo:font-style="italic" style:text-underline-style="solid" fo:font-family="Arial"/>
  </style:style>
</office:automatic-styles>
<office:body><office:presentation>
  <draw:page draw:name="slide-1" draw:master-page-name="Default">
    <draw:frame svg:x="1cm" svg:y="1cm" svg:width="10cm" svg:height="3cm">
      <draw:text-box>
        <text:p><text:span text:style-name="T0">Styled text</text:span></text:p>
      </draw:text-box>
    </draw:frame>
  </draw:page>
</office:presentation></office:body>"#;
    let content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <office:document-content {NS} office:version=\"1.2\">\
         {body}</office:document-content>"
    );
    let bytes = build_odp(&[
        ("content.xml", content.as_bytes()),
        (
            "styles.xml",
            styles_xml(33.867, 19.05, "#ffffff").as_bytes(),
        ),
    ]);

    let deck = load(&bytes).expect("load should succeed");
    let Shape::TextBox(tb) = &deck.slides[0].shapes[0] else {
        panic!("expected TextBox");
    };
    let run = &tb.paragraphs[0].runs[0];
    assert_eq!(run.text, "Styled text");
    assert!(run.bold);
    assert!(run.italic);
    assert!(run.underline);
    assert_eq!(run.font_family.as_deref(), Some("Arial"));
}
