//! PPTX load and save (native format).
//!
//! This crate is the native format boundary for 900Slides. It converts a PPTX
//! package into a [`slides_core::Deck`] while preserving every unrecognized
//! OOXML part as a [`slides_core::PassthroughObject`]. Saving rebuilds only the
//! slide parts that have been edited; all other parts are copied byte-for-byte
//! from the original package.

use std::io::Write;

use zip::write::{FileOptions, ZipWriter};

mod error;
mod ledger;
mod load;
mod package;
mod save;
mod session;

pub use error::{Error, Result};
pub use ledger::{LossLedger, LossWarning};
pub use session::Session;

use crate::package::{CT_MANIFEST, REL_TYPE_MANIFEST};
use slides_core::Deck;

/// Loads a PPTX package from bytes into an editable [`Session`].
///
/// The returned session keeps the original bytes so that a later [`save`]
/// can preserve untouched parts byte-for-byte.
pub fn load(bytes: &[u8]) -> Result<Session> {
    let result = load::load(bytes)?;
    let content_types = {
        let mut archive = load::open_and_validate(bytes)?;
        let xml = load::read_entry_to_string(&mut archive, "[Content_Types].xml")?;
        package::parse_content_types(&xml)?
    };
    Ok(Session::new(
        result.deck,
        bytes.to_vec(),
        result.package_rels,
        content_types,
        result.slide_paths,
        result.manifest_path,
        result.loss_ledger,
    ))
}

/// Saves the current [`Session`] as a PPTX package.
///
/// Only slides in the dirty set are regenerated; all other parts are copied
/// verbatim from the original package bytes.
pub fn save(session: &Session) -> Result<Vec<u8>> {
    save::save(session)
}

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Creates a minimal, valid blank PPTX package in memory.
///
/// The package contains the required OOXML parts for PowerPoint to open it:
/// `[Content_Types].xml`, `_rels/.rels`, `ppt/presentation.xml`,
/// `ppt/_rels/presentation.xml.rels`, `ppt/slides/slide1.xml`,
/// `ppt/theme/theme1.xml`, and a 900Slides custom XML manifest.
/// The single slide contains one empty text box.
pub fn create_blank_pptx() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut buf);
        let options =
            FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

        writer.start_file("[Content_Types].xml", options).unwrap();
        writer
            .write_all(blank_content_types_xml().as_bytes())
            .unwrap();

        writer.start_file("_rels/.rels", options).unwrap();
        writer
            .write_all(blank_package_rels_xml().as_bytes())
            .unwrap();

        writer.start_file("ppt/presentation.xml", options).unwrap();
        writer
            .write_all(blank_presentation_xml().as_bytes())
            .unwrap();

        writer
            .start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        writer
            .write_all(blank_presentation_rels_xml().as_bytes())
            .unwrap();

        writer.start_file("ppt/slides/slide1.xml", options).unwrap();
        writer.write_all(blank_slide1_xml().as_bytes()).unwrap();

        writer.start_file("ppt/theme/theme1.xml", options).unwrap();
        writer.write_all(blank_theme_xml().as_bytes()).unwrap();

        writer.start_file("customXml/item1.xml", options).unwrap();
        writer.write_all(blank_manifest_xml().as_bytes()).unwrap();

        writer.finish().unwrap();
    }
    buf.into_inner()
}

fn blank_content_types_xml() -> String {
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

fn blank_package_rels_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
  <Relationship Id="rId2" Type="{REL_TYPE_MANIFEST}" Target="customXml/item1.xml"/>
</Relationships>"#
    )
}

fn blank_presentation_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
</Relationships>"#
        .to_string()
}

fn blank_presentation_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="{P_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">
  <p:sldIdLst>
    <p:sldId id="256" r:id="rId1"/>
  </p:sldIdLst>
</p:presentation>"#
    )
}

fn blank_slide1_xml() -> String {
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
          </a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#
    )
}

fn blank_theme_xml() -> String {
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

fn blank_manifest_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<manifest xmlns="http://900labs.github.io/900Slides/1.0" appVersion="0.1.0" schemaVersion="1" deckId="new"/>"#
        .to_string()
}

const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[cfg(test)]
mod tests;
