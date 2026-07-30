//! Rich-text loading and saving tests for the PPTX boundary.

use std::io::Write;

use slides_core::{
    EditTextBox, Link, ListStyle, Paragraph, ParagraphStyle, Rect, Run, Shape, TextBox,
    VerticalAlign,
};
use zip::write::{FileOptions, ZipWriter};

use crate::{load, save};

const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_TYPE_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
const REL_TYPE_MANIFEST: &str = "http://900labs.github.io/900Slides/1.0/relationships/manifest";
const CT_MANIFEST: &str = "application/vnd.900labs.900slides.manifest+xml";

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

fn theme_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="{A_NS}" name="Office Theme">
  <a:themeElements>
    <a:clrScheme name="Office">
      <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
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

fn slide_rels_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId3" Type="{REL_TYPE_HYPERLINK}" Target="https://example.com" TargetMode="External"/>
  <Relationship Id="rId4" Type="{REL_TYPE_HYPERLINK}" Target="mailto:test@example.com" TargetMode="External"/>
</Relationships>"#
    )
}

fn rich_text_slide_xml() -> String {
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
            <a:pPr lvl="1"/>
            <a:r>
              <a:rPr b="1" i="1" u="sng" strike="sngStrike" baseline="30000">
                <a:hlinkClick r:id="rId3"/>
              </a:rPr>
              <a:t>Rich</a:t>
            </a:r>
            <a:r>
              <a:rPr baseline="-30000">
                <a:latin typeface="Consolas"/>
              </a:rPr>
              <a:t xml:space="preserve">run</a:t>
            </a:r>
          </a:p>
          <a:p>
            <a:r>
              <a:rPr/>
              <a:t>plain</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#
    )
}

fn build_rich_text_pptx() -> Vec<u8> {
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

        writer
            .start_file("ppt/slides/_rels/slide1.xml.rels", options)
            .unwrap();
        writer.write_all(slide_rels_xml().as_bytes()).unwrap();

        writer.start_file("ppt/slides/slide1.xml", options).unwrap();
        writer.write_all(rich_text_slide_xml().as_bytes()).unwrap();

        writer.start_file("ppt/theme/theme1.xml", options).unwrap();
        writer.write_all(theme_xml().as_bytes()).unwrap();

        writer.start_file("customXml/item1.xml", options).unwrap();
        writer.write_all(manifest_xml().as_bytes()).unwrap();

        writer.finish().unwrap();
    }
    buf.into_inner()
}

fn entry_bytes(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut out = Vec::new();
    std::io::copy(&mut file, &mut out).unwrap();
    out
}

#[test]
fn load_extracts_rich_text_run_properties() {
    let original = build_rich_text_pptx();
    let session = load(&original).expect("load should succeed");

    assert_eq!(session.deck().slides.len(), 1);
    let slide = &session.deck().slides[0];
    let text_box = match &slide.shapes[0] {
        Shape::TextBox(tb) => tb,
        _ => panic!("first shape should be a text box"),
    };
    assert_eq!(text_box.paragraphs.len(), 2);

    let p0 = &text_box.paragraphs[0];
    assert_eq!(p0.style.indent_level, 1);
    assert_eq!(p0.runs.len(), 2);

    let r0 = &p0.runs[0];
    assert_eq!(r0.text, "Rich");
    assert!(r0.bold);
    assert!(r0.italic);
    assert!(r0.underline);
    assert!(r0.strikethrough);
    assert_eq!(r0.vertical_align, VerticalAlign::Superscript);
    let link = r0.link.as_ref().expect("run should have a hyperlink");
    assert_eq!(link.url, "https://example.com");

    let r1 = &p0.runs[1];
    assert_eq!(r1.text, "run");
    assert_eq!(r1.vertical_align, VerticalAlign::Subscript);
    assert!(r1.code);
    assert_eq!(r1.font_family.as_deref(), Some("Consolas"));

    let p1 = &text_box.paragraphs[1];
    assert_eq!(p1.style.indent_level, 0);
    assert_eq!(p1.runs.len(), 1);
    assert_eq!(p1.runs[0].text, "plain");
    assert!(!p1.runs[0].bold);
}

#[test]
fn load_records_hyperlink_allowlist_warning() {
    let original = build_rich_text_pptx();
    let session = load(&original).expect("load should succeed");

    // https://example.com is now an allowed scheme (http/https are safe for
    // presentation links), so no warning should be generated for it.
    assert!(!session
        .loss_ledger()
        .warnings()
        .iter()
        .any(|w| w.message.contains("https://example.com")));
}

#[test]
fn round_trip_rich_text() {
    let original = build_rich_text_pptx();
    let mut session = load(&original).expect("load should succeed");

    let slide_id = "ppt/slides/slide1.xml".to_string();
    let mut text_box = TextBox {
        id: String::new(),
        frame: Rect::new(0.0, 0.0, 100.0, 100.0),
        paragraphs: vec![
            Paragraph {
                runs: vec![
                    Run::new("Bold").bold(),
                    Run::new("Italic").italic(),
                    Run::new("Underline").underline(),
                    Run::new("Strike").strikethrough(),
                    Run::new("Super").superscript(),
                    Run::new("Sub").subscript(),
                    Run::new("Code").code().font("Consolas"),
                ],
                list_style: ListStyle::Unordered,
                style: ParagraphStyle {
                    indent_level: 2,
                    ..Default::default()
                },
            },
            Paragraph {
                runs: vec![Run {
                    text: "Link".to_string(),
                    link: Some(Link::new("mailto:test@example.com").expect("allowed link")),
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
    };
    text_box.frame = match &session.deck().slides[0].shapes[0] {
        Shape::TextBox(tb) => tb.frame,
        _ => text_box.frame,
    };

    session
        .execute(Box::new(EditTextBox::new(
            slide_id.clone(),
            0,
            text_box.paragraphs,
        )))
        .expect("edit should apply");

    let saved = save(&session).expect("save should succeed");
    let saved_slide = String::from_utf8(entry_bytes(&saved, "ppt/slides/slide1.xml")).unwrap();

    assert!(saved_slide.contains("Bold"));
    assert!(saved_slide.contains("Italic"));
    assert!(saved_slide.contains("Underline"));
    assert!(saved_slide.contains("Strike"));
    assert!(saved_slide.contains("Super"));
    assert!(saved_slide.contains("Sub"));
    assert!(saved_slide.contains("Code"));
    assert!(saved_slide.contains("lvl=\"2\""));
    assert!(saved_slide.contains("strike=\"sngStrike\""));
    assert!(saved_slide.contains("baseline=\"30000\""));
    assert!(saved_slide.contains("baseline=\"-30000\""));
    assert!(saved_slide.contains("typeface=\"Consolas\""));

    let saved_rels =
        String::from_utf8(entry_bytes(&saved, "ppt/slides/_rels/slide1.xml.rels")).unwrap();
    assert!(saved_rels.contains(REL_TYPE_HYPERLINK));
    assert!(saved_rels.contains("mailto:test@example.com"));

    let again = load(&saved).expect("reload should succeed");
    let reloaded = match &again.deck().slides[0].shapes[0] {
        Shape::TextBox(tb) => tb,
        _ => panic!("shape should still be a text box"),
    };
    assert_eq!(reloaded.paragraphs.len(), 2);
    let p0 = &reloaded.paragraphs[0];
    assert!(p0.runs.iter().any(|r| r.text == "Bold" && r.bold));
    assert!(p0.runs.iter().any(|r| r.text == "Italic" && r.italic));
    assert!(p0.runs.iter().any(|r| r.text == "Underline" && r.underline));
    assert!(p0
        .runs
        .iter()
        .any(|r| r.text == "Strike" && r.strikethrough));
    assert!(p0
        .runs
        .iter()
        .any(|r| r.text == "Super" && r.vertical_align == VerticalAlign::Superscript));
    assert!(p0
        .runs
        .iter()
        .any(|r| r.text == "Sub" && r.vertical_align == VerticalAlign::Subscript));
    assert!(p0.runs.iter().any(|r| r.text == "Code" && r.code));
    assert_eq!(p0.style.indent_level, 2);
    assert_eq!(p0.list_style, ListStyle::Unordered);

    let p1 = &reloaded.paragraphs[1];
    assert_eq!(p1.runs.len(), 1);
    assert_eq!(p1.runs[0].text, "Link");
    assert_eq!(
        p1.runs[0].link.as_ref().map(|l| l.url.as_str()),
        Some("mailto:test@example.com")
    );
}

#[test]
fn round_trip_unsafe_hyperlink_is_preserved() {
    let original = build_rich_text_pptx();
    let mut session = load(&original).expect("load should succeed");

    let slide_id = "ppt/slides/slide1.xml".to_string();
    let mut paragraphs = match &session.deck().slides[0].shapes[0] {
        Shape::TextBox(tb) => tb.paragraphs.clone(),
        _ => panic!("expected text box"),
    };
    paragraphs[0].runs[0].link = Some(Link::new_unchecked("https://example.com"));

    session
        .execute(Box::new(EditTextBox::new(slide_id.clone(), 0, paragraphs)))
        .expect("edit should apply");

    let saved = save(&session).expect("save should succeed");
    let saved_rels =
        String::from_utf8(entry_bytes(&saved, "ppt/slides/_rels/slide1.xml.rels")).unwrap();
    assert!(saved_rels.contains("https://example.com"));

    let again = load(&saved).expect("reload should succeed");
    let reloaded = match &again.deck().slides[0].shapes[0] {
        Shape::TextBox(tb) => tb,
        _ => panic!("expected text box"),
    };
    assert_eq!(
        reloaded.paragraphs[0].runs[0]
            .link
            .as_ref()
            .map(|l| l.url.as_str()),
        Some("https://example.com")
    );
}
