//! PPTX saving: regenerate only edited parts, copy everything else verbatim.

use std::collections::HashSet;
use std::io::{Cursor, Write};

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use slides_core::{ListStyle, Paragraph, Rect, Run, Shape, Slide};
use zip::write::{FileOptions, ZipWriter};

use crate::error::Result;
use crate::package::{write_content_types, write_rels, Rel, CT_MANIFEST, REL_TYPE_MANIFEST};
use crate::session::Session;

const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const MANIFEST_NS: &str = "http://900labs.github.io/900Slides/1.0";

/// Serializes the current deck to a PPTX package, preserving every untouched
/// part byte-for-byte.
pub fn save(session: &Session) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(&session.original_bytes))?;
    let mut out = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(&mut out);
    let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

    let mut content_types = session.content_types.clone();
    content_types.ensure_override(&session.manifest_path, CT_MANIFEST);

    let manifest_xml = write_manifest(session);
    let need_manifest_rel = session.manifest_rel_id.is_none();
    let dirty_paths: HashSet<String> = session
        .dirty_slides
        .iter()
        .filter_map(|id| session.slide_paths.get(id).cloned())
        .collect();

    let mut manifest_seen = false;
    let mut content_types_seen = false;
    let mut rels_seen = false;

    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        if name == "[Content_Types].xml" {
            let xml = write_content_types(&content_types)?;
            writer.start_file(&name, options)?;
            writer.write_all(&xml)?;
            content_types_seen = true;
        } else if name == session.manifest_path {
            writer.start_file(&name, options)?;
            writer.write_all(&manifest_xml)?;
            manifest_seen = true;
        } else if name == "_rels/.rels" && need_manifest_rel {
            let rels = add_manifest_rel(&session.package_rels, &session.manifest_path);
            let xml = write_rels(&rels)?;
            writer.start_file(&name, options)?;
            writer.write_all(&xml)?;
            rels_seen = true;
        } else if let Some(slide) = find_slide_by_path(session, &dirty_paths, &name) {
            let xml = write_slide_xml(slide)?;
            writer.start_file(&name, options)?;
            writer.write_all(&xml)?;
        } else {
            writer.raw_copy_file(entry)?;
        }
    }

    if !manifest_seen {
        writer.start_file(&session.manifest_path, options)?;
        writer.write_all(&manifest_xml)?;
    }
    if !content_types_seen {
        let xml = write_content_types(&content_types)?;
        writer.start_file("[Content_Types].xml", options)?;
        writer.write_all(&xml)?;
    }
    if need_manifest_rel && !rels_seen {
        let rels = add_manifest_rel(&session.package_rels, &session.manifest_path);
        let xml = write_rels(&rels)?;
        writer.start_file("_rels/.rels", options)?;
        writer.write_all(&xml)?;
    }

    writer.finish()?;
    Ok(out.into_inner())
}

fn find_slide_by_path<'a>(
    session: &'a Session,
    dirty_paths: &HashSet<String>,
    path: &str,
) -> Option<&'a Slide> {
    if !dirty_paths.contains(path) {
        return None;
    }
    session
        .slide_paths
        .iter()
        .find(|(_, p)| *p == path)
        .and_then(|(id, _)| session.deck.slides.iter().find(|s| s.id == *id))
}

fn add_manifest_rel(rels: &[Rel], manifest_path: &str) -> Vec<Rel> {
    let mut rels = rels.to_vec();
    rels.push(Rel {
        id: next_rel_id(&rels),
        rel_type: REL_TYPE_MANIFEST.to_string(),
        target: manifest_path.trim_start_matches('/').to_string(),
        target_mode: None,
    });
    rels
}

fn next_rel_id(rels: &[Rel]) -> String {
    let mut max = 0usize;
    for rel in rels {
        if let Some(n) = rel
            .id
            .strip_prefix("rId")
            .and_then(|s| s.parse::<usize>().ok())
        {
            max = max.max(n);
        }
    }
    format!("rId{}", max + 1)
}

fn write_manifest(session: &Session) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut writer = Writer::new_with_indent(&mut out, b' ', 2);
        writer
            .write_event(Event::Decl(BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                Some("yes"),
            )))
            .ok();
        let mut elem = BytesStart::new("manifest");
        elem.push_attribute(("xmlns", MANIFEST_NS));
        elem.push_attribute(("appVersion", env!("CARGO_PKG_VERSION")));
        elem.push_attribute((
            "schemaVersion",
            session.deck.schema_version.to_string().as_str(),
        ));
        elem.push_attribute(("deckId", session.deck.id.as_str()));
        writer.write_event(Event::Empty(elem)).ok();
    }
    out
}

fn write_slide_xml(slide: &Slide) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut writer = Writer::new_with_indent(&mut out, b' ', 2);
        writer.write_event(Event::Decl(BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            Some("yes"),
        )))?;

        let mut sld = BytesStart::new("p:sld");
        sld.push_attribute(("xmlns:p", P_NS));
        sld.push_attribute(("xmlns:a", A_NS));
        sld.push_attribute(("xmlns:r", R_NS));
        writer.write_event(Event::Start(sld.clone()))?;

        writer.write_event(Event::Start(BytesStart::new("p:cSld")))?;
        writer.write_event(Event::Start(BytesStart::new("p:bg")))?;
        writer.write_event(Event::Start(BytesStart::new("p:bgPr")))?;
        writer.write_event(Event::Empty(BytesStart::new("a:noFill")))?;
        writer.write_event(Event::End(BytesEnd::new("p:bgPr")))?;
        writer.write_event(Event::End(BytesEnd::new("p:bg")))?;

        writer.write_event(Event::Start(BytesStart::new("p:spTree")))?;
        writer.write_event(Event::Start(BytesStart::new("p:nvGrpSpPr")))?;
        writer.write_event(Event::Empty(BytesStart::new("p:cNvPr")))?;
        writer.write_event(Event::Empty(BytesStart::new("p:cNvGrpSpPr")))?;
        writer.write_event(Event::Empty(BytesStart::new("p:nvPr")))?;
        writer.write_event(Event::End(BytesEnd::new("p:nvGrpSpPr")))?;
        writer.write_event(Event::Empty(BytesStart::new("p:grpSpPr")))?;

        for (shape_index, shape) in slide.shapes.iter().enumerate() {
            match shape {
                Shape::TextBox(text_box) => {
                    write_text_box(&mut writer, shape_index + 1, text_box)?;
                }
                Shape::Passthrough(obj) => {
                    writer.get_mut().write_all(&obj.raw_bytes)?;
                }
            }
        }

        writer.write_event(Event::End(BytesEnd::new("p:spTree")))?;
        writer.write_event(Event::End(BytesEnd::new("p:cSld")))?;
        writer.write_event(Event::End(BytesEnd::new("p:sld")))?;
    }
    Ok(out)
}

fn write_text_box<W: Write>(
    writer: &mut Writer<W>,
    shape_id: usize,
    text_box: &slides_core::TextBox,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("p:sp")))?;

    writer.write_event(Event::Start(BytesStart::new("p:nvSpPr")))?;
    let mut cnvpr = BytesStart::new("p:cNvPr");
    cnvpr.push_attribute(("id", shape_id.to_string().as_str()));
    cnvpr.push_attribute(("name", format!("TextBox {}", shape_id).as_str()));
    writer.write_event(Event::Empty(cnvpr))?;
    writer.write_event(Event::Empty(BytesStart::new("p:cNvSpPr")))?;
    writer.write_event(Event::Empty(BytesStart::new("p:nvPr")))?;
    writer.write_event(Event::End(BytesEnd::new("p:nvSpPr")))?;

    writer.write_event(Event::Start(BytesStart::new("p:spPr")))?;
    write_xfrm(writer, &text_box.frame)?;
    writer.write_event(Event::End(BytesEnd::new("p:spPr")))?;

    writer.write_event(Event::Start(BytesStart::new("p:txBody")))?;
    writer.write_event(Event::Empty(BytesStart::new("a:bodyPr")))?;
    writer.write_event(Event::Empty(BytesStart::new("a:lstStyle")))?;
    for paragraph in &text_box.paragraphs {
        write_paragraph(writer, paragraph)?;
    }
    writer.write_event(Event::End(BytesEnd::new("p:txBody")))?;

    writer.write_event(Event::End(BytesEnd::new("p:sp")))?;
    Ok(())
}

fn write_xfrm<W: Write>(writer: &mut Writer<W>, frame: &Rect) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("a:xfrm")))?;
    let mut off = BytesStart::new("a:off");
    off.push_attribute(("x", format_emu(frame.x).as_str()));
    off.push_attribute(("y", format_emu(frame.y).as_str()));
    writer.write_event(Event::Empty(off))?;
    let mut ext = BytesStart::new("a:ext");
    ext.push_attribute(("cx", format_emu(frame.width).as_str()));
    ext.push_attribute(("cy", format_emu(frame.height).as_str()));
    writer.write_event(Event::Empty(ext))?;
    writer.write_event(Event::End(BytesEnd::new("a:xfrm")))?;
    Ok(())
}

fn format_emu(value: f64) -> String {
    format!("{:.0}", value)
}

fn write_paragraph<W: Write>(writer: &mut Writer<W>, paragraph: &Paragraph) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("a:p")))?;

    match paragraph.list_style {
        ListStyle::None => {
            writer.write_event(Event::Empty(BytesStart::new("a:pPr")))?;
        }
        ListStyle::Ordered => {
            let ppr = BytesStart::new("a:pPr");
            let mut num = BytesStart::new("a:buAutoNum");
            num.push_attribute(("type", "arabicParenR"));
            writer.write_event(Event::Start(ppr))?;
            writer.write_event(Event::Empty(num))?;
            writer.write_event(Event::End(BytesEnd::new("a:pPr")))?;
        }
        ListStyle::Unordered => {
            let ppr = BytesStart::new("a:pPr");
            let mut bullet = BytesStart::new("a:buChar");
            bullet.push_attribute(("char", "•"));
            writer.write_event(Event::Start(ppr))?;
            writer.write_event(Event::Empty(bullet))?;
            writer.write_event(Event::End(BytesEnd::new("a:pPr")))?;
        }
    }

    for run in &paragraph.runs {
        write_run(writer, run)?;
    }

    writer.write_event(Event::End(BytesEnd::new("a:p")))?;
    Ok(())
}

fn write_run<W: Write>(writer: &mut Writer<W>, run: &Run) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("a:r")))?;

    let mut rpr = BytesStart::new("a:rPr");
    if run.bold {
        rpr.push_attribute(("b", "1"));
    }
    if run.italic {
        rpr.push_attribute(("i", "1"));
    }
    if run.underline {
        rpr.push_attribute(("u", "sng"));
    }
    writer.write_event(Event::Empty(rpr))?;

    if run.text.is_empty() {
        writer.write_event(Event::Empty(BytesStart::new("a:t")))?;
    } else {
        let mut t = BytesStart::new("a:t");
        t.push_attribute(("xml:space", "preserve"));
        writer.write_event(Event::Start(t))?;
        writer.write_event(Event::Text(BytesText::new(&run.text)))?;
        writer.write_event(Event::End(BytesEnd::new("a:t")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("a:r")))?;
    Ok(())
}
