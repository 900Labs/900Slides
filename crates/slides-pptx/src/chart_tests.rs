//! Tests for PPTX chart load/save (Wave 4, components 4 & 5).

use std::collections::HashSet;
use std::io::Write;

use slides_core::{
    AddChart, ChartData, ChartShape, ChartType, DeleteShape, MoveShape, Rect, SetChartData,
    SetChartTitle, SetChartType, Shape, Transform,
};
use zip::write::{FileOptions, ZipWriter};

use crate::{load, save};

const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const C_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const REL_TYPE_MANIFEST: &str = "http://900labs.github.io/900Slides/1.0/relationships/manifest";
const REL_TYPE_CHART: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const CT_MANIFEST: &str = "application/vnd.900labs.900slides.manifest+xml";
const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";

/// Builds a minimal PPTX package with a single slide containing a chart.
fn build_chart_pptx(chart_xml: &str) -> Vec<u8> {
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

        writer
            .start_file("ppt/slides/_rels/slide1.xml.rels", options)
            .unwrap();
        writer.write_all(slide1_rels_xml().as_bytes()).unwrap();

        writer.start_file("ppt/charts/chart1.xml", options).unwrap();
        writer.write_all(chart_xml.as_bytes()).unwrap();

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
  <Override PartName="/ppt/charts/chart1.xml" ContentType="{CT_CHART}"/>
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
  <Relationship Id="rIdTheme" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
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
      <p:graphicFrame>
        <p:nvGraphicFramePr>
          <p:cNvPr id="7" name="Chart 1"/>
          <p:cNvGraphicFramePr/>
          <p:nvPr/>
        </p:nvGraphicFramePr>
        <p:xfrm>
          <a:off x="914400" y="914400"/>
          <a:ext cx="6400800" cy="1828800"/>
        </p:xfrm>
        <a:graphic>
          <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">
            <c:chart xmlns:c="{C_NS}" r:id="rIdChart1"/>
          </a:graphicData>
        </a:graphic>
      </p:graphicFrame>
    </p:spTree>
  </p:cSld>
</p:sld>"#
    )
}

fn slide1_rels_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdChart1" Type="{REL_TYPE_CHART}" Target="../charts/chart1.xml"/>
</Relationships>"#
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

/// A bar chart with 3 categories and 2 series, including a title.
fn sample_bar_chart_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:chartSpace xmlns:c="{C_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">
  <c:chart>
    <c:title>
      <c:tx>
        <c:rich>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p>
            <a:r><a:rPr/><a:t>Quarterly Sales</a:t></a:r>
          </a:p>
        </c:rich>
      </c:tx>
      <c:overlay val="0"/>
    </c:title>
    <c:plotArea>
      <c:barChart>
        <c:barDir val="bar"/>
        <c:grouping val="clustered"/>
        <c:ser>
          <c:idx val="0"/>
          <c:order val="0"/>
          <c:tx>
            <c:strRef>
              <c:f>Sheet1!$B$1</c:f>
              <c:strCache>
                <c:ptCount val="1"/>
                <c:pt idx="0"><c:v>Product A</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:tx>
          <c:cat>
            <c:strRef>
              <c:f>Sheet1!$A$2:$A$4</c:f>
              <c:strCache>
                <c:ptCount val="3"/>
                <c:pt idx="0"><c:v>Q1</c:v></c:pt>
                <c:pt idx="1"><c:v>Q2</c:v></c:pt>
                <c:pt idx="2"><c:v>Q3</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:cat>
          <c:val>
            <c:numRef>
              <c:f>Sheet1!$B$2:$B$4</c:f>
              <c:numCache>
                <c:ptCount val="3"/>
                <c:pt idx="0"><c:v>10</c:v></c:pt>
                <c:pt idx="1"><c:v>20</c:v></c:pt>
                <c:pt idx="2"><c:v>30</c:v></c:pt>
              </c:numCache>
            </c:numRef>
          </c:val>
        </c:ser>
        <c:ser>
          <c:idx val="1"/>
          <c:order val="1"/>
          <c:tx>
            <c:strRef>
              <c:f>Sheet1!$C$1</c:f>
              <c:strCache>
                <c:ptCount val="1"/>
                <c:pt idx="0"><c:v>Product B</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:tx>
          <c:cat>
            <c:strRef>
              <c:f>Sheet1!$A$2:$A$4</c:f>
              <c:strCache>
                <c:ptCount val="3"/>
                <c:pt idx="0"><c:v>Q1</c:v></c:pt>
                <c:pt idx="1"><c:v>Q2</c:v></c:pt>
                <c:pt idx="2"><c:v>Q3</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:cat>
          <c:val>
            <c:numRef>
              <c:f>Sheet1!$C$2:$C$4</c:f>
              <c:numCache>
                <c:ptCount val="3"/>
                <c:pt idx="0"><c:v>15</c:v></c:pt>
                <c:pt idx="1"><c:v>25</c:v></c:pt>
                <c:pt idx="2"><c:v>35</c:v></c:pt>
              </c:numCache>
            </c:numRef>
          </c:val>
        </c:ser>
        <c:axId val="1"/>
        <c:axId val="2"/>
      </c:barChart>
      <c:catAx>
        <c:axId val="1"/>
        <c:scaling><c:orientation val="minMax"/></c:scaling>
        <c:delete val="0"/>
        <c:axPos val="l"/>
        <c:crossAx val="2"/>
      </c:catAx>
      <c:valAx>
        <c:axId val="2"/>
        <c:scaling><c:orientation val="minMax"/></c:scaling>
        <c:delete val="0"/>
        <c:axPos val="b"/>
        <c:crossAx val="1"/>
      </c:valAx>
    </c:plotArea>
    <c:plotVisOnly val="1"/>
  </c:chart>
</c:chartSpace>"#
    )
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

fn build_two_chart_pptx() -> Vec<u8> {
    let first = build_chart_pptx(&sample_bar_chart_xml());
    let mut parts: Vec<(String, Vec<u8>)> = zip_entries(&first)
        .into_iter()
        .map(|name| {
            let bytes = entry_bytes(&first, &name);
            (name, bytes)
        })
        .collect();
    for (name, bytes) in &mut parts {
        let xml = String::from_utf8(bytes.clone()).unwrap();
        let xml = match name.as_str() {
            "ppt/slides/slide1.xml" => {
                let start = xml.find("<p:graphicFrame>").unwrap();
                let end = xml.find("</p:graphicFrame>").unwrap() + "</p:graphicFrame>".len();
                let second_frame = xml[start..end]
                    .replace("id=\"7\"", "id=\"8\"")
                    .replace("Chart 1", "Chart 2")
                    .replace("rIdChart1", "rIdChart2");
                xml.replace("</p:spTree>", &format!("{second_frame}</p:spTree>"))
            }
            "ppt/slides/_rels/slide1.xml.rels" => xml.replace(
                "</Relationships>",
                &format!(
                    r#"<Relationship Id="rIdChart2" Type="{REL_TYPE_CHART}" Target="../charts/chart2.xml"/></Relationships>"#
                ),
            ),
            "[Content_Types].xml" => xml.replace(
                "</Types>",
                &format!(
                    r#"<Override PartName="/ppt/charts/chart2.xml" ContentType="{CT_CHART}"/></Types>"#
                ),
            ),
            _ => xml,
        };
        *bytes = xml.into_bytes();
    }
    let second_chart = sample_bar_chart_xml()
        .replace("Quarterly Sales", "Regional Revenue")
        .replace("</c:chartSpace>", r#"<c:extLst><c:ext uri="preserved-second-chart"><custom:marker xmlns:custom="urn:chart-test">opaque second chart metadata</custom:marker></c:ext></c:extLst></c:chartSpace>"#);
    parts.push((
        "ppt/charts/chart2.xml".to_string(),
        second_chart.into_bytes(),
    ));
    let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        writer
            .start_file(name, FileOptions::<()>::default())
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn slide_organization_session() -> crate::Session {
    let original = build_two_chart_pptx();
    let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in zip_entries(&original) {
        let bytes = entry_bytes(&original, &name);
        let xml = String::from_utf8(bytes).unwrap();
        let xml = match name.as_str() {
            "ppt/slides/slide1.xml" => xml.replace("</p:cSld>", r#"<p:extLst><p:ext uri="opaque-slide-payload"><custom:data xmlns:custom="urn:900slides:test">Keep this source payload</custom:data></p:ext></p:extLst></p:cSld>"#),
            "ppt/slides/_rels/slide1.xml.rels" => xml.replace("</Relationships>", r#"<Relationship Id="rIdNotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide1.xml"/><Relationship Id="rIdLayout" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#),
            "[Content_Types].xml" => xml.replace("</Types>", r#"<Override PartName="/ppt/notesSlides/notesSlide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/></Types>"#),
            _ => xml,
        };
        writer
            .start_file(name, FileOptions::<()>::default())
            .unwrap();
        writer.write_all(xml.as_bytes()).unwrap();
    }
    writer
        .start_file(
            "ppt/notesSlides/notesSlide1.xml",
            FileOptions::<()>::default(),
        )
        .unwrap();
    writer.write_all(format!(r#"<p:notes xmlns:p="{P_NS}" xmlns:a="{A_NS}"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Speaker notes must survive deletion undo.</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>"#).as_bytes()).unwrap();
    writer
        .start_file(
            "ppt/slideLayouts/slideLayout1.xml",
            FileOptions::<()>::default(),
        )
        .unwrap();
    writer
        .write_all(
            format!(r#"<p:sldLayout xmlns:p="{P_NS}"><p:cSld><p:spTree/></p:cSld></p:sldLayout>"#)
                .as_bytes(),
        )
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let mut session = load(&bytes).unwrap();
    let first = session.deck().slides[0].id.clone();
    session
        .execute(Box::new(slides_core::InsertSlide::new(
            1,
            slides_core::Slide {
                id: "second-slide".into(),
                ..Default::default()
            },
        )))
        .unwrap();
    session.deck_mut().sections = vec![
        slides_core::SlideSection {
            name: "First ]]> section".into(),
            start_slide_id: first.clone(),
        },
        slides_core::SlideSection {
            name: "Second".into(),
            start_slide_id: "second-slide".into(),
        },
    ];
    for (id, slide_id) in [
        ("first-comment", first),
        ("second-comment", "second-slide".into()),
    ] {
        session
            .deck_mut()
            .comments
            .push(slides_core::CommentThread {
                id: id.into(),
                anchor: slides_core::CommentAnchor::Slide { slide_id },
                comments: vec![],
                assigned_to: None,
                resolved: false,
            });
    }
    session
}

#[test]
fn moving_slides_round_trips_order_and_preserves_slide_parts() {
    let mut session = slide_organization_session();
    let original = save(&session).unwrap();
    session.commit_save(original.clone()).unwrap();
    let first_id = session.deck().slides[0].id.clone();
    session
        .execute(Box::new(slides_core::MoveSlide::new(first_id, 1)))
        .unwrap();
    assert!(session.dirty_slides().is_empty());
    let moved = save(&session).unwrap();
    let reloaded = load(&moved).unwrap();
    assert_eq!(reloaded.deck().slides[0].id, "ppt/slides/slide2.xml");
    assert_eq!(reloaded.deck().slides[1].id, "ppt/slides/slide1.xml");
    assert_eq!(reloaded.deck().sections[0].name, "Second");
    for name in [
        "ppt/slides/slide1.xml",
        "ppt/slides/slide2.xml",
        "ppt/charts/chart1.xml",
        "ppt/charts/chart2.xml",
        "ppt/notesSlides/notesSlide1.xml",
    ] {
        assert_eq!(
            entry_bytes(&original, name),
            entry_bytes(&moved, name),
            "reordering changed {name}"
        );
    }
    session.commit_save(moved).unwrap();
    assert!(session.undo());
    assert!(session.dirty_slides().is_empty());
    let undone = load(&save(&session).unwrap()).unwrap();
    assert_eq!(undone.deck().slides[0].id, "ppt/slides/slide1.xml");
    assert_eq!(undone.deck().sections[0].name, "First ]]> section");
    assert!(session.redo());
    let redone = load(&save(&session).unwrap()).unwrap();
    assert_eq!(redone.deck().slides[0].id, "ppt/slides/slide2.xml");
}

#[test]
fn saved_slide_deletion_undo_restores_source_notes_charts_and_metadata() {
    let mut session = slide_organization_session();
    let original = save(&session).unwrap();
    session.commit_save(original.clone()).unwrap();
    let first_id = session.deck().slides[0].id.clone();
    session
        .execute(Box::new(slides_core::DeleteSlide::new(first_id)))
        .unwrap();
    assert_eq!(session.deck().comments.len(), 1);
    assert_eq!(session.deck().sections.len(), 1);
    let deleted = save(&session).unwrap();
    assert!(!zip_entries(&deleted).contains("ppt/slides/slide1.xml"));
    assert!(!zip_entries(&deleted).contains("ppt/slides/_rels/slide1.xml.rels"));
    let reloaded = load(&deleted).unwrap();
    assert_eq!(reloaded.deck().slides.len(), 1);
    assert_eq!(
        reloaded.deck().comments[0].anchor.slide_id(),
        "ppt/slides/slide2.xml"
    );
    assert_eq!(
        reloaded.deck().sections[0].start_slide_id,
        "ppt/slides/slide2.xml"
    );
    session.commit_save(deleted).unwrap();
    session.commit_save(save(&session).unwrap()).unwrap();
    assert!(session.undo());
    let restored = save(&session).unwrap();
    let reloaded = load(&restored).unwrap();
    assert_eq!(reloaded.deck().slides.len(), 2);
    assert_eq!(reloaded.deck().slides[0].id, "ppt/slides/slide1.xml");
    assert!(reloaded.deck().slides[0]
        .notes
        .contains("Speaker notes must survive"));
    assert_eq!(
        chart_at(&reloaded, 0).title.as_deref(),
        Some("Quarterly Sales")
    );
    assert_eq!(
        chart_at(&reloaded, 1).title.as_deref(),
        Some("Regional Revenue")
    );
    assert_eq!(reloaded.deck().comments.len(), 2);
    assert_eq!(reloaded.deck().sections.len(), 2);
    assert!(
        String::from_utf8(entry_bytes(&restored, "ppt/slides/slide1.xml"))
            .unwrap()
            .contains("Keep this source payload")
    );
    for name in [
        "ppt/slides/_rels/slide1.xml.rels",
        "ppt/charts/chart1.xml",
        "ppt/charts/chart2.xml",
        "ppt/notesSlides/notesSlide1.xml",
    ] {
        assert_eq!(
            entry_bytes(&original, name),
            entry_bytes(&restored, name),
            "restoring changed {name}"
        );
    }
    session.commit_save(restored).unwrap();
    assert!(session.redo());
    assert_eq!(
        load(&save(&session).unwrap()).unwrap().deck().slides.len(),
        1
    );
}

fn chart_at(session: &crate::Session, index: usize) -> &ChartShape {
    match &session.deck().slides[0].shapes[index] {
        Shape::Chart(chart) => chart,
        other => panic!("expected chart, got {other:?}"),
    }
}

fn assert_saved_chart_content(session: &crate::Session, bytes: &[u8]) {
    let reloaded = load(bytes).expect("reload saved chart");
    let expected = chart_at(session, 0);
    let actual = chart_at(&reloaded, 0);
    assert_eq!(actual.chart_type, expected.chart_type);
    assert_eq!(actual.data, expected.data);
    assert_eq!(actual.title, expected.title);
}

#[test]
fn chart_edits_round_trip_after_committed_undo_and_redo() {
    let original = build_chart_pptx(&sample_bar_chart_xml());
    let mut session = load(&original).unwrap();
    let slide_id = session.deck().slides[0].id.clone();
    let mut data = chart_at(&session, 0).data.clone();
    if let ChartData::Category { series, .. } = &mut data {
        series[0].values[0] = 123.0;
    }
    let commands: Vec<Box<dyn slides_core::Command>> = vec![
        Box::new(SetChartData::new(slide_id.clone(), 0, data)),
        Box::new(SetChartTitle::new(
            slide_id.clone(),
            0,
            Some("Updated title".into()),
        )),
        Box::new(SetChartTitle::new(slide_id.clone(), 0, None)),
        Box::new(SetChartTitle::new(
            slide_id.clone(),
            0,
            Some("Sales & <Revenue>".into()),
        )),
        Box::new(SetChartType::new(slide_id.clone(), 0, ChartType::Column)),
        Box::new(SetChartType::new(slide_id, 0, ChartType::Line)),
    ];
    let command_count = commands.len();
    for command in commands {
        session.execute(command).unwrap();
        let bytes = save(&session).unwrap();
        assert_saved_chart_content(&session, &bytes);
        session.commit_save(bytes).unwrap();
    }
    for redo in [false, true] {
        for _ in 0..command_count {
            assert!(if redo { session.redo() } else { session.undo() });
            let bytes = save(&session).unwrap();
            assert_saved_chart_content(&session, &bytes);
            for name in ["ppt/theme/theme1.xml", "ppt/slides/_rels/slide1.xml.rels"] {
                assert_eq!(entry_bytes(&original, name), entry_bytes(&bytes, name));
            }
            session.commit_save(bytes).unwrap();
        }
    }
}

#[test]
fn deleting_first_chart_preserves_surviving_chart_source_part() {
    let original = build_two_chart_pptx();
    let mut session = load(&original).unwrap();
    let slide_id = session.deck().slides[0].id.clone();
    session
        .execute(Box::new(DeleteShape::new(slide_id.clone(), 0)))
        .unwrap();
    let bytes = save(&session).unwrap();
    let reloaded = load(&bytes).unwrap();
    assert_eq!(reloaded.deck().slides[0].shapes.len(), 1);
    assert_eq!(
        chart_at(&reloaded, 0).title.as_deref(),
        Some("Regional Revenue")
    );
    assert_eq!(chart_at(&reloaded, 0).id, "8");
    for name in ["ppt/charts/chart1.xml", "ppt/charts/chart2.xml"] {
        assert_eq!(entry_bytes(&original, name), entry_bytes(&bytes, name));
    }
    session.commit_save(bytes).unwrap();
    session
        .execute(Box::new(SetChartTitle::new(
            slide_id,
            0,
            Some("Updated survivor".into()),
        )))
        .unwrap();
    let bytes = save(&session).unwrap();
    assert_saved_chart_content(&session, &bytes);
    assert_eq!(
        entry_bytes(&original, "ppt/charts/chart1.xml"),
        entry_bytes(&bytes, "ppt/charts/chart1.xml")
    );
    let chart_xml = String::from_utf8(entry_bytes(&bytes, "ppt/charts/chart2.xml")).unwrap();
    assert!(chart_xml.contains("opaque second chart metadata"));
}

#[test]
fn undo_chart_deletion_after_save_restores_original_parts_and_ids() {
    let original = build_two_chart_pptx();
    let mut session = load(&original).unwrap();
    let slide_id = session.deck().slides[0].id.clone();
    session
        .execute(Box::new(DeleteShape::new(slide_id, 0)))
        .unwrap();
    session.commit_save(save(&session).unwrap()).unwrap();
    assert!(session.undo());
    let bytes = save(&session).unwrap();
    let reloaded = load(&bytes).unwrap();
    assert_eq!(reloaded.deck().slides[0].shapes.len(), 2);
    assert_eq!(
        chart_at(&reloaded, 0).title.as_deref(),
        Some("Quarterly Sales")
    );
    assert_eq!(
        chart_at(&reloaded, 1).title.as_deref(),
        Some("Regional Revenue")
    );
    assert_eq!(chart_at(&reloaded, 0).id, "7");
    assert_eq!(chart_at(&reloaded, 1).id, "8");
    for name in ["ppt/charts/chart1.xml", "ppt/charts/chart2.xml"] {
        assert_eq!(entry_bytes(&original, name), entry_bytes(&bytes, name));
    }
    session.commit_save(bytes).unwrap();
    assert!(session.redo());
    let bytes = save(&session).unwrap();
    assert_eq!(load(&bytes).unwrap().deck().slides[0].shapes.len(), 1);
    assert_saved_chart_content(&session, &bytes);
}

#[test]
fn chart_move_and_undo_preserve_chart_xml_byte_for_byte() {
    let original = build_chart_pptx(&sample_bar_chart_xml());
    let mut session = load(&original).unwrap();
    let slide_id = session.deck().slides[0].id.clone();
    let mut transform = chart_at(&session, 0).transform;
    transform.frame.x += 50_000.0;
    session
        .execute(Box::new(MoveShape::new(slide_id, 0, transform)))
        .unwrap();
    let moved = save(&session).unwrap();
    session.commit_save(moved.clone()).unwrap();
    assert!(session.undo());
    let undone = save(&session).unwrap();
    for bytes in [&moved, &undone] {
        assert_eq!(
            entry_bytes(&original, "ppt/charts/chart1.xml"),
            entry_bytes(bytes, "ppt/charts/chart1.xml")
        );
    }
}

#[test]
fn inserted_charts_keep_distinct_parts_after_save_and_edit() {
    let mut session = load(&crate::create_blank_pptx()).unwrap();
    let slide_id = session.deck().slides[0].id.clone();
    let first_index = session.deck().slides[0].shapes.len();
    for title in ["First chart", "Second chart"] {
        let chart = ChartShape::new(
            Transform::default(),
            ChartType::Column,
            ChartData::Category {
                categories: vec!["A".into()],
                series: vec![slides_core::CategorySeries {
                    name: "Sales".into(),
                    values: vec![1.0],
                }],
            },
            Some(title.into()),
        )
        .unwrap();
        session
            .execute(Box::new(AddChart::new(slide_id.clone(), chart)))
            .unwrap();
    }
    let original = save(&session).unwrap();
    session.commit_save(original.clone()).unwrap();
    session
        .execute(Box::new(SetChartTitle::new(
            slide_id,
            first_index + 1,
            Some("Edited second chart".into()),
        )))
        .unwrap();
    let bytes = save(&session).unwrap();
    let reloaded = load(&bytes).unwrap();
    assert_eq!(
        chart_at(&reloaded, first_index).title.as_deref(),
        Some("First chart")
    );
    assert_eq!(
        chart_at(&reloaded, first_index + 1).title.as_deref(),
        Some("Edited second chart")
    );
    assert_ne!(
        chart_at(&reloaded, first_index).id,
        chart_at(&reloaded, first_index + 1).id
    );
    for index in [first_index, first_index + 1] {
        assert!(chart_at(&reloaded, index).id.parse::<u32>().unwrap() > 0);
    }
    assert_eq!(
        entry_bytes(&original, "ppt/charts/chart1.xml"),
        entry_bytes(&bytes, "ppt/charts/chart1.xml")
    );
}

#[test]
fn adding_chart_with_duplicate_shape_id_is_rejected() {
    let mut session = load(&build_chart_pptx(&sample_bar_chart_xml())).unwrap();
    let slide_id = session.deck().slides[0].id.clone();
    let duplicate = chart_at(&session, 0).clone();
    assert!(session
        .execute(Box::new(AddChart::new(slide_id, duplicate)))
        .is_err());
    assert_eq!(session.deck().slides[0].shapes.len(), 1);
}

#[test]
fn load_extracts_chart_shape() {
    let original = build_chart_pptx(&sample_bar_chart_xml());
    let session = load(&original).expect("load should succeed");
    let slide = &session.deck().slides[0];
    assert_eq!(slide.shapes.len(), 1);

    let chart = match &slide.shapes[0] {
        Shape::Chart(c) => c,
        other => panic!("expected a chart shape, got {:?}", other),
    };

    assert_eq!(chart.chart_type, ChartType::Bar);
    assert_eq!(chart.title.as_deref(), Some("Quarterly Sales"));
    assert_eq!(
        chart.transform.frame,
        Rect::new(914_400.0, 914_400.0, 6_400_800.0, 1_828_800.0)
    );

    let (categories, series) = match &chart.data {
        ChartData::Category { categories, series } => (categories, series),
        ChartData::XY { .. } => panic!("expected category data"),
    };
    assert_eq!(categories, &["Q1", "Q2", "Q3"]);
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].name, "Product A");
    assert_eq!(series[0].values, &[10.0, 20.0, 30.0]);
    assert_eq!(series[1].name, "Product B");
    assert_eq!(series[1].values, &[15.0, 25.0, 35.0]);

    // The loader records the chart source part separately.
    assert_eq!(
        session
            .chart_source_parts
            .get("ppt/slides/slide1.xml")
            .and_then(|m| m.get("7")),
        Some(&"ppt/charts/chart1.xml".to_string())
    );
}

#[test]
fn save_no_edit_keeps_chart_byte_identical() {
    let original = build_chart_pptx(&sample_bar_chart_xml());
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

#[test]
fn save_edits_chart_data_and_preserves_other_parts() {
    let original = build_chart_pptx(&sample_bar_chart_xml());
    let mut session = load(&original).expect("load should succeed");
    let slide_id = "ppt/slides/slide1.xml".to_string();

    let new_data = ChartData::Category {
        categories: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        series: vec![
            slides_core::CategorySeries {
                name: "Alpha".to_string(),
                values: vec![1.0, 2.0, 3.0],
            },
            slides_core::CategorySeries {
                name: "Beta".to_string(),
                values: vec![4.0, 5.0, 6.0],
            },
        ],
    };
    session
        .execute(Box::new(SetChartData::new(
            slide_id.clone(),
            0,
            new_data.clone(),
        )))
        .expect("set chart data");

    let saved = save(&session).expect("save should succeed");

    // All parts except the chart XML (and regenerated manifest/content-types)
    // must be byte-identical.
    for name in zip_entries(&original) {
        if name == "ppt/charts/chart1.xml"
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

    // Chart part must contain the new data, but still be a valid chart.
    let chart_xml = String::from_utf8(entry_bytes(&saved, "ppt/charts/chart1.xml")).unwrap();
    assert!(chart_xml.contains("Alpha"), "new series name missing");
    assert!(chart_xml.contains("Beta"), "new series name missing");
    assert!(chart_xml.contains("<c:v>1</c:v>"), "new value missing");
    assert!(chart_xml.contains("<c:v>6</c:v>"), "new value missing");
    assert!(
        chart_xml.contains("Quarterly Sales"),
        "title should be preserved"
    );

    // Reload to verify the model reflects the saved data.
    let again = load(&saved).expect("reload should succeed");
    let chart = match &again.deck().slides[0].shapes[0] {
        Shape::Chart(c) => c,
        other => panic!("expected chart after reload, got {:?}", other),
    };
    match &chart.data {
        ChartData::Category { categories, series } => {
            assert_eq!(categories, &["A", "B", "C"]);
            assert_eq!(series.len(), 2);
            assert_eq!(series[0].name, "Alpha");
            assert_eq!(series[0].values, &[1.0, 2.0, 3.0]);
        }
        _ => panic!("expected category data"),
    }
}

#[test]
fn malformed_chart_stays_passthrough_and_warns() {
    let broken_chart = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:chartSpace xmlns:c="{C_NS}">
  <c:chart>
    <c:plotArea>
      <c:unknownChartType/>
    </c:plotArea>
  </c:chart>
</c:chartSpace>"#
    );
    let original = build_chart_pptx(&broken_chart);
    let session = load(&original).expect("load should succeed");
    let slide = &session.deck().slides[0];

    match &slide.shapes[0] {
        Shape::Passthrough(obj) => {
            assert_eq!(obj.label, "graphicFrame");
            let s = String::from_utf8_lossy(&obj.raw_bytes);
            assert!(s.contains("c:chart"));
        }
        other => panic!("expected passthrough for malformed chart, got {:?}", other),
    }

    assert!(session
        .loss_ledger()
        .warnings()
        .iter()
        .any(|w| w.message.contains("chart") && w.message.contains("opaque")));
}

#[test]
fn inserted_chart_round_trips() {
    let blank = crate::create_blank_pptx();
    let mut session = load(&blank).expect("load blank");
    let slide_id = "ppt/slides/slide1.xml".to_string();

    let chart = ChartShape::new(
        Transform {
            frame: Rect::new(1_000_000.0, 1_000_000.0, 4_000_000.0, 2_000_000.0),
            rotation: 0.0,
        },
        ChartType::Column,
        ChartData::Category {
            categories: vec!["X".to_string(), "Y".to_string()],
            series: vec![slides_core::CategorySeries {
                name: "S1".to_string(),
                values: vec![7.0, 8.0],
            }],
        },
        Some("New Chart".to_string()),
    )
    .expect("valid chart");

    session
        .execute(Box::new(slides_core::AddChart::new(
            slide_id.clone(),
            chart,
        )))
        .expect("add chart");

    let saved = save(&session).expect("save");
    let again = load(&saved).expect("reload");
    let found = again.deck().slides[0]
        .shapes
        .iter()
        .find(|s| matches!(s, Shape::Chart(_)))
        .expect("chart should reload");
    let reloaded = match found {
        Shape::Chart(c) => c,
        _ => unreachable!(),
    };
    assert_eq!(reloaded.chart_type, ChartType::Column);
    assert_eq!(reloaded.title.as_deref(), Some("New Chart"));
    match &reloaded.data {
        ChartData::Category { categories, series } => {
            assert_eq!(categories, &["X", "Y"]);
            assert_eq!(series.len(), 1);
            assert_eq!(series[0].name, "S1");
            assert_eq!(series[0].values, &[7.0, 8.0]);
        }
        _ => panic!("expected category data"),
    }
}
