//! PPTX saving: regenerate only edited parts, copy everything else verbatim.

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use slides_core::{ListStyle, Paragraph, Run, Shape, Slide, TextBox};
use zip::write::{FileOptions, ZipWriter};

use crate::error::Result;
use crate::load::{copy_element, SHAPE_ELEMENT_NAMES};
use crate::package::{write_content_types, write_rels, Rel, CT_MANIFEST, REL_TYPE_MANIFEST};
use crate::session::Session;

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
        let mut entry = archive.by_index(i)?;
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
            let mut original_xml = String::new();
            entry.read_to_string(&mut original_xml)?;
            let xml = patch_slide_xml(slide, &original_xml)?;
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

/// Patches a single slide XML document, replacing only the paragraph content of
/// editable text boxes and leaving every other element (pictures, transitions,
/// timing, backgrounds, etc.) untouched.
fn patch_slide_xml(slide: &Slide, original_xml: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut writer = Writer::new(&mut out);
        let mut reader = Reader::from_str(original_xml);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut shape_idx = 0usize;
        let mut in_sp_tree = false;
        let mut sp_tree_depth = 0usize;
        let mut depth = 0usize;

        loop {
            let event = reader.read_event_into(&mut buf)?;
            match &event {
                Event::Start(e) => {
                    let local = qname_str(e.name());
                    if local == "spTree" {
                        in_sp_tree = true;
                        sp_tree_depth = depth + 1;
                    }

                    if in_sp_tree
                        && depth == sp_tree_depth
                        && SHAPE_ELEMENT_NAMES.contains(&local.as_str())
                    {
                        let start = e.clone().into_owned();
                        let mut captured = Vec::new();
                        let mut capture_writer = Writer::new(&mut captured);
                        copy_element(&mut reader, &start, &mut capture_writer, &mut buf)?;
                        let captured_str = String::from_utf8_lossy(&captured);

                        if let Some(Shape::TextBox(text_box)) = slide.shapes.get(shape_idx) {
                            if captured_str.contains("<p:txBody") {
                                write_patched_text_box(&mut writer, &captured_str, text_box)?;
                            } else {
                                writer.get_mut().write_all(&captured)?;
                            }
                        } else {
                            writer.get_mut().write_all(&captured)?;
                        }
                        shape_idx += 1;
                        buf.clear();
                        continue;
                    }

                    writer.write_event(event)?;
                    depth += 1;
                }
                Event::End(e) => {
                    let local = qname_str(e.name());
                    if local == "spTree" && in_sp_tree && depth == sp_tree_depth + 1 {
                        in_sp_tree = false;
                    }
                    writer.write_event(event)?;
                    depth = depth.saturating_sub(1);
                }
                Event::Empty(e) => {
                    let local = qname_str(e.name());
                    if in_sp_tree
                        && depth == sp_tree_depth
                        && SHAPE_ELEMENT_NAMES.contains(&local.as_str())
                    {
                        if let Some(Shape::TextBox(_)) = slide.shapes.get(shape_idx) {
                            // An empty shape element cannot contain a txBody,
                            // so it does not correspond to an editable text box.
                        }
                        shape_idx += 1;
                    }
                    writer.write_event(event)?;
                }
                Event::Eof => break,
                _ => {
                    writer.write_event(event)?;
                }
            }
            buf.clear();
        }
    }
    Ok(out)
}

fn write_patched_text_box<W: Write>(
    writer: &mut Writer<W>,
    xml: &str,
    text_box: &TextBox,
) -> Result<()> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut depth = 0usize;
    let mut tx_body_depth: Option<usize> = None;
    let mut a_p_depth: usize = 0;
    let mut skip_a_p = false;

    loop {
        let event = reader.read_event_into(&mut buf)?;
        match &event {
            Event::Start(e) => {
                let local = qname_str(e.name());
                if local == "txBody" && tx_body_depth.is_none() {
                    tx_body_depth = Some(depth + 1);
                }
                if local == "p" && tx_body_depth == Some(depth) && !skip_a_p {
                    skip_a_p = true;
                    a_p_depth = 1;
                } else if skip_a_p {
                    a_p_depth += 1;
                }
                if !skip_a_p {
                    writer.write_event(event)?;
                }
                depth += 1;
            }
            Event::End(e) => {
                let local = qname_str(e.name());
                if local == "txBody" && tx_body_depth == Some(depth) {
                    for paragraph in &text_box.paragraphs {
                        write_paragraph(writer, paragraph)?;
                    }
                    tx_body_depth = None;
                }
                if skip_a_p {
                    a_p_depth = a_p_depth.saturating_sub(1);
                    if a_p_depth == 0 {
                        skip_a_p = false;
                    }
                } else {
                    writer.write_event(event)?;
                }
                depth -= 1;
            }
            Event::Empty(e) => {
                let local = qname_str(e.name());
                if local == "p" && tx_body_depth == Some(depth) && !skip_a_p {
                    // Skip an empty paragraph placeholder; new paragraphs are
                    // written before txBody closes.
                } else if skip_a_p {
                    // Empty elements do not change the nesting depth, so do
                    // not increment a_p_depth here.
                } else {
                    writer.write_event(event)?;
                }
            }
            Event::Eof => break,
            _ => {
                if !skip_a_p {
                    writer.write_event(event)?;
                }
            }
        }
        buf.clear();
    }
    Ok(())
}

fn qname_str(q: quick_xml::name::QName) -> String {
    String::from_utf8_lossy(q.local_name().as_ref()).into_owned()
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
