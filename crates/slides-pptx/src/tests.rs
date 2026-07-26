//! Tests for the PPTX load/save boundary.

use std::collections::HashSet;
use std::io::{Read, Write};

use slides_core::{
    AddShape, Color, DashStyle, Deck, DeleteShape, EditText, Fill, GeometricShape, Geometry,
    InsertImage, ListStyle, MediaEntry, MoveShape, Outline, Paragraph, Rect, Run, SetShapeStyle,
    Shape, Style, TextBox, Transform,
};
use zip::write::{FileOptions, ZipWriter};

use crate::{load, save};

const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_TYPE_MANIFEST: &str = "http://900labs.github.io/900Slides/1.0/relationships/manifest";
const REL_TYPE_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const CT_MANIFEST: &str = "application/vnd.900labs.900slides.manifest+xml";

fn build_minimal_pptx() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut buf);
        let options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        writer.start_file("[Content_Types].xml", options).unwrap();
        writer.write_all(content_types_xml().as_bytes()).unwrap();

        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(package_rels_xml().as_bytes()).unwrap();

        writer.start_file("ppt/presentation.xml", options).unwrap();
        writer.write_all(presentation_xml().as_bytes()).unwrap();

        writer
            .start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        writer
            .write_all(presentation_rels_xml().as_bytes())
            .unwrap();

        writer.start_file("ppt/slides/slide1.xml", options).unwrap();
        writer.write_all(slide1_xml().as_bytes()).unwrap();

        writer.start_file("ppt/theme/theme1.xml", options).unwrap();
        writer.write_all(theme_xml().as_bytes()).unwrap();

        writer.start_file("customXml/item1.xml", options).unwrap();
        writer.write_all(manifest_xml().as_bytes()).unwrap();

        writer.finish().unwrap();
    }
    buf.into_inner()
}

fn content_types_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/customXml/item1.xml" ContentType="{CT_MANIFEST}"/>
</Types>"#
    )
}

fn package_rels_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
  <Relationship Id="rId2" Type="{REL_TYPE_MANIFEST}" Target="customXml/item1.xml"/>
</Relationships>"#
    )
}

fn presentation_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
</Relationships>"#
        .to_string()
}

fn presentation_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="{P_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">
  <p:sldIdLst>
    <p:sldId id="256" r:id="rId1"/>
  </p:sldIdLst>
</p:presentation>"#
    )
}

fn slide1_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="{P_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="0" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="TextBox 1"/>
          <p:cNvSpPr/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="914400" y="457200"/>
            <a:ext cx="4572000" cy="762000"/>
          </a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p>
            <a:pPr/>
            <a:r>
              <a:rPr b="1"/>
              <a:t xml:space="preserve">Hello</a:t>
            </a:r>
            <a:r>
              <a:rPr i="1"/>
              <a:t xml:space="preserve"> world</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
      <p:pic>
        <p:nvPicPr>
          <p:cNvPr id="3" name="Picture 1"/>
          <p:cNvPicPr/>
          <p:nvPr/>
        </p:nvPicPr>
        <p:blipFill/>
        <p:spPr/>
      </p:pic>
    </p:spTree>
  </p:cSld>
</p:sld>"#
    )
}

fn theme_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="{A_NS}" name="Office Theme">
  <a:themeElements>
    <a:clrScheme name="Office">
      <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="44546A"/></a:dk2>
      <a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
      <a:accent1><a:srgbClr val="4472C4"/></a:accent1>
      <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
    </a:clrScheme>
    <a:fontScheme name="Office">
      <a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont>
      <a:minorFont><a:latin typeface="Calibri"/></a:minorFont>
    </a:fontScheme>
  </a:themeElements>
</a:theme>"#
    )
}

fn manifest_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<manifest xmlns="http://900labs.github.io/900Slides/1.0" appVersion="0.1.0" schemaVersion="1" deckId="fixture-deck-id"/>"#
        .to_string()
}

fn zip_entries(bytes: &[u8]) -> HashSet<String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut set = HashSet::new();
    for i in 0..archive.len() {
        set.insert(archive.by_index(i).unwrap().name().to_string());
    }
    set
}

fn entry_bytes(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut out = Vec::new();
    std::io::copy(&mut file, &mut out).unwrap();
    out
}

#[test]
fn round_trip_preserves_untouched_parts() {
    let original = build_minimal_pptx();
    let session = load(&original).expect("load should succeed");
    let saved = save(&session).expect("save should succeed");

    let original_entries = zip_entries(&original);
    let saved_entries = zip_entries(&saved);
    assert_eq!(original_entries, saved_entries);

    for name in original_entries {
        if name == "[Content_Types].xml" || name == "customXml/item1.xml" {
            continue;
        }
        assert_eq!(
            entry_bytes(&original, &name),
            entry_bytes(&saved, &name),
            "{name} should be byte-identical"
        );
    }
}

#[test]
fn edit_changes_only_target_slide() {
    let original = build_minimal_pptx();
    let mut session = load(&original).expect("load should succeed");

    let slide_id = "ppt/slides/slide1.xml".to_string();
    session
        .execute(Box::new(EditText::new(
            slide_id.clone(),
            0,
            0,
            vec![Run::new("Goodbye").bold()],
        )))
        .expect("edit should apply");

    let saved = save(&session).expect("save should succeed");

    for name in zip_entries(&original) {
        if name == "[Content_Types].xml"
            || name == "customXml/item1.xml"
            || name == "ppt/slides/slide1.xml"
        {
            continue;
        }
        assert_eq!(
            entry_bytes(&original, &name),
            entry_bytes(&saved, &name),
            "{name} should be byte-identical"
        );
    }

    let original_slide =
        String::from_utf8(entry_bytes(&original, "ppt/slides/slide1.xml")).unwrap();
    let saved_slide = String::from_utf8(entry_bytes(&saved, "ppt/slides/slide1.xml")).unwrap();
    assert!(
        saved_slide.contains("Goodbye"),
        "edited slide should contain the new text"
    );
    assert_ne!(
        original_slide, saved_slide,
        "edited slide should have changed"
    );
}

#[test]
fn load_extracts_text_box_and_passthrough() {
    let original = build_minimal_pptx();
    let session = load(&original).expect("load should succeed");

    assert_eq!(session.deck().slides.len(), 1);
    let slide = &session.deck().slides[0];
    assert_eq!(slide.id, "ppt/slides/slide1.xml");
    assert_eq!(slide.shapes.len(), 2);

    match &slide.shapes[0] {
        Shape::TextBox(text_box) => {
            assert_eq!(text_box.paragraphs.len(), 1);
            assert_eq!(text_box.paragraphs[0].runs.len(), 2);
            assert!(text_box.paragraphs[0].runs[0].bold);
            assert!(!text_box.paragraphs[0].runs[0].italic);
            assert_eq!(text_box.paragraphs[0].runs[0].text, "Hello");
            assert!(text_box.paragraphs[0].runs[1].italic);
        }
        _ => panic!("first shape should be a text box"),
    }

    match &slide.shapes[1] {
        Shape::Passthrough(obj) => {
            assert_eq!(obj.label, "pic");
            assert!(!obj.raw_bytes.is_empty());
        }
        _ => panic!("second shape should be passthrough"),
    }
}

#[test]
fn load_extracts_theme() {
    let original = build_minimal_pptx();
    let session = load(&original).expect("load should succeed");

    assert_eq!(session.deck().theme.background, slides_core::Color::white());
    assert_eq!(
        session.deck().theme.accent_color,
        slides_core::Color::rgb(68, 114, 196)
    );
    assert_eq!(session.deck().theme.heading_font, "Calibri Light");
    assert_eq!(session.deck().theme.body_font, "Calibri");
}

#[test]
fn loss_ledger_records_passthrough_warning() {
    let original = build_minimal_pptx();
    let session = load(&original).expect("load should succeed");

    assert!(!session.loss_ledger().is_empty());
    let warnings = session.loss_ledger().warnings();
    assert!(warnings
        .iter()
        .any(|w| w.slide_id == "ppt/slides/slide1.xml"
            && w.message.contains("pic")
            && w.message.contains("opaque")));
}

#[test]
fn deck_round_trip_serialization() {
    let mut deck = Deck::new();
    deck.slides.push(slides_core::Slide {
        id: "slide-1".to_string(),
        notes: "note".to_string(),
        shapes: vec![Shape::TextBox(TextBox {
            frame: Rect::new(0.0, 0.0, 100.0, 100.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("test").bold().italic().underline()],
                list_style: ListStyle::Unordered,
            }],
        })],
        animation: None,
        transition: None,
    });

    let json = serde_json::to_string(&deck).expect("serialize");
    let restored: Deck = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deck, restored);
}

fn build_pptx_with_notes() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut buf);
        let options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        writer.start_file("[Content_Types].xml", options).unwrap();
        writer
            .write_all(content_types_with_notes_xml().as_bytes())
            .unwrap();

        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(package_rels_xml().as_bytes()).unwrap();

        writer.start_file("ppt/presentation.xml", options).unwrap();
        writer.write_all(presentation_xml().as_bytes()).unwrap();

        writer
            .start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        writer
            .write_all(presentation_rels_xml().as_bytes())
            .unwrap();

        writer
            .start_file("ppt/slides/_rels/slide1.xml.rels", options)
            .unwrap();
        writer.write_all(slide1_rels_xml().as_bytes()).unwrap();

        writer.start_file("ppt/slides/slide1.xml", options).unwrap();
        writer.write_all(slide1_xml().as_bytes()).unwrap();

        writer
            .start_file("ppt/notesSlides/notesSlide1.xml", options)
            .unwrap();
        writer.write_all(notes_slide1_xml().as_bytes()).unwrap();

        writer.start_file("ppt/theme/theme1.xml", options).unwrap();
        writer.write_all(theme_xml().as_bytes()).unwrap();

        writer.start_file("customXml/item1.xml", options).unwrap();
        writer.write_all(manifest_xml().as_bytes()).unwrap();

        writer.finish().unwrap();
    }
    buf.into_inner()
}

fn content_types_with_notes_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/notesSlides/notesSlide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/customXml/item1.xml" ContentType="{CT_MANIFEST}"/>
</Types>"#
    )
}

fn slide1_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide1.xml"/>
</Relationships>"#
        .to_string()
}

fn notes_slide1_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notes xmlns:p="{P_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="0" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Slide Image Placeholder 1"/>
          <p:cNvSpPr/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr/>
      </p:sp>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Notes Placeholder 2"/>
          <p:cNvSpPr/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr/>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p>
            <a:r>
              <a:rPr/>
              <a:t>First note</a:t>
            </a:r>
          </a:p>
          <a:p>
            <a:r>
              <a:rPr/>
              <a:t>Second note</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:notes>"#
    )
}

#[test]
fn load_extracts_notes() {
    let original = build_pptx_with_notes();
    let session = load(&original).expect("load should succeed");

    assert_eq!(session.deck().slides.len(), 1);
    assert_eq!(session.deck().slides[0].notes, "First note\nSecond note");
}

#[test]
fn save_preserves_non_text_xml() {
    let original = build_minimal_pptx();
    let mut session = load(&original).expect("load should succeed");

    let slide_id = "ppt/slides/slide1.xml".to_string();
    session
        .execute(Box::new(EditText::new(
            slide_id.clone(),
            0,
            0,
            vec![Run::new("Goodbye").bold()],
        )))
        .expect("edit should apply");

    let saved = save(&session).expect("save should succeed");
    let saved_slide = String::from_utf8(entry_bytes(&saved, "ppt/slides/slide1.xml")).unwrap();

    assert!(
        saved_slide.contains("Goodbye"),
        "edited slide should contain new text"
    );
    assert!(
        saved_slide.contains("<p:pic>"),
        "non-text shape should survive the save"
    );
}

#[test]
fn save_clears_dirty_and_updates_original_bytes() {
    let original = build_minimal_pptx();
    let mut session = load(&original).expect("load should succeed");

    let slide_id = "ppt/slides/slide1.xml".to_string();
    session
        .execute(Box::new(EditText::new(
            slide_id.clone(),
            0,
            0,
            vec![Run::new("First edit")],
        )))
        .expect("edit should apply");
    assert!(!session.dirty_slides().is_empty());

    let first_save = save(&session).expect("first save should succeed");
    session.commit_save(first_save.clone());
    assert!(session.dirty_slides().is_empty());

    session
        .execute(Box::new(EditText::new(
            slide_id.clone(),
            0,
            0,
            vec![Run::new("Second edit")],
        )))
        .expect("second edit should apply");

    let second_save = save(&session).expect("second save should succeed");
    let second_slide =
        String::from_utf8(entry_bytes(&second_save, "ppt/slides/slide1.xml")).unwrap();
    eprintln!("second_slide={second_slide}");
    assert!(
        second_slide.contains("Second edit"),
        "second edit should appear in the saved slide"
    );
    assert!(
        !second_slide.contains("First edit"),
        "first edit should be replaced by the second edit"
    );
}

#[test]
fn blank_theme_font_scheme_is_outside_color_scheme() {
    let bytes = crate::create_blank_pptx();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut file = archive.by_name("ppt/theme/theme1.xml").unwrap();
    let mut xml = String::new();
    file.read_to_string(&mut xml).unwrap();

    let clr_close = xml
        .find("</a:clrScheme>")
        .expect("clrScheme close tag should exist");
    let font_open = xml
        .find("<a:fontScheme")
        .expect("fontScheme open tag should exist");
    assert!(
        clr_close < font_open,
        "fontScheme must not be nested inside clrScheme"
    );

    let session = load(crate::create_blank_pptx().as_slice()).expect("load should succeed");
    assert_eq!(session.deck().theme.heading_font, "Calibri Light");
    assert_eq!(session.deck().theme.body_font, "Calibri");
}

#[test]
fn load_extracts_passthrough_frame() {
    let original = build_minimal_pptx();
    let session = load(&original).expect("load should succeed");

    match &session.deck().slides[0].shapes[1] {
        Shape::Passthrough(obj) => {
            // The fixture picture does not have an xfrm, so the frame is None.
            assert!(obj.frame.is_none());
        }
        _ => panic!("second shape should be passthrough"),
    }
}

// ---------------------------------------------------------------------------
// Image and geometric-shape loading and saving (Wave 1 components 4 & 5)
// ---------------------------------------------------------------------------

/// Encodes a solid red PNG of the given dimensions for use in fixtures.
fn real_png(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(width, height, image::Rgba([255, 0, 0, 255]));
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .expect("encode png");
    out
}

/// Assembles a single-slide PPTX package from a slide body, an optional slide
/// relationships part, and a set of media parts.
fn build_pptx(slide_xml: &str, slide_rels: Option<&str>, media: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut buf);
        let options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        writer.start_file("[Content_Types].xml", options).unwrap();
        let ct = if media.is_empty() {
            content_types_xml()
        } else {
            image_content_types_xml()
        };
        writer.write_all(ct.as_bytes()).unwrap();

        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(package_rels_xml().as_bytes()).unwrap();

        writer.start_file("ppt/presentation.xml", options).unwrap();
        writer.write_all(presentation_xml().as_bytes()).unwrap();

        writer
            .start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        writer
            .write_all(presentation_rels_xml().as_bytes())
            .unwrap();

        if let Some(rels) = slide_rels {
            writer
                .start_file("ppt/slides/_rels/slide1.xml.rels", options)
                .unwrap();
            writer.write_all(rels.as_bytes()).unwrap();
        }

        writer.start_file("ppt/slides/slide1.xml", options).unwrap();
        writer.write_all(slide_xml.as_bytes()).unwrap();

        for (name, bytes) in media {
            writer.start_file(name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }

        writer.start_file("ppt/theme/theme1.xml", options).unwrap();
        writer.write_all(theme_xml().as_bytes()).unwrap();

        writer.start_file("customXml/item1.xml", options).unwrap();
        writer.write_all(manifest_xml().as_bytes()).unwrap();

        writer.finish().unwrap();
    }
    buf.into_inner()
}

fn image_content_types_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/customXml/item1.xml" ContentType="{CT_MANIFEST}"/>
</Types>"#
    )
}

fn image_slide_rels_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId3" Type="{REL_TYPE_IMAGE}" Target="../media/image1.png"/>
</Relationships>"#
    )
}

fn pic_xml(id: i64, name: &str, embed: &str) -> String {
    format!(
        r#"<p:pic>
  <p:nvPicPr>
    <p:cNvPr id="{id}" name="{name}"/>
    <p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>
    <p:nvPr/>
  </p:nvPicPr>
  <p:blipFill>
    <a:blip r:embed="{embed}"/>
    <a:srcRect l="10000" t="0" r="10000" b="0"/>
  </p:blipFill>
  <p:spPr>
    <a:xfrm rot="750000">
      <a:off x="1000000" y="500000"/>
      <a:ext cx="2000000" cy="1500000"/>
    </a:xfrm>
    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
  </p:spPr>
</p:pic>"#
    )
}

fn slide_wrapper(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="{P_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="0" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      {body}
    </p:spTree>
  </p:cSld>
</p:sld>"#
    )
}

fn image_slide_xml() -> String {
    slide_wrapper(&pic_xml(3, "Picture 1", "rId3"))
}

fn duplicate_image_slide_xml() -> String {
    slide_wrapper(&format!(
        "{}\n{}",
        pic_xml(3, "Picture 1", "rId3"),
        pic_xml(4, "Picture 2", "rId3")
    ))
}

fn dangling_image_slide_xml() -> String {
    slide_wrapper(&pic_xml(3, "Picture 1", "rId99"))
}

fn geometric_slide_xml() -> String {
    let body = r#"<p:sp>
  <p:nvSpPr>
    <p:cNvPr id="2" name="Oval 1"/>
    <p:cNvSpPr><a:spLocks/></p:cNvSpPr>
    <p:nvPr/>
  </p:nvSpPr>
  <p:spPr>
    <a:xfrm>
      <a:off x="100000" y="100000"/>
      <a:ext cx="914400" cy="914400"/>
    </a:xfrm>
    <a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>
    <a:solidFill><a:srgbClr val="4472C4"/></a:solidFill>
    <a:ln w="9525"><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:prstDash val="dash"/></a:ln>
    <a:effectLst><a:outerShdw blurRad="40000" dist="20000" dir="2700000" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="40000"/></a:srgbClr></a:outerShdw></a:effectLst>
  </p:spPr>
  <p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody>
</p:sp>
<p:sp>
  <p:nvSpPr>
    <p:cNvPr id="3" name="Rectangle 2"/>
    <p:cNvSpPr><a:spLocks/></p:cNvSpPr>
    <p:nvPr/>
  </p:nvSpPr>
  <p:spPr>
    <a:xfrm>
      <a:off x="200000" y="200000"/>
      <a:ext cx="1828800" cy="914400"/>
    </a:xfrm>
    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
  </p:spPr>
  <p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody>
</p:sp>
<p:sp>
  <p:nvSpPr>
    <p:cNvPr id="4" name="Rounded Rectangle 3"/>
    <p:cNvSpPr><a:spLocks/></p:cNvSpPr>
    <p:nvPr/>
  </p:nvSpPr>
  <p:spPr>
    <a:xfrm>
      <a:off x="300000" y="300000"/>
      <a:ext cx="1000000" cy="500000"/>
    </a:xfrm>
    <a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 25000"/></a:avLst></a:prstGeom>
  </p:spPr>
  <p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody>
</p:sp>
<p:sp>
  <p:nvSpPr>
    <p:cNvPr id="5" name="Triangle 4"/>
    <p:cNvSpPr><a:spLocks/></p:cNvSpPr>
    <p:nvPr/>
  </p:nvSpPr>
  <p:spPr>
    <a:xfrm>
      <a:off x="400000" y="400000"/>
      <a:ext cx="800000" cy="800000"/>
    </a:xfrm>
    <a:prstGeom prst="triangle"><a:avLst/></a:prstGeom>
  </p:spPr>
  <p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody>
</p:sp>
<p:sp>
  <p:nvSpPr>
    <p:cNvPr id="6" name="Line 5"/>
    <p:cNvSpPr><a:spLocks/></p:cNvSpPr>
    <p:nvPr/>
  </p:nvSpPr>
  <p:spPr>
    <a:xfrm>
      <a:off x="500000" y="500000"/>
      <a:ext cx="1200000" cy="0"/>
    </a:xfrm>
    <a:prstGeom prst="line"><a:avLst/></a:prstGeom>
  </p:spPr>
  <p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody>
</p:sp>"#;
    slide_wrapper(body)
}

fn build_pptx_with_image() -> Vec<u8> {
    let png = real_png(4, 4);
    build_pptx(
        &image_slide_xml(),
        Some(&image_slide_rels_xml()),
        &[("ppt/media/image1.png", png.as_slice())],
    )
}

fn build_pptx_with_duplicate_image() -> Vec<u8> {
    let png = real_png(4, 4);
    build_pptx(
        &duplicate_image_slide_xml(),
        Some(&image_slide_rels_xml()),
        &[("ppt/media/image1.png", png.as_slice())],
    )
}

fn build_pptx_with_dangling_image() -> Vec<u8> {
    build_pptx(&dangling_image_slide_xml(), None, &[])
}

fn build_pptx_with_geometric() -> Vec<u8> {
    build_pptx(&geometric_slide_xml(), None, &[])
}

#[test]
fn load_extracts_image_shape() {
    let original = build_pptx_with_image();
    let session = load(&original).expect("load should succeed");
    let slide = &session.deck().slides[0];
    assert_eq!(slide.shapes.len(), 1);

    let img = match &slide.shapes[0] {
        Shape::Image(i) => i,
        _ => panic!("expected an image shape"),
    };
    assert_eq!(
        img.transform.frame,
        Rect::new(1_000_000.0, 500_000.0, 2_000_000.0, 1_500_000.0)
    );
    assert!((img.transform.rotation - 12.5).abs() < 1e-9);
    let crop = img.crop.as_ref().expect("image should carry a crop");
    assert!((crop.left - 0.1).abs() < 1e-9);
    assert!((crop.right - 0.1).abs() < 1e-9);
    assert_eq!(crop.top, 0.0);
    assert_eq!(crop.bottom, 0.0);

    assert_eq!(session.deck().media.len(), 1);
    let entry = session
        .deck()
        .media
        .get(&img.media_ref)
        .expect("media entry should exist");
    assert_eq!(entry.mime, "image/png");
    assert_eq!(entry.width, 4);
    assert_eq!(entry.height, 4);
    assert!(entry.bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47]));
}

#[test]
fn load_dedups_identical_image_media() {
    let original = build_pptx_with_duplicate_image();
    let session = load(&original).expect("load should succeed");
    let slide = &session.deck().slides[0];
    assert_eq!(slide.shapes.len(), 2);
    let key_a = match &slide.shapes[0] {
        Shape::Image(i) => &i.media_ref,
        _ => panic!("first shape should be an image"),
    };
    let key_b = match &slide.shapes[1] {
        Shape::Image(i) => &i.media_ref,
        _ => panic!("second shape should be an image"),
    };
    assert_eq!(key_a, key_b, "identical media should dedup to one key");
    assert_eq!(session.deck().media.len(), 1);
}

#[test]
fn load_dangling_image_is_passthrough() {
    let original = build_pptx_with_dangling_image();
    let session = load(&original).expect("load should succeed");
    let slide = &session.deck().slides[0];
    assert!(
        matches!(slide.shapes[0], Shape::Passthrough(_)),
        "an image with no resolvable relationship must be preserved opaquely"
    );
    assert!(session
        .loss_ledger()
        .warnings()
        .iter()
        .any(|w| w.message.contains("pic")));
}

#[test]
fn load_extracts_geometric_shapes() {
    let original = build_pptx_with_geometric();
    let session = load(&original).expect("load should succeed");
    let shapes = &session.deck().slides[0].shapes;
    assert_eq!(shapes.len(), 5);
    assert!(
        shapes.iter().all(|s| matches!(s, Shape::Geometric(_))),
        "all fixture shapes should load as geometric"
    );

    let ellipse = match &shapes[0] {
        Shape::Geometric(g) => g,
        _ => panic!("ellipse"),
    };
    assert_eq!(ellipse.geometry, Geometry::Ellipse);
    assert_eq!(
        ellipse.transform.frame,
        Rect::new(100_000.0, 100_000.0, 914_400.0, 914_400.0)
    );
    let fill = match &ellipse.style.fill {
        Some(Fill::Solid(c)) => *c,
        _ => panic!("ellipse should have a solid fill"),
    };
    assert_eq!(fill, Color::rgb(0x44, 0x72, 0xC4));
    let outline = ellipse.style.outline.as_ref().expect("ellipse outline");
    assert_eq!(outline.color, Color::black());
    assert_eq!(outline.dash, DashStyle::Dash);
    let shadow = ellipse.style.shadow.as_ref().expect("ellipse shadow");
    assert_eq!(shadow.color, Color::black());
    assert!((shadow.opacity - 0.4).abs() < 1e-9);
    assert!((shadow.blur - 40_000.0).abs() < 1e-9);

    let rect = match &shapes[1] {
        Shape::Geometric(g) => g,
        _ => panic!("rectangle"),
    };
    assert_eq!(rect.geometry, Geometry::Rectangle);

    let round = match &shapes[2] {
        Shape::Geometric(g) => g,
        _ => panic!("rounded rectangle"),
    };
    match round.geometry {
        Geometry::RoundedRectangle { radius } => {
            // adj 25000 -> 0.25 of the smaller side (500000) = 125000.
            assert!((radius - 125_000.0).abs() < 1.0, "radius was {radius}");
        }
        _ => panic!("expected a rounded rectangle"),
    }

    let triangle = match &shapes[3] {
        Shape::Geometric(g) => g,
        _ => panic!("triangle"),
    };
    assert_eq!(triangle.geometry, Geometry::Triangle);

    let line = match &shapes[4] {
        Shape::Geometric(g) => g,
        _ => panic!("line"),
    };
    assert_eq!(line.geometry, Geometry::Line);
}

#[test]
fn round_trip_image_no_edit_is_stable() {
    let original = build_pptx_with_image();
    let session = load(&original).expect("load");
    let saved = save(&session).expect("save");

    // No edits: the media part is copied byte-for-byte.
    assert_eq!(
        entry_bytes(&original, "ppt/media/image1.png"),
        entry_bytes(&saved, "ppt/media/image1.png"),
        "untouched media must be byte-identical"
    );

    let again = load(&saved).expect("reload");
    let first = match &session.deck().slides[0].shapes[0] {
        Shape::Image(i) => i,
        _ => panic!("image"),
    };
    let second = match &again.deck().slides[0].shapes[0] {
        Shape::Image(i) => i,
        _ => panic!("image"),
    };
    assert_eq!(first.media_ref, second.media_ref);
    assert_eq!(first.transform, second.transform);
    assert_eq!(first.crop, second.crop);
    assert!(again.deck().media.contains_key(&second.media_ref));
}

#[test]
fn round_trip_move_image() {
    let original = build_pptx_with_image();
    let mut session = load(&original).expect("load");
    let slide_id = "ppt/slides/slide1.xml".to_string();
    let idx = session.deck().slides[0]
        .shapes
        .iter()
        .position(|s| matches!(s, Shape::Image(_)))
        .unwrap();
    let new_transform = Transform {
        frame: Rect::new(5_000_000.0, 3_000_000.0, 800_000.0, 600_000.0),
        rotation: 45.0,
    };
    session
        .execute(Box::new(MoveShape::new(
            slide_id.clone(),
            idx,
            new_transform,
        )))
        .expect("move");
    let saved = save(&session).expect("save");

    // The media part is untouched even though the slide was edited.
    assert_eq!(
        entry_bytes(&original, "ppt/media/image1.png"),
        entry_bytes(&saved, "ppt/media/image1.png")
    );

    let again = load(&saved).expect("reload");
    let img = match &again.deck().slides[0].shapes[idx] {
        Shape::Image(i) => i,
        _ => panic!("image"),
    };
    assert_eq!(img.transform, new_transform);
    let crop = img.crop.as_ref().expect("crop preserved");
    assert!((crop.left - 0.1).abs() < 1e-9);
    assert!(again.deck().media.contains_key(&img.media_ref));
}

#[test]
fn round_trip_move_geometric() {
    let original = build_pptx_with_geometric();
    let mut session = load(&original).expect("load");
    let slide_id = "ppt/slides/slide1.xml".to_string();
    let idx = 0; // ellipse
    let new_transform = Transform {
        frame: Rect::new(7_000_000.0, 4_000_000.0, 1_000_000.0, 1_000_000.0),
        rotation: 12.5,
    };
    session
        .execute(Box::new(MoveShape::new(
            slide_id.clone(),
            idx,
            new_transform,
        )))
        .expect("move");
    let saved = save(&session).expect("save");

    let again = load(&saved).expect("reload");
    let g = match &again.deck().slides[0].shapes[idx] {
        Shape::Geometric(g) => g,
        _ => panic!("geometric"),
    };
    assert_eq!(g.transform, new_transform);
    assert_eq!(g.geometry, Geometry::Ellipse);
}

#[test]
fn round_trip_change_geometric_style() {
    let original = build_pptx_with_geometric();
    let mut session = load(&original).expect("load");
    let slide_id = "ppt/slides/slide1.xml".to_string();
    let new_style = Style {
        fill: Some(Fill::Solid(Color::rgb(255, 128, 0))),
        outline: Some(Outline {
            color: Color::rgb(10, 20, 30),
            width_emu: 19050.0,
            dash: DashStyle::Dot,
        }),
        shadow: None,
    };
    session
        .execute(Box::new(SetShapeStyle::new(
            slide_id.clone(),
            0,
            new_style.clone(),
        )))
        .expect("set style");
    let saved = save(&session).expect("save");

    let again = load(&saved).expect("reload");
    let g = match &again.deck().slides[0].shapes[0] {
        Shape::Geometric(g) => g,
        _ => panic!("geometric"),
    };
    assert_eq!(g.style, new_style);
}

#[test]
fn insert_image_then_save_round_trips() {
    let blank = crate::create_blank_pptx();
    let mut session = load(&blank).expect("load blank");
    let slide_id = "ppt/slides/slide1.xml".to_string();
    let png = real_png(3, 5);
    let entry = MediaEntry {
        mime: "image/png".to_string(),
        bytes: png,
        width: 3,
        height: 5,
    };
    let transform = Transform {
        frame: Rect::new(1_000_000.0, 500_000.0, 2_000_000.0, 1_500_000.0),
        rotation: 0.0,
    };
    session
        .execute(Box::new(InsertImage::new(
            slide_id.clone(),
            "img-inserted".to_string(),
            entry,
            transform,
            None,
        )))
        .expect("insert image");

    let saved = save(&session).expect("save");
    let entries = zip_entries(&saved);
    assert!(
        entries.iter().any(|n| n.starts_with("ppt/media/image")),
        "a media part should be written, got {entries:?}"
    );
    assert!(entries.contains("ppt/slides/_rels/slide1.xml.rels"));
    let content_types = String::from_utf8(entry_bytes(&saved, "[Content_Types].xml")).unwrap();
    assert!(
        content_types.contains("Extension=\"png\""),
        "png content-type default should be registered"
    );

    let again = load(&saved).expect("reload");
    let slide = &again.deck().slides[0];
    let inserted = slide
        .shapes
        .iter()
        .find_map(|s| match s {
            Shape::Image(i) => Some(i),
            _ => None,
        })
        .expect("inserted image should reload");
    assert_eq!(inserted.transform.frame.width, 2_000_000.0);
    let entry = again
        .deck()
        .media
        .get(&inserted.media_ref)
        .expect("media entry should reload");
    assert_eq!(entry.mime, "image/png");
    assert_eq!(entry.width, 3);
}

#[test]
fn insert_geometric_then_save_round_trips() {
    let blank = crate::create_blank_pptx();
    let mut session = load(&blank).expect("load blank");
    let slide_id = "ppt/slides/slide1.xml".to_string();
    let geo = GeometricShape {
        transform: Transform {
            frame: Rect::new(100_000.0, 100_000.0, 914_400.0, 914_400.0),
            rotation: 0.0,
        },
        geometry: Geometry::Ellipse,
        style: Style {
            fill: Some(Fill::Solid(Color::rgb(255, 0, 0))),
            outline: None,
            shadow: None,
        },
    };
    session
        .execute(Box::new(AddShape::new(
            slide_id.clone(),
            Shape::Geometric(geo),
        )))
        .expect("add shape");

    let saved = save(&session).expect("save");
    let again = load(&saved).expect("reload");
    let g = again.deck().slides[0]
        .shapes
        .iter()
        .find_map(|s| match s {
            Shape::Geometric(g) => Some(g),
            _ => None,
        })
        .expect("inserted geometric should reload");
    assert_eq!(g.geometry, Geometry::Ellipse);
    assert_eq!(g.transform.frame.width, 914_400.0);
}

#[test]
fn delete_shape_is_persisted_on_save() {
    // The geometric fixture loads five geometric shapes. Deleting the middle
    // one must remove it from the saved PPTX (the positional patch path used
    // to misalign and leave the deleted element in place).
    let original = build_pptx_with_geometric();
    let mut session = load(&original).expect("load");
    let slide_id = "ppt/slides/slide1.xml".to_string();
    session
        .execute(Box::new(DeleteShape::new(slide_id.clone(), 2)))
        .expect("delete middle shape");
    assert_eq!(session.deck().slides[0].shapes.len(), 4);

    let saved = save(&session).expect("save");
    let again = load(&saved).expect("reload");
    let slide = &again.deck().slides[0];
    // The deleted shape is gone: 4 shapes remain, and reloading produced a
    // deck whose shape count matches the model after deletion.
    assert_eq!(
        slide.shapes.len(),
        4,
        "deleted shape must not be present in the saved PPTX"
    );
    // Every remaining shape is still geometric (no misalignment corruption).
    assert!(
        slide
            .shapes
            .iter()
            .all(|s| matches!(s, Shape::Geometric(_))),
        "remaining shapes must stay geometric, not be misaligned"
    );
}

#[test]
fn delete_shape_then_undo_round_trips() {
    let original = build_pptx_with_geometric();
    let mut session = load(&original).expect("load");
    let slide_id = "ppt/slides/slide1.xml".to_string();
    session
        .execute(Box::new(DeleteShape::new(slide_id.clone(), 0)))
        .expect("delete");
    assert!(session.undo(), "undo should restore the shape");

    // After undo the model has 5 shapes again; the patch path runs (equal
    // counts) and must keep all parts byte-identical except the slide.
    let saved = save(&session).expect("save");
    let again = load(&saved).expect("reload");
    assert_eq!(again.deck().slides[0].shapes.len(), 5);
}
