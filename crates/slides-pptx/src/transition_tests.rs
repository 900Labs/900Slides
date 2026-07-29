//! Tests for the Wave 5 loader: `p:transition` and simple `p:timing` build-ins.
//!
//! Each test hand-crafts a minimal PPTX package in memory (no network) and
//! loads it through the public `load` entry point.

use std::io::Write;

use slides_core::{BuildEffect, TransitionKind};
use zip::write::{FileOptions, ZipWriter};

use crate::load;

const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_TYPE_MANIFEST: &str = "http://900labs.github.io/900Slides/1.0/relationships/manifest";
const CT_MANIFEST: &str = "application/vnd.900labs.900slides.manifest+xml";

/// Builds a minimal valid PPTX whose single slide uses the given slide XML
/// body (the children of `<p:sld>`).
fn build_pptx_with_slide_body(slide_body: &str) -> Vec<u8> {
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
        writer.write_all(slide_xml(slide_body).as_bytes()).unwrap();

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

fn presentation_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
</Relationships>"#
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

fn slide_xml(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="{P_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">
{body}
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

fn manifest_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<manifest xmlns="http://900labs.github.io/900Slides/1.0" appVersion="0.1.0" schemaVersion="1" deckId="fixture-deck-id"/>"#
}

/// An empty `p:spTree` boilerplate (required by OOXML).
fn empty_sp_tree() -> &'static str {
    r#"  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="0" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
  </p:cSld>"#
}

/// Slide body with an empty `p:spTree` followed by a fade `p:transition`.
fn fade_transition_body(spd: &str) -> String {
    format!(
        r#"  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="0" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
    <p:transition spd="{spd}"><p:fade/></p:transition>
  </p:cSld>"#
    )
}

/// Slide body with an empty `p:spTree` followed by a `p:morph` transition
/// (OOXML's Morph / Magic Move transition), with no `spd` attribute.
fn morph_transition_body() -> String {
    r#"  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="0" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
    <p:transition><p:morph option="byObject"/></p:transition>
  </p:cSld>"#
        .to_string()
}

#[test]
fn load_extracts_fade_transition() {
    let bytes = build_pptx_with_slide_body(&fade_transition_body("med"));
    let session = load(&bytes).expect("load should succeed");

    let slide = &session.deck().slides[0];
    let transition = slide
        .transition
        .as_ref()
        .expect("fade transition should be modeled");
    assert_eq!(transition.kind, TransitionKind::Fade);
    assert_eq!(transition.duration_ms, 500);
}

#[test]
fn load_extracts_morph_transition() {
    let bytes = build_pptx_with_slide_body(&morph_transition_body());
    let session = load(&bytes).expect("load should succeed");

    let slide = &session.deck().slides[0];
    let transition = slide
        .transition
        .as_ref()
        .expect("morph transition should be modeled");
    assert_eq!(transition.kind, TransitionKind::Morph);
    assert_eq!(transition.duration_ms, 500);
    // Morph is now modeled, so it must not produce a loss warning.
    assert!(
        session.loss_ledger().is_empty(),
        "modeled morph transition must not warn"
    );
}

#[test]
fn load_no_transition_is_none() {
    let bytes = build_pptx_with_slide_body(empty_sp_tree());
    let session = load(&bytes).expect("load should succeed");

    let slide = &session.deck().slides[0];
    assert!(
        slide.transition.is_none(),
        "slide without p:transition should load as None"
    );
}

#[test]
fn load_transition_duration_from_spd() {
    for (spd, expected_ms) in [("slow", 1000u32), ("med", 500), ("fast", 250)] {
        let bytes = build_pptx_with_slide_body(&fade_transition_body(spd));
        let session = load(&bytes).expect("load should succeed");
        let slide = &session.deck().slides[0];
        let transition = slide
            .transition
            .as_ref()
            .expect("transition should be modeled");
        assert_eq!(transition.kind, TransitionKind::Fade, "kind for spd={spd}");
        assert_eq!(
            transition.duration_ms, expected_ms,
            "duration for spd={spd}"
        );
    }
}

#[test]
fn load_extracts_simple_build_in() {
    // One text box (p:cNvPr id="2") with a single fade build-in entrance
    // targeting it (spTgt spid="2"). Shape index 0 in the model.
    let body = r#"  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="0" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
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
          <a:p><a:r><a:t>Hello</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
  <p:timing>
    <p:tnLst>
      <p:par>
        <p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot">
          <p:childTnLst>
            <p:seq concurrent="1" nextAc="seek">
              <p:cTn id="2" dur="indefinite" nodeType="mainSeq">
                <p:childTnLst>
                  <p:par>
                    <p:cTn id="3" fill="hold">
                      <p:stCondLst><p:cond delay="indefinite"/></p:stCondLst>
                      <p:childTnLst>
                        <p:par>
                          <p:cTn id="4" fill="hold">
                            <p:stCondLst><p:cond delay="0"/></p:stCondLst>
                            <p:childTnLst>
                              <p:par>
                                <p:cTn id="5" presetID="10" presetClass="entr" presetSubtype="0" fill="hold" grpId="0" nodeType="clickEffect">
                                  <p:stCondLst><p:cond delay="0"/></p:stCondLst>
                                  <p:childTnLst>
                                    <p:set>
                                      <p:cBhvr>
                                        <p:cTn id="6" dur="1" fill="hold"/>
                                        <p:tgtEl><p:spTgt spid="2"/></p:tgtEl>
                                        <p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst>
                                      </p:cBhvr>
                                      <p:to><p:str val="visible"/></p:to>
                                    </p:set>
                                    <p:animEffect transition="in" filter="fade">
                                      <p:cBhvr>
                                        <p:cTn id="7" dur="500"/>
                                        <p:tgtEl><p:spTgt spid="2"/></p:tgtEl>
                                      </p:cBhvr>
                                    </p:animEffect>
                                  </p:childTnLst>
                                </p:cTn>
                              </p:par>
                            </p:childTnLst>
                          </p:cTn>
                        </p:par>
                      </p:childTnLst>
                    </p:cTn>
                  </p:par>
                </p:childTnLst>
              </p:cTn>
            </p:seq>
          </p:childTnLst>
        </p:cTn>
      </p:par>
    </p:tnLst>
  </p:timing>"#;
    let bytes = build_pptx_with_slide_body(body);
    let session = load(&bytes).expect("load should succeed");

    let slide = &session.deck().slides[0];
    assert_eq!(
        slide.shapes.len(),
        1,
        "the text box should be the only modeled shape"
    );
    let animation = slide
        .animation
        .as_ref()
        .expect("a simple build-in should be modeled, not passthrough");
    assert_eq!(animation.steps.len(), 1);
    let step = &animation.steps[0];
    assert_eq!(step.shape_index, 0);
    assert_eq!(step.effect, BuildEffect::Fade);
    assert_eq!(step.duration_ms, 500);
}

#[test]
fn load_complex_timing_falls_back() {
    // A timing tree containing an emphasis animation (p:anim), which is outside
    // the simple build-in subset. The loader must fall back to animation=None
    // with a loss warning and must NOT panic.
    let body = r#"  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="0" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr/>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p><a:r><a:t>Hi</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
  <p:timing>
    <p:tnLst>
      <p:par>
        <p:cTn id="1" dur="indefinite" nodeType="tmRoot">
          <p:childTnLst>
            <p:seq concurrent="1" nextAc="seek">
              <p:cTn id="2" dur="indefinite" nodeType="mainSeq">
                <p:childTnLst>
                  <p:par>
                    <p:cTn id="3" fill="hold">
                      <p:childTnLst>
                        <p:par>
                          <p:cTn id="4" presetClass="emph" nodeType="clickEffect">
                            <p:childTnLst>
                              <p:anim calcmode="lin" valueType="num">
                                <p:cBhvr>
                                  <p:cTn id="5" dur="500"/>
                                  <p:tgtEl><p:spTgt spid="2"/></p:tgtEl>
                                  <p:attrNameLst><p:attrName>style.opacity</p:attrName></p:attrNameLst>
                                </p:cBhvr>
                              </p:anim>
                            </p:childTnLst>
                          </p:cTn>
                        </p:par>
                      </p:childTnLst>
                    </p:cTn>
                  </p:par>
                </p:childTnLst>
              </p:cTn>
            </p:seq>
          </p:childTnLst>
        </p:cTn>
      </p:par>
    </p:tnLst>
  </p:timing>"#;
    let bytes = build_pptx_with_slide_body(body);
    let session = load(&bytes).expect("load should not panic on complex timing");

    let slide = &session.deck().slides[0];
    assert!(
        slide.animation.is_none(),
        "complex timing must fall back to None"
    );
    assert!(
        !session.loss_ledger().is_empty(),
        "complex timing must record a loss warning"
    );
}
