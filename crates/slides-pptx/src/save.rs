//! PPTX saving: regenerate only edited parts, copy everything else verbatim.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use slides_core::{
    Crop, DashStyle, Fill, GeometricShape, Geometry, ImageShape, ListStyle, Outline, Paragraph,
    Run, Shape, Slide, TextBox, Transform, VerticalAlign,
};
use zip::write::{FileOptions, ZipWriter};

use crate::error::{Error, Result};
use crate::geometry;
use crate::load::{copy_element, extract_blip_embed, rels_path_for, SHAPE_ELEMENT_NAMES};
use crate::media as pkgmedia;
use crate::package::{
    parse_rels, write_content_types, write_rels, Rel, CT_MANIFEST, REL_TYPE_HYPERLINK,
    REL_TYPE_IMAGE, REL_TYPE_MANIFEST,
};
use crate::session::Session;

const MANIFEST_NS: &str = "http://900labs.github.io/900Slides/1.0";

/// Serializes the current deck to a PPTX package, preserving every untouched
/// part byte-for-byte.
pub fn save(session: &Session) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(session.original_bytes.as_slice()))?;
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

    // Pre-pass: resolve media for any images inserted since load and collect
    // hyperlink URLs used by runs on dirty slides. This assigns each new image a
    // package media part, each new hyperlink a relationship id, and any needed
    // content-type default, all captured before the main write loop runs.
    let mut slide_rids: HashMap<String, HashMap<String, String>> = session.slide_media_rids.clone();
    let mut slide_link_rids: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut new_media_parts: Vec<(String, Vec<u8>)> = Vec::new();
    // rels path -> relationships to append (or, if the file is absent, to write
    // as a brand-new part).
    let mut slide_rels_additions: HashMap<String, Vec<Rel>> = HashMap::new();

    let mut media_counter = max_media_counter(&mut archive)?;

    for slide_id in session.dirty_slides.iter() {
        let Some(slide_path) = session.slide_paths.get(slide_id) else {
            continue;
        };
        let Some(slide) = session.deck.slides.iter().find(|s| &s.id == slide_id) else {
            continue;
        };

        let rels_path = rels_path_for(slide_path);
        let existing_rels = match crate::load::read_entry_to_string(&mut archive, &rels_path) {
            Ok(xml) => parse_rels(&xml).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        let mut max_rid = max_rel_number(&existing_rels);
        let rids = slide_rids.entry(slide_id.clone()).or_default();
        let mut additions = Vec::new();

        // Reuse existing hyperlink relationships and allocate new ones for URLs
        // that do not yet have a relationship on this slide.
        let mut url_to_rid: HashMap<String, String> = existing_rels
            .iter()
            .filter(|r| r.rel_type == REL_TYPE_HYPERLINK)
            .map(|r| (r.target.clone(), r.id.clone()))
            .collect();
        for shape in &slide.shapes {
            let Shape::TextBox(text_box) = shape else {
                continue;
            };
            for paragraph in &text_box.paragraphs {
                for run in &paragraph.runs {
                    if let Some(link) = &run.link {
                        if url_to_rid.contains_key(&link.url) {
                            continue;
                        }
                        max_rid += 1;
                        let rid = format!("rId{max_rid}");
                        url_to_rid.insert(link.url.clone(), rid.clone());
                        additions.push(Rel {
                            id: rid,
                            rel_type: REL_TYPE_HYPERLINK.to_string(),
                            target: link.url.clone(),
                            target_mode: Some("External".to_string()),
                        });
                    }
                }
            }
        }
        slide_link_rids.insert(rels_path.clone(), url_to_rid);

        for shape in &slide.shapes {
            let Shape::Image(image) = shape else {
                continue;
            };
            if rids.contains_key(&image.media_ref) {
                continue;
            };
            let Some(entry) = session.deck.media.get(&image.media_ref) else {
                continue;
            };
            let ext = pkgmedia::extension_for_mime(&entry.mime).ok_or_else(|| {
                crate::error::Error::Save(format!(
                    "image media '{}' has unsupported MIME type '{}'; cannot emit a \
                     package part. Re-insert the image in a supported format.",
                    image.media_ref, entry.mime
                ))
            })?;
            // The legacy `else { continue }` silently produced a PPTX whose
            // `<a:blip r:embed>` pointed at a part that was never written (a
            // corrupt file). An explicit error surfaces the problem instead.
            media_counter += 1;
            let part_path = format!("ppt/media/image{media_counter}.{ext}");
            max_rid += 1;
            let rid = format!("rId{max_rid}");
            rids.insert(image.media_ref.clone(), rid.clone());
            new_media_parts.push((part_path.clone(), entry.bytes.clone()));
            additions.push(Rel {
                id: rid,
                rel_type: REL_TYPE_IMAGE.to_string(),
                target: pkgmedia::relative_target(slide_path, &part_path),
                target_mode: None,
            });
            content_types
                .defaults
                .entry(ext.to_string())
                .or_insert_with(|| entry.mime.clone());
        }

        if !additions.is_empty() {
            slide_rels_additions.insert(rels_path, additions);
        }
    }

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
        } else if let Some(additions) = slide_rels_additions.remove(&name) {
            let mut xml = String::new();
            entry.read_to_string(&mut xml)?;
            let mut rels = parse_rels(&xml)?;
            rels.extend(additions);
            let bytes = write_rels(&rels)?;
            writer.start_file(&name, options)?;
            writer.write_all(&bytes)?;
        } else if let Some(slide) = find_slide_by_path(session, &dirty_paths, &name) {
            let mut original_xml = String::new();
            entry.read_to_string(&mut original_xml)?;
            let rids = slide_rids
                .get(slide.id.as_str())
                .cloned()
                .unwrap_or_default();
            let rels_path = rels_path_for(&name);
            let link_rids = slide_link_rids.get(&rels_path).cloned().unwrap_or_default();
            let xml = patch_slide_xml(slide, &original_xml, &rids, &link_rids)?;
            writer.start_file(&name, options)?;
            writer.write_all(&xml)?;
        } else {
            writer.raw_copy_file(entry)?;
        }
    }

    // Write brand-new media parts (inserted images) and any slide rels files
    // that did not previously exist.
    for (part, bytes) in &new_media_parts {
        writer.start_file(part.as_str(), options)?;
        writer.write_all(bytes)?;
    }
    for (rels_path, additions) in &slide_rels_additions {
        let bytes = write_rels(additions)?;
        writer.start_file(rels_path.as_str(), options)?;
        writer.write_all(&bytes)?;
    }

    if !manifest_seen {
        writer.start_file(session.manifest_path.as_str(), options)?;
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

/// Returns the highest `image<N>` index under `ppt/media/`, or `0` if none.
fn max_media_counter(archive: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Result<usize> {
    let mut max = 0usize;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name();
        if let Some(rest) = name.strip_prefix("ppt/media/image") {
            if let Some(num) = rest.split('.').next() {
                if let Ok(n) = num.parse::<usize>() {
                    max = max.max(n);
                }
            }
        }
    }
    Ok(max)
}

/// Returns the highest numeric `rId<N>` in `rels`, or `0` if none.
fn max_rel_number(rels: &[Rel]) -> usize {
    rels.iter()
        .filter_map(|r| {
            r.id.strip_prefix("rId")
                .and_then(|s| s.parse::<usize>().ok())
        })
        .max()
        .unwrap_or(0)
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

/// Counts the top-level shape elements (`p:sp`, `p:pic`, `p:grpSp`, ...) inside
/// the slide's `<p:spTree>`.
fn count_top_level_shapes(xml: &str) -> Result<usize> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut count = 0usize;
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
                    count += 1;
                }
                depth += 1;
            }
            Event::End(e) => {
                let local = qname_str(e.name());
                if local == "spTree" && in_sp_tree {
                    in_sp_tree = false;
                }
                depth = depth.saturating_sub(1);
            }
            Event::Empty(e) => {
                let local = qname_str(e.name());
                if in_sp_tree
                    && depth == sp_tree_depth
                    && SHAPE_ELEMENT_NAMES.contains(&local.as_str())
                {
                    count += 1;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(count)
}

/// Regenerates the slide XML by rewriting the entire `<p:spTree>` shape set from
/// the model. Used when a shape has been deleted, where the positional patch
/// path cannot stay aligned. Everything outside the shape tree (and the
/// non-shape children of the tree: `nvGrpSpPr`, `grpSpPr`) is copied verbatim;
/// all original shape elements are dropped and replaced by the model's shapes
/// in order.
fn regenerate_sp_tree(
    slide: &Slide,
    original_xml: &str,
    rids: &HashMap<String, String>,
    link_rids: &HashMap<String, String>,
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut writer = Writer::new(&mut out);
        let mut reader = Reader::from_str(original_xml);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut in_sp_tree = false;
        let mut sp_tree_depth = 0usize;
        let mut depth = 0usize;

        loop {
            let event = reader.read_event_into(&mut buf)?;
            match &event {
                Event::Start(e) => {
                    let local = qname_str(e.name());
                    if local == "spTree" && !in_sp_tree {
                        in_sp_tree = true;
                        sp_tree_depth = depth + 1;
                    }
                    // A top-level original shape element is dropped entirely
                    // (skipped without writing) and replaced by the model's
                    // shapes when the tree closes. `skip_subtree` consumes the
                    // element's body and closing End, so depth is unchanged.
                    if in_sp_tree
                        && depth == sp_tree_depth
                        && SHAPE_ELEMENT_NAMES.contains(&local.as_str())
                    {
                        skip_subtree(&mut reader, &mut buf)?;
                    } else {
                        writer.write_event(event.clone())?;
                        depth += 1;
                    }
                }
                Event::End(e) => {
                    let local = qname_str(e.name());
                    if local == "spTree" && in_sp_tree {
                        for (index, shape) in slide.shapes.iter().enumerate() {
                            write_model_shape(&mut writer, shape, rids, link_rids, index)?;
                        }
                        in_sp_tree = false;
                    }
                    writer.write_event(event.clone())?;
                    depth = depth.saturating_sub(1);
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

/// Writes a single model shape as a complete OOXML element.
fn write_model_shape<W: Write>(
    writer: &mut Writer<W>,
    shape: &Shape,
    rids: &HashMap<String, String>,
    link_rids: &HashMap<String, String>,
    index: usize,
) -> Result<()> {
    let id = 100_000 + index as i64;
    match shape {
        Shape::Image(image) => {
            if let Some(embed) = rids.get(&image.media_ref) {
                let name = format!("Picture {}", index + 1);
                let xml = pic_element_xml(image, embed, id, &name);
                writer.get_mut().write_all(xml.as_bytes())?;
            }
        }
        Shape::Geometric(geo) => {
            let name = format!("Shape {}", index + 1);
            let xml = sp_element_xml(geo, id, &name);
            writer.get_mut().write_all(xml.as_bytes())?;
        }
        Shape::TextBox(text_box) => {
            let name = format!("TextBox {}", index + 1);
            let header = format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/>\
                 <p:cNvSpPr><a:spLocks/></p:cNvSpPr><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{xfrm}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr/><a:lstStyle/>",
                xfrm = xfrm_xml(&slides_core::Transform {
                    frame: text_box.frame,
                    rotation: 0.0,
                })
            );
            writer.get_mut().write_all(header.as_bytes())?;
            for paragraph in &text_box.paragraphs {
                write_paragraph(writer, paragraph, link_rids)?;
            }
            writer.get_mut().write_all(b"</p:txBody></p:sp>")?;
        }
        Shape::Passthrough(object) => {
            writer.get_mut().write_all(&object.raw_bytes)?;
        }
    }
    Ok(())
}

/// Patches a single slide XML document in place.
///
/// Editable text boxes have their paragraphs rewritten; modeled images and
/// geometric shapes have their `<p:spPr>` (and `<p:blipFill>` for pictures)
/// regenerated from the model while every other element on the slide —
/// pictures, transitions, timing, backgrounds, non-editable shapes — is copied
/// through untouched. Shapes added since load are appended to the shape tree.
fn patch_slide_xml(
    slide: &Slide,
    original_xml: &str,
    rids: &HashMap<String, String>,
    link_rids: &HashMap<String, String>,
) -> Result<Vec<u8>> {
    // If the model has fewer shapes than the original slide XML, a shape was
    // deleted. The positional patch below cannot represent a deletion (it would
    // misalign every following shape and leave the deleted element in place), so
    // fall back to regenerating the entire shape tree from the model.
    if count_top_level_shapes(original_xml)? > slide.shapes.len() {
        return regenerate_sp_tree(slide, original_xml, rids, link_rids);
    }

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

                        let model_shape = slide.shapes.get(shape_idx);
                        let handled = match model_shape {
                            Some(shape @ Shape::TextBox(text_box))
                                if local == "sp"
                                    && String::from_utf8_lossy(&captured).contains("<p:txBody") =>
                            {
                                write_patched_text_box(
                                    &mut writer,
                                    &String::from_utf8_lossy(&captured),
                                    text_box,
                                    link_rids,
                                )?;
                                Some(shape)
                            }
                            Some(shape) if matches!(shape, Shape::Image(_)) && local == "pic" => {
                                patch_shape_xml_into(&mut writer, &captured, shape, rids)?;
                                Some(shape)
                            }
                            Some(shape)
                                if matches!(shape, Shape::Geometric(_)) && local == "sp" =>
                            {
                                patch_shape_xml_into(&mut writer, &captured, shape, rids)?;
                                Some(shape)
                            }
                            _ => {
                                writer.get_mut().write_all(&captured)?;
                                model_shape
                            }
                        };
                        if handled.is_some() {
                            shape_idx += 1;
                        }
                        buf.clear();
                        continue;
                    }

                    writer.write_event(event)?;
                    depth += 1;
                }
                Event::End(e) => {
                    let local = qname_str(e.name());
                    if local == "spTree" && in_sp_tree {
                        // Append any shapes the model carries beyond the
                        // original XML (inserted since load).
                        while shape_idx < slide.shapes.len() {
                            append_shape(
                                &mut writer,
                                &slide.shapes[shape_idx],
                                rids,
                                link_rids,
                                shape_idx,
                            )?;
                            shape_idx += 1;
                        }
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

/// Replaces the `<p:spPr>` (and `<p:blipFill>` for pictures) of a captured
/// shape element with output regenerated from the model, streaming every other
/// part of the element through verbatim so non-modeled attributes survive.
fn patch_shape_xml_into<W: Write>(
    writer: &mut Writer<W>,
    captured: &[u8],
    shape: &Shape,
    rids: &HashMap<String, String>,
) -> Result<()> {
    let captured_str = std::str::from_utf8(captured)
        .map_err(|_| Error::UnsupportedFormat("non-utf8 shape element".into()))?;
    let generated = generate_shape_xml(shape, rids, captured_str)?;
    replace_sp_pr_and_blip_fill(
        writer,
        captured_str,
        &generated.sp_pr,
        generated.blip_fill.as_deref(),
    )?;
    Ok(())
}

/// Streams `captured_str` to `writer`, replacing the top-level `<p:spPr>` child
/// with `sp_pr` and (when provided) the top-level `<p:blipFill>` child with
/// `blip_fill`. All other content is copied verbatim.
fn replace_sp_pr_and_blip_fill<W: Write>(
    writer: &mut Writer<W>,
    captured_str: &str,
    sp_pr: &str,
    blip_fill: Option<&str>,
) -> Result<()> {
    let mut reader = Reader::from_str(captured_str);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut depth: i64 = 0;
    loop {
        let event = reader.read_event_into(&mut buf)?;
        match &event {
            Event::Start(e) => {
                let local = qname_str(e.name());
                if depth == 1 && local == "spPr" {
                    writer.get_mut().write_all(sp_pr.as_bytes())?;
                    skip_subtree(&mut reader, &mut buf)?;
                    buf.clear();
                    continue;
                }
                if depth == 1 && local == "blipFill" {
                    if let Some(fill) = blip_fill {
                        writer.get_mut().write_all(fill.as_bytes())?;
                        skip_subtree(&mut reader, &mut buf)?;
                        buf.clear();
                        continue;
                    }
                }
                writer.write_event(event)?;
                depth += 1;
            }
            Event::Empty(e) => {
                let local = qname_str(e.name());
                if depth == 1 && local == "spPr" {
                    writer.get_mut().write_all(sp_pr.as_bytes())?;
                    continue;
                }
                if depth == 1 && local == "blipFill" {
                    if let Some(fill) = blip_fill {
                        writer.get_mut().write_all(fill.as_bytes())?;
                        continue;
                    }
                }
                writer.write_event(event)?;
            }
            Event::End(_) => {
                writer.write_event(event)?;
                depth -= 1;
            }
            Event::Eof => break,
            _ => {
                writer.write_event(event)?;
            }
        }
        buf.clear();
    }
    Ok(())
}

/// Consumes events until the matching `End` of an already-opened `Start`.
fn skip_subtree<R: std::io::BufRead>(reader: &mut Reader<R>, buf: &mut Vec<u8>) -> Result<()> {
    let mut depth = 0i64;
    loop {
        match reader.read_event_into(buf)? {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            Event::Eof => return Err(Error::MissingPart("truncated shape XML".into())),
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

/// The model-generated XML for a modeled shape element.
struct GeneratedShape {
    sp_pr: String,
    blip_fill: Option<String>,
}

/// Builds the regenerated `<p:spPr>` (and `<p:blipFill>` for images) for a
/// modeled shape, recovering the relationship id for an image from `rids` or,
/// failing that, from the original captured XML.
fn generate_shape_xml(
    shape: &Shape,
    rids: &HashMap<String, String>,
    captured_str: &str,
) -> Result<GeneratedShape> {
    Ok(match shape {
        Shape::Image(image) => {
            let embed = rids
                .get(&image.media_ref)
                .cloned()
                .or_else(|| extract_blip_embed(captured_str));
            GeneratedShape {
                sp_pr: image_sp_pr_xml(image),
                blip_fill: embed.map(|e| blip_fill_xml(&e, image.crop.as_ref())),
            }
        }
        Shape::Geometric(geo) => GeneratedShape {
            sp_pr: geometric_sp_pr_xml(geo),
            blip_fill: None,
        },
        _ => GeneratedShape {
            sp_pr: String::new(),
            blip_fill: None,
        },
    })
}

/// Appends a brand-new shape element (inserted since load) to the shape tree.
fn append_shape<W: Write>(
    writer: &mut Writer<W>,
    shape: &Shape,
    rids: &HashMap<String, String>,
    link_rids: &HashMap<String, String>,
    index: usize,
) -> Result<()> {
    let id = 100_000 + index as i64;
    match shape {
        Shape::Image(image) => {
            let Some(embed) = rids.get(&image.media_ref) else {
                return Ok(());
            };
            let name = format!("Picture {}", index + 1);
            let xml = pic_element_xml(image, embed, id, &name);
            writer.get_mut().write_all(xml.as_bytes())?;
        }
        Shape::Geometric(geo) => {
            let name = format!("Shape {}", index + 1);
            let xml = sp_element_xml(geo, id, &name);
            writer.get_mut().write_all(xml.as_bytes())?;
        }
        Shape::TextBox(text_box) => {
            let name = format!("TextBox {}", index + 1);
            let header = format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/>\
                 <p:cNvSpPr><a:spLocks/></p:cNvSpPr><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{xfrm}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr/><a:lstStyle/>",
                xfrm = xfrm_xml(&slides_core::Transform {
                    frame: text_box.frame,
                    rotation: 0.0,
                })
            );
            writer.get_mut().write_all(header.as_bytes())?;
            for paragraph in &text_box.paragraphs {
                write_paragraph(writer, paragraph, link_rids)?;
            }
            writer.get_mut().write_all(b"</p:txBody></p:sp>")?;
        }
        Shape::Passthrough(_) => {}
    }
    Ok(())
}

/// Formats an EMU/coordinate `f64` as a deterministic attribute string.
fn emu(value: f64) -> String {
    format!("{value}")
}

/// Formats a color as an uppercase `RRGGBB` hex string.
fn hex_color(color: &slides_core::Color) -> String {
    format!("{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

/// Builds the `<a:xfrm>` element for a transform.
fn xfrm_xml(transform: &Transform) -> String {
    let f = transform.frame;
    let rot = if transform.rotation != 0.0 {
        format!(
            " rot=\"{}\"",
            (transform.rotation * 60_000.0).round() as i64
        )
    } else {
        String::new()
    };
    format!(
        "<a:xfrm{rot}><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm>",
        emu(f.x),
        emu(f.y),
        emu(f.width),
        emu(f.height)
    )
}

/// Builds the `<a:prstGeom>` element for a geometric primitive.
fn prst_geom_xml(geometry: &Geometry, frame: slides_core::Rect) -> String {
    let prst = geometry::prst_from_geometry(geometry);
    if matches!(geometry, Geometry::RoundedRectangle { .. }) {
        if let Some(adj) = geometry::rounded_rect_adj(geometry, frame) {
            return format!(
                "<a:prstGeom prst=\"{prst}\"><a:avLst><a:gd name=\"adj\" fmla=\"val {adj}\"/></a:avLst></a:prstGeom>"
            );
        }
    }
    format!("<a:prstGeom prst=\"{prst}\"><a:avLst/></a:prstGeom>")
}

/// Builds a fill element, or an empty string when the shape has no fill.
fn fill_xml(fill: &Option<Fill>) -> String {
    match fill {
        Some(Fill::Solid(color)) => format!(
            "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            hex_color(color)
        ),
        None => String::new(),
    }
}

/// Builds an `<a:ln>` element, or an empty string when the shape has no outline.
fn outline_xml(outline: &Option<Outline>) -> String {
    let Some(o) = outline else {
        return String::new();
    };
    let dash = match o.dash {
        DashStyle::Solid => "solid",
        DashStyle::Dash => "dash",
        DashStyle::Dot => "dot",
        DashStyle::DashDot => "dashDot",
    };
    format!(
        "<a:ln w=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:prstDash val=\"{dash}\"/></a:ln>",
        emu(o.width_emu),
        hex_color(&o.color)
    )
}

/// Builds an `<a:effectLst>` with an outer shadow, or an empty string.
fn shadow_xml(shadow: &Option<slides_core::Shadow>) -> String {
    let Some(s) = shadow else {
        return String::new();
    };
    let dist = (s.offset_x.powi(2) + s.offset_y.powi(2)).sqrt();
    let dir_deg = s.offset_y.atan2(s.offset_x).to_degrees();
    let dir = (dir_deg * 60_000.0).round() as i64;
    let alpha = if (s.opacity - 1.0).abs() > f64::EPSILON {
        format!(
            "<a:alpha val=\"{}\"/>",
            (s.opacity * 100_000.0).round() as i64
        )
    } else {
        String::new()
    };
    format!(
        "<a:effectLst><a:outerShdw blurRad=\"{}\" dist=\"{}\" dir=\"{}\" rotWithShape=\"0\"><a:srgbClr val=\"{}\">{alpha}</a:srgbClr></a:outerShdw></a:effectLst>",
        emu(s.blur),
        emu(dist),
        dir,
        hex_color(&s.color)
    )
}

/// Builds the full `<p:spPr>` for an image.
fn image_sp_pr_xml(image: &ImageShape) -> String {
    format!(
        "<p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>",
        xfrm_xml(&image.transform)
    )
}

/// Builds the full `<p:spPr>` for a geometric shape.
fn geometric_sp_pr_xml(geo: &GeometricShape) -> String {
    let mut s = String::from("<p:spPr>");
    s.push_str(&xfrm_xml(&geo.transform));
    s.push_str(&prst_geom_xml(&geo.geometry, geo.transform.frame));
    s.push_str(&fill_xml(&geo.style.fill));
    s.push_str(&outline_xml(&geo.style.outline));
    s.push_str(&shadow_xml(&geo.style.shadow));
    s.push_str("</p:spPr>");
    s
}

/// Builds the `<p:blipFill>` element for an image, including a crop when one is
/// set.
fn blip_fill_xml(embed: &str, crop: Option<&Crop>) -> String {
    let src = crop
        .filter(|c| !is_zero_crop(c))
        .map(|c| {
            format!(
                "<a:srcRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/>",
                (c.left * 100_000.0).round() as i64,
                (c.top * 100_000.0).round() as i64,
                (c.right * 100_000.0).round() as i64,
                (c.bottom * 100_000.0).round() as i64
            )
        })
        .unwrap_or_default();
    format!("<p:blipFill><a:blip r:embed=\"{embed}\"/>{src}</p:blipFill>")
}

fn is_zero_crop(c: &Crop) -> bool {
    c.left == 0.0 && c.top == 0.0 && c.right == 0.0 && c.bottom == 0.0
}

/// Builds a complete `<p:pic>` element for an inserted image.
fn pic_element_xml(image: &ImageShape, embed: &str, id: i64, name: &str) -> String {
    format!(
        "<p:pic><p:nvPicPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvPicPr><a:picLocks/></p:cNvPicPr><p:nvPr/></p:nvPicPr>{}{}</p:pic>",
        blip_fill_xml(embed, image.crop.as_ref()),
        image_sp_pr_xml(image)
    )
}

/// Builds a complete `<p:sp>` element for an inserted geometric shape.
fn sp_element_xml(geo: &GeometricShape, id: i64, name: &str) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr><a:spLocks/></p:cNvSpPr><p:nvPr/></p:nvSpPr>{}<p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp>",
        geometric_sp_pr_xml(geo)
    )
}

fn write_patched_text_box<W: Write>(
    writer: &mut Writer<W>,
    xml: &str,
    text_box: &TextBox,
    link_rids: &HashMap<String, String>,
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
                        write_paragraph(writer, paragraph, link_rids)?;
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

fn write_paragraph<W: Write>(
    writer: &mut Writer<W>,
    paragraph: &Paragraph,
    link_rids: &HashMap<String, String>,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("a:p")))?;

    let mut ppr = BytesStart::new("a:pPr");
    let lvl_str;
    if paragraph.style.indent_level > 0 {
        lvl_str = paragraph.style.indent_level.to_string();
        ppr.push_attribute(("lvl", lvl_str.as_str()));
    }
    match paragraph.list_style {
        ListStyle::None => {
            writer.write_event(Event::Empty(ppr))?;
        }
        ListStyle::Ordered => {
            let mut num = BytesStart::new("a:buAutoNum");
            num.push_attribute(("type", "arabicParenR"));
            writer.write_event(Event::Start(ppr))?;
            writer.write_event(Event::Empty(num))?;
            writer.write_event(Event::End(BytesEnd::new("a:pPr")))?;
        }
        ListStyle::Unordered => {
            let mut bullet = BytesStart::new("a:buChar");
            bullet.push_attribute(("char", "•"));
            writer.write_event(Event::Start(ppr))?;
            writer.write_event(Event::Empty(bullet))?;
            writer.write_event(Event::End(BytesEnd::new("a:pPr")))?;
        }
    }

    for run in &paragraph.runs {
        write_run(writer, run, link_rids)?;
    }

    writer.write_event(Event::End(BytesEnd::new("a:p")))?;
    Ok(())
}

fn write_run<W: Write>(
    writer: &mut Writer<W>,
    run: &Run,
    link_rids: &HashMap<String, String>,
) -> Result<()> {
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
    if run.strikethrough {
        rpr.push_attribute(("strike", "sngStrike"));
    }
    let baseline_str;
    match run.vertical_align {
        VerticalAlign::Superscript => {
            baseline_str = "30000".to_string();
            rpr.push_attribute(("baseline", baseline_str.as_str()));
        }
        VerticalAlign::Subscript => {
            baseline_str = "-30000".to_string();
            rpr.push_attribute(("baseline", baseline_str.as_str()));
        }
        VerticalAlign::Baseline => {}
    }

    let has_link = run.link.is_some();
    let needs_rpr_inner = run.font_family.is_some() || has_link;
    if needs_rpr_inner {
        writer.write_event(Event::Start(rpr))?;
        if let Some(font) = &run.font_family {
            let mut latin = BytesStart::new("a:latin");
            latin.push_attribute(("typeface", font.as_str()));
            writer.write_event(Event::Empty(latin))?;
        }
        if let Some(link) = &run.link {
            if let Some(rid) = link_rids.get(&link.url) {
                let mut hlink = BytesStart::new("a:hlinkClick");
                hlink.push_attribute(("r:id", rid.as_str()));
                writer.write_event(Event::Empty(hlink))?;
            }
        }
        writer.write_event(Event::End(BytesEnd::new("a:rPr")))?;
    } else {
        writer.write_event(Event::Empty(rpr))?;
    }

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
