//! Tests for the PPTX load/save boundary.

use std::collections::HashSet;
use std::io::Write;

use slides_core::{Deck, EditText, ListStyle, Paragraph, Rect, Run, Shape, TextBox};
use zip::write::{FileOptions, ZipWriter};

use crate::{load, save};

const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_TYPE_MANIFEST: &str = "http://900labs.github.io/900Slides/1.0/relationships/manifest";
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
      <a:fontScheme name="Office">
        <a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont>
        <a:minorFont><a:latin typeface="Calibri"/></a:minorFont>
      </a:fontScheme>
    </a:clrScheme>
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
