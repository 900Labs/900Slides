//! Tests for PPTX table load/save (Wave 3, components 3 & 4).

use std::collections::HashSet;
use std::io::Write;

use slides_core::{
    BorderEdge, CellAlign, Color, DashStyle, Fill, Rect, ResizeTable, SetCellText, Shape,
    TableBorders,
};
use zip::write::{FileOptions, ZipWriter};

use crate::{load, save};

const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_TYPE_MANIFEST: &str = "http://900labs.github.io/900Slides/1.0/relationships/manifest";
const CT_MANIFEST: &str = "application/vnd.900labs.900slides.manifest+xml";

const COL_W: u32 = 2_133_600;
const ROW_H: u32 = 609_600;

/// Builds a PPTX package with one slide whose `<p:spTree>` body is `body`.
fn build_single_slide(body: &str) -> Vec<u8> {
    build_package(&[slide_wrapper(body)])
}

/// Builds a PPTX package with two slides carrying the given spTree bodies.
fn build_two_slides(body1: &str, body2: &str) -> Vec<u8> {
    build_package(&[slide_wrapper(body1), slide_wrapper(body2)])
}

fn build_package(slide_xmls: &[String]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut buf);
        let options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        writer.start_file("[Content_Types].xml", options).unwrap();
        writer
            .write_all(content_types_xml(slide_xmls.len()).as_bytes())
            .unwrap();

        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(package_rels_xml().as_bytes()).unwrap();

        writer.start_file("ppt/presentation.xml", options).unwrap();
        writer
            .write_all(presentation_xml(slide_xmls.len()).as_bytes())
            .unwrap();

        writer
            .start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        writer
            .write_all(presentation_rels_xml(slide_xmls.len()).as_bytes())
            .unwrap();

        for (i, xml) in slide_xmls.iter().enumerate() {
            let n = i + 1;
            writer
                .start_file(format!("ppt/slides/slide{n}.xml"), options)
                .unwrap();
            writer.write_all(xml.as_bytes()).unwrap();
        }

        writer.start_file("ppt/theme/theme1.xml", options).unwrap();
        writer.write_all(theme_xml().as_bytes()).unwrap();

        writer.start_file("customXml/item1.xml", options).unwrap();
        writer.write_all(manifest_xml().as_bytes()).unwrap();

        writer.finish().unwrap();
    }
    buf.into_inner()
}

fn content_types_xml(slides: usize) -> String {
    let mut overrides = String::new();
    for n in 1..=slides {
        overrides.push_str(&format!(
            "<Override PartName=\"/ppt/slides/slide{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  {overrides}
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

fn presentation_rels_xml(slides: usize) -> String {
    let mut rels = String::new();
    for n in 1..=slides {
        rels.push_str(&format!(
            "<Relationship Id=\"rId{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{n}.xml\"/>"
        ));
    }
    rels.push_str(
        "<Relationship Id=\"rIdTheme\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"theme/theme1.xml\"/>",
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {rels}
</Relationships>"#
    )
}

fn presentation_xml(slides: usize) -> String {
    let mut ids = String::new();
    for n in 1..=slides {
        ids.push_str(&format!(
            "<p:sldId id=\"{}\" r:id=\"rId{n}\"/>",
            256 + n - 1
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="{P_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">
  <p:sldIdLst>
    {ids}
  </p:sldIdLst>
</p:presentation>"#
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

/// A header cell (centered, blue fill).
fn header_tc(text: &str) -> String {
    format!(
        r#"<a:tc>
  <a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="en-US"/><a:t>{text}</a:t></a:r></a:p></a:txBody>
  <a:tcPr><a:solidFill><a:srgbClr val="4472C4"/></a:solidFill></a:tcPr>
</a:tc>"#
    )
}

/// A plain cell (left aligned by default), with optional fill hex.
fn plain_tc(text: &str, fill_hex: Option<&str>, algn: Option<&str>) -> String {
    let ppr = match algn {
        Some(a) => format!("<a:pPr algn=\"{a}\"/>"),
        None => String::new(),
    };
    let fill = match fill_hex {
        Some(hex) => format!("<a:solidFill><a:srgbClr val=\"{hex}\"/></a:solidFill>"),
        None => String::new(),
    };
    format!(
        r#"<a:tc>
  <a:txBody><a:bodyPr/><a:lstStyle/><a:p>{ppr}<a:r><a:rPr lang="en-US"/><a:t>{text}</a:t></a:r></a:p></a:txBody>
  <a:tcPr>{fill}</a:tcPr>
</a:tc>"#
    )
}

/// Builds the body of a 3x3 table graphicFrame fixture.
fn table_body() -> String {
    let mut grid = String::new();
    for _ in 0..3 {
        grid.push_str(&format!("<a:gridCol w=\"{COL_W}\"/>"));
    }

    let row0 = format!(
        "<a:tr h=\"{ROW_H}\">{}{}{}</a:tr>",
        header_tc("Name"),
        header_tc("Value"),
        header_tc("Note"),
    );
    let row1 = format!(
        "<a:tr h=\"{ROW_H}\">{}{}{}</a:tr>",
        plain_tc("Alpha", None, None),
        plain_tc("1", Some("FFE699"), None),
        plain_tc("x", None, Some("r")),
    );
    // Row 2, cell 0 carries a left red dashed edge and a bottom green edge.
    let bordered = r#"<a:tc>
  <a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>Beta</a:t></a:r></a:p></a:txBody>
  <a:tcPr><a:lnL w="9525"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:prstDash val="dash"/></a:lnL><a:lnB w="9525"><a:solidFill><a:srgbClr val="00B050"/></a:solidFill></a:lnB></a:tcPr>
</a:tc>"#
        .to_string();
    let row2 = format!(
        "<a:tr h=\"{ROW_H}\">{}{}{}</a:tr>",
        bordered,
        plain_tc("2", None, None),
        plain_tc("y", None, None),
    );

    format!(
        r#"<p:graphicFrame>
  <p:nvGraphicFramePr><p:cNvPr id="7" name="Table 1"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>
  <p:xfrm><a:off x="914400" y="914400"/><a:ext cx="6400800" cy="1828800"/></p:xfrm>
  <a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table">
    <a:tbl>
      <a:tblPr firstRow="1" bandRow="1"/>
      <a:tblGrid>{grid}</a:tblGrid>
      {row0}{row1}{row2}
    </a:tbl>
  </a:graphicData></a:graphic>
</p:graphicFrame>"#
    )
}

fn build_table_pptx() -> Vec<u8> {
    build_single_slide(&table_body())
}

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------

#[test]
fn load_extracts_table_shape() {
    let original = build_table_pptx();
    let session = load(&original).expect("load should succeed");
    let slide = &session.deck().slides[0];
    assert_eq!(slide.shapes.len(), 1);

    let table = match &slide.shapes[0] {
        Shape::Table(t) => t,
        _ => panic!("expected a table shape, got {:?}", slide.shapes[0]),
    };
    assert!(table.header_row, "header_row should be detected");
    assert_eq!(
        table.column_widths,
        vec![COL_W as f64, COL_W as f64, COL_W as f64]
    );
    assert_eq!(table.row_count(), 3);
    assert_eq!(table.col_count(), 3);
    assert_eq!(table.rows[0].height, ROW_H as f64);

    // Header row: centered, blue fill.
    assert_eq!(table.cell(0, 0).unwrap().text, "Name");
    assert_eq!(table.cell(0, 0).unwrap().align, CellAlign::Center);
    assert_eq!(
        table.cell(0, 0).unwrap().fill,
        Some(Fill::Solid(Color::rgb(0x44, 0x72, 0xC4)))
    );

    // (1,0) plain left-aligned, no fill.
    let c = table.cell(1, 0).unwrap();
    assert_eq!(c.text, "Alpha");
    assert_eq!(c.align, CellAlign::Left);
    assert!(c.fill.is_none());
    assert!(c.borders.is_none());

    // (1,1) yellow fill.
    assert_eq!(
        table.cell(1, 1).unwrap().fill,
        Some(Fill::Solid(Color::rgb(0xFF, 0xE6, 0x99)))
    );

    // (1,2) right aligned.
    assert_eq!(table.cell(1, 2).unwrap().align, CellAlign::Right);
    assert_eq!(table.cell(1, 2).unwrap().text, "x");

    // (2,0) border overrides.
    let c = table.cell(2, 0).unwrap();
    assert_eq!(c.text, "Beta");
    let borders = c.borders.as_ref().expect("cell should carry borders");
    let left = borders.left.as_ref().expect("left edge");
    assert_eq!(left.color, Color::rgb(0xFF, 0x00, 0x00));
    assert_eq!(left.dash, DashStyle::Dash);
    assert!((left.width_emu - 9525.0).abs() < 1e-6);
    let bottom = borders.bottom.as_ref().expect("bottom edge");
    assert_eq!(bottom.color, Color::rgb(0x00, 0xB0, 0x50));
    assert_eq!(bottom.dash, DashStyle::Solid);
    assert!(borders.top.is_none());
    assert!(borders.right.is_none());

    // (2,1) and (2,2) have no borders -> inherit.
    assert!(table.cell(2, 1).unwrap().borders.is_none());
    assert!(table.cell(2, 2).unwrap().borders.is_none());

    // Transform.
    assert_eq!(
        table.transform.frame,
        Rect::new(914_400.0, 914_400.0, 6_400_800.0, 1_828_800.0)
    );
}

#[test]
fn load_clamps_oversized_table_and_warns() {
    // A 51-row, 1-column table exceeds the 50-row cap.
    let mut rows = String::new();
    for i in 0..51 {
        rows.push_str(&format!(
            "<a:tr h=\"100000\"><a:tc><a:txBody><a:bodyPr/><a:lstStyle/>\
             <a:p><a:r><a:t>r{i}</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc></a:tr>"
        ));
    }
    let body = format!(
        r#"<p:graphicFrame>
  <p:nvGraphicFramePr><p:cNvPr id="7" name="Big"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>
  <p:xfrm><a:off x="0" y="0"/><a:ext cx="1000000" cy="5000000"/></p:xfrm>
  <a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table">
    <a:tbl><a:tblPr/><a:tblGrid><a:gridCol w="1000000"/></a:tblGrid>{rows}</a:tbl>
  </a:graphicData></a:graphic>
</p:graphicFrame>"#
    );

    let pptx = build_single_slide(&body);
    let session = load(&pptx).expect("load should succeed");
    let slide = &session.deck().slides[0];
    let table = match &slide.shapes[0] {
        Shape::Table(t) => t,
        _ => panic!("oversized table should still load as a table (clamped)"),
    };
    assert_eq!(table.row_count(), 50, "rows should be clamped to 50");
    assert_eq!(table.cell(49, 0).unwrap().text, "r49");
    assert!(session
        .loss_ledger()
        .warnings()
        .iter()
        .any(|w| w.message.contains("50x50") && w.message.contains("truncated")));
}

#[test]
fn load_chart_graphic_frame_stays_passthrough() {
    // A graphicFrame whose graphicData is a chart (no a:tbl) must be preserved
    // opaquely, not modeled as a table.
    let body = r#"<p:graphicFrame>
  <p:nvGraphicFramePr><p:cNvPr id="8" name="Chart 1"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>
  <p:xfrm><a:off x="0" y="0"/><a:ext cx="1000000" cy="1000000"/></p:xfrm>
  <a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">
    <c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"/>
  </a:graphicData></a:graphic>
</p:graphicFrame>"#;
    let pptx = build_single_slide(body);
    let session = load(&pptx).expect("load should succeed");
    let slide = &session.deck().slides[0];
    match &slide.shapes[0] {
        Shape::Passthrough(obj) => {
            assert_eq!(obj.label, "graphicFrame");
            let s = String::from_utf8_lossy(&obj.raw_bytes);
            assert!(s.contains("chart"), "chart graphicData should survive");
            // It must NOT be modeled as a table: no <a:tbl element.
            assert!(
                !s.contains("<a:tbl"),
                "chart frame should not contain a table element"
            );
        }
        _ => panic!("chart graphicFrame should be preserved as passthrough"),
    }
}

#[test]
fn load_rich_text_cell_collapses_with_warning() {
    let body = r#"<p:graphicFrame>
  <p:nvGraphicFramePr><p:cNvPr id="9" name="Rich"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>
  <p:xfrm><a:off x="0" y="0"/><a:ext cx="1000000" cy="1000000"/></p:xfrm>
  <a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table">
    <a:tbl><a:tblPr/><a:tblGrid><a:gridCol w="1000000"/></a:tblGrid>
      <a:tr h="100000"><a:tc>
        <a:txBody><a:bodyPr/><a:lstStyle/><a:p>
          <a:r><a:rPr b="1" i="1"/><a:t>Bold italic text</a:t></a:r>
        </a:p></a:txBody>
        <a:tcPr/>
      </a:tc></a:tr>
    </a:tbl>
  </a:graphicData></a:graphic>
</p:graphicFrame>"#;
    let pptx = build_single_slide(body);
    let session = load(&pptx).expect("load should succeed");
    let slide = &session.deck().slides[0];
    let table = match &slide.shapes[0] {
        Shape::Table(t) => t,
        _ => panic!("expected a table"),
    };
    // Text is preserved (collapsed to plain text).
    assert_eq!(table.cell(0, 0).unwrap().text, "Bold italic text");
    // A loss warning was recorded.
    assert!(session
        .loss_ledger()
        .warnings()
        .iter()
        .any(|w| w.message.contains("rich text collapsed")));
}

// ---------------------------------------------------------------------------
// Save round-trip
// ---------------------------------------------------------------------------

#[test]
fn save_edits_cell_text_and_column_widths() {
    let original = build_table_pptx();
    let mut session = load(&original).expect("load should succeed");
    let slide_id = "ppt/slides/slide1.xml".to_string();

    session
        .execute(Box::new(SetCellText::new(
            slide_id.clone(),
            0,
            0,
            0,
            "Renamed",
        )))
        .expect("set cell text");

    let table = match &session.deck().slides[0].shapes[0] {
        Shape::Table(t) => t,
        _ => panic!("table"),
    };
    let new_widths: Vec<f64> = table.column_widths.iter().map(|w| w + 100_000.0).collect();
    let new_heights: Vec<f64> = table.rows.iter().map(|r| r.height).collect();
    session
        .execute(Box::new(ResizeTable::new(
            slide_id.clone(),
            0,
            new_widths.clone(),
            new_heights,
        )))
        .expect("resize table");

    let saved = save(&session).expect("save should succeed");
    let again = load(&saved).expect("reload should succeed");
    let table = match &again.deck().slides[0].shapes[0] {
        Shape::Table(t) => t,
        _ => panic!("table after reload"),
    };
    assert_eq!(table.cell(0, 0).unwrap().text, "Renamed");
    assert_eq!(table.column_widths, new_widths);
    // Untouched cells survive.
    assert_eq!(table.cell(1, 1).unwrap().text, "1");
    assert_eq!(
        table.cell(0, 1).unwrap().fill,
        Some(Fill::Solid(Color::rgb(0x44, 0x72, 0xC4)))
    );
}

// ---------------------------------------------------------------------------
// Lossless passthrough (§4.9)
// ---------------------------------------------------------------------------

#[test]
fn untouched_table_slide_is_byte_identical() {
    // Slide 1 carries a table; slide 2 carries a plain text box. Editing slide
    // 2's text must leave slide 1 (and every other part) byte-for-byte identical.
    let original = build_two_slides(&table_body(), &text_box_body("Original"));

    let mut session = load(&original).expect("load should succeed");
    let slide2 = "ppt/slides/slide2.xml".to_string();
    session
        .execute(Box::new(slides_core::EditText::new(
            slide2.clone(),
            0,
            0,
            vec![slides_core::Run::new("Edited")],
        )))
        .expect("edit slide 2");

    let saved = save(&session).expect("save should succeed");

    // Every part except slide2 (and the regenerated manifest/content-types)
    // must be byte-identical. Crucially, the table slide1 is untouched.
    for name in zip_entries(&original) {
        if name == "ppt/slides/slide2.xml"
            || name == "[Content_Types].xml"
            || name == "customXml/item1.xml"
        {
            continue;
        }
        assert_eq!(
            entry_bytes(&original, &name),
            entry_bytes(&saved, &name),
            "{name} should be byte-identical (§4.9)"
        );
    }

    // slide2 changed.
    let edited = String::from_utf8(entry_bytes(&saved, "ppt/slides/slide2.xml")).unwrap();
    assert!(edited.contains("Edited"));
}

#[test]
fn save_no_edit_keeps_table_slide_byte_identical() {
    // Loading and saving without any edit must reproduce the package (except
    // the manifest/content-types) byte-for-byte.
    let original = build_table_pptx();
    let session = load(&original).expect("load should succeed");
    let saved = save(&session).expect("save should succeed");

    for name in zip_entries(&original) {
        if name == "[Content_Types].xml" || name == "customXml/item1.xml" {
            continue;
        }
        assert_eq!(
            entry_bytes(&original, &name),
            entry_bytes(&saved, &name),
            "{name} should be byte-identical with no edits"
        );
    }
}

fn text_box_body(text: &str) -> String {
    format!(
        r#"<p:sp>
  <p:nvSpPr><p:cNvPr id="2" name="TextBox 1"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
  <p:spPr><a:xfrm><a:off x="914400" y="457200"/><a:ext cx="4572000" cy="762000"/></a:xfrm></p:spPr>
  <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:txBody>
</p:sp>"#
    )
}

#[test]
fn inserted_table_round_trips() {
    // Inserting a brand-new table onto a blank slide and saving must reload it
    // as a table with the same structure.
    let blank = crate::create_blank_pptx();
    let mut session = load(&blank).expect("load blank");
    let slide_id = "ppt/slides/slide1.xml".to_string();

    let mut table = slides_core::TableShape::default_grid(
        2,
        2,
        Rect::new(1_000_000.0, 1_000_000.0, 4_000_000.0, 1_000_000.0),
    );
    table.header_row = true;
    table.cell_mut(0, 0).unwrap().text = "A".to_string();
    table.cell_mut(0, 0).unwrap().align = CellAlign::Center;
    table.cell_mut(1, 1).unwrap().borders = Some(TableBorders {
        top: Some(BorderEdge {
            color: Color::rgb(0, 0, 0),
            width_emu: 9525.0,
            dash: DashStyle::Dot,
        }),
        bottom: None,
        left: None,
        right: None,
    });

    session
        .execute(Box::new(slides_core::AddTable::new(
            slide_id.clone(),
            table,
        )))
        .expect("add table");

    let saved = save(&session).expect("save");
    let again = load(&saved).expect("reload");
    let tbl = match &again.deck().slides[0]
        .shapes
        .iter()
        .find(|s| matches!(s, Shape::Table(_)))
        .unwrap()
    {
        Shape::Table(t) => t,
        _ => unreachable!(),
    };
    assert_eq!(tbl.row_count(), 2);
    assert_eq!(tbl.col_count(), 2);
    assert!(tbl.header_row);
    assert_eq!(tbl.cell(0, 0).unwrap().text, "A");
    assert_eq!(tbl.cell(0, 0).unwrap().align, CellAlign::Center);
    let top = tbl
        .cell(1, 1)
        .unwrap()
        .borders
        .as_ref()
        .unwrap()
        .top
        .as_ref();
    assert_eq!(top.unwrap().dash, DashStyle::Dot);
}
