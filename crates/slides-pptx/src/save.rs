//! PPTX saving: regenerate only edited parts, copy everything else verbatim.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};

use quick_xml::events::{BytesCData, BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use slides_core::{
    Animation, BorderEdge, BuildEffect, BuildStep, CellAlign, Crop, DashStyle, Fill,
    GeometricShape, Geometry, ImageShape, ListStyle, Outline, Paragraph, Run, Shape, Slide,
    TableCell, TableShape, TextBox, Transform, Transition, TransitionKind, VerticalAlign,
};
use zip::write::{FileOptions, ZipWriter};

use crate::chart::{
    chart_graphic_frame_xml, chart_part_path, generate_chart_xml, is_chart_frame, next_chart_index,
    parse_chart_xml, patch_chart_xml, CT_CHART,
};
use crate::error::{Error, Result};
use crate::geometry;
use crate::load::{
    attr_by_local_name, copy_element, extract_blip_embed, rels_path_for, SHAPE_ELEMENT_NAMES,
};
use crate::media as pkgmedia;
use crate::package::{
    parse_rels, write_content_types, write_rels, Rel, CT_MANIFEST, REL_TYPE_CHART,
    REL_TYPE_HYPERLINK, REL_TYPE_IMAGE, REL_TYPE_MANIFEST, REL_TYPE_OFFICE_DOCUMENT,
    REL_TYPE_SLIDE, REL_TYPE_SLIDE_LAYOUT,
};
use crate::session::Session;

const MANIFEST_NS: &str = "http://900labs.github.io/900Slides/1.0";
const CT_SLIDE: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";

const BLANK_SLIDE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="0" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld></p:sld>"#;

/// Serializes the current deck to a PPTX package, preserving every untouched
/// part byte-for-byte.
pub fn save(session: &Session) -> Result<Vec<u8>> {
    let source_bytes = session.source_bytes_for_save()?;
    let mut archive = zip::ZipArchive::new(Cursor::new(source_bytes.as_ref()))?;
    let mut out = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(&mut out);
    let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Deflated);

    let mut content_types = session.content_types.clone();
    content_types.ensure_override(&session.manifest_path, CT_MANIFEST);

    let need_manifest_rel = session.manifest_rel_id.is_none();

    // Existing slides retain their original part paths. Allocate parts for
    // inserted slides in deck order, choosing the next free conventional
    // `slide<N>.xml` name so that package output is deterministic and never
    // overwrites an untouched part.
    let mut all_slide_paths = session.slide_paths.clone();
    let mut new_slide_ids = Vec::new();
    let mut next_slide_index = next_slide_part_index(&mut archive)?;
    for slide in &session.deck.slides {
        if !all_slide_paths.contains_key(&slide.id) {
            let mut path = format!("ppt/slides/slide{next_slide_index}.xml");
            while all_slide_paths.values().any(|existing| existing == &path) {
                next_slide_index += 1;
                path = format!("ppt/slides/slide{next_slide_index}.xml");
            }
            next_slide_index += 1;
            all_slide_paths.insert(slide.id.clone(), path);
            new_slide_ids.push(slide.id.clone());
        }
    }
    for slide_id in session.deck.slides.iter().map(|slide| &slide.id) {
        let path = all_slide_paths
            .get(slide_id)
            .expect("new slide path is allocated above");
        content_types.ensure_override(path, CT_SLIDE);
    }

    let mut dirty_slide_ids: HashSet<String> = session
        .dirty_slides
        .iter()
        .filter(|id| session.deck.slide(id).is_some())
        .cloned()
        .collect();
    dirty_slide_ids.extend(new_slide_ids.iter().cloned());
    let dirty_paths: HashSet<String> = dirty_slide_ids
        .iter()
        .filter_map(|id| all_slide_paths.get(id).cloned())
        .collect();
    let shape_save = prepare_shape_ids(session, &mut archive, &all_slide_paths, &dirty_slide_ids)?;
    let manifest_xml = write_manifest(session, &all_slide_paths, &shape_save.model_ids);

    // A presentation part only needs rewriting for a structural mutation. This
    // covers insertion, undoing a persisted insertion, and future reordering;
    // ordinary edit saves still keep it byte-for-byte as before.
    let has_added_or_removed_slide = !new_slide_ids.is_empty()
        || session
            .slide_paths
            .keys()
            .any(|id| session.deck.slide(id).is_none());
    let mut presentation_save =
        match prepare_presentation_save(session, &mut archive, &all_slide_paths) {
            Ok(state) => state,
            // Some narrow synthetic fixtures model individual slide XML parts
            // without a complete package relationship graph. They cannot carry
            // a structural delta, so retain their legacy lossless save path.
            Err(Error::MissingRelationship(_)) if !has_added_or_removed_slide => None,
            Err(error) => return Err(error),
        };
    if let Some(state) = &presentation_save {
        for path in &state.removed_slide_paths {
            content_types.overrides.remove(&format!("/{path}"));
        }
    }

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

    for slide_id in &dirty_slide_ids {
        let Some(slide_path) = all_slide_paths.get(slide_id) else {
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

    // Pre-pass for charts: allocate part paths and relationship ids for charts on
    // dirty slides, and pre-patch dirty existing chart XML parts.
    let mut chart_save_state = prepare_charts(
        session,
        &mut archive,
        &all_slide_paths,
        &dirty_slide_ids,
        &dirty_paths,
        &mut slide_rels_additions,
        &mut content_types,
    )?;

    // Every new slide needs the same layout relationship as a neighboring
    // template slide. Copy the resolved target rather than a raw relative path
    // so this also works for packages whose slides live outside ppt/slides.
    add_new_slide_layout_relationships(
        session,
        &mut archive,
        &all_slide_paths,
        &new_slide_ids,
        &mut slide_rels_additions,
    )?;

    let mut manifest_seen = false;
    let mut content_types_seen = false;
    let mut rels_seen = false;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        if presentation_save.as_ref().is_some_and(|state| {
            state.removed_slide_paths.contains(&name)
                || state.removed_slide_rels_paths.contains(&name)
        }) {
            continue;
        } else if name == "[Content_Types].xml" {
            let xml = write_content_types(&content_types)?;
            writer.start_file(&name, options)?;
            writer.write_all(&xml)?;
            content_types_seen = true;
        } else if name == session.manifest_path {
            writer.start_file(&name, options)?;
            writer.write_all(&manifest_xml)?;
            manifest_seen = true;
        } else if presentation_save
            .as_ref()
            .is_some_and(|state| name == state.presentation_path)
        {
            let state = presentation_save
                .as_mut()
                .expect("checked presentation save state above");
            let mut original_xml = String::new();
            entry.read_to_string(&mut original_xml)?;
            let xml = patch_presentation_slide_list(&original_xml, &state.entries)?;
            writer.start_file(&name, options)?;
            writer.write_all(&xml)?;
            state.presentation_seen = true;
        } else if presentation_save
            .as_ref()
            .is_some_and(|state| name == state.presentation_rels_path)
        {
            let state = presentation_save
                .as_mut()
                .expect("checked presentation save state above");
            let xml = write_rels(&state.rels)?;
            writer.start_file(&name, options)?;
            writer.write_all(&xml)?;
            state.presentation_rels_seen = true;
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
        } else if let Some(bytes) = chart_save_state.patched_chart_parts.remove(&name) {
            writer.start_file(&name, options)?;
            writer.write_all(&bytes)?;
        } else if let Some(slide) =
            find_slide_by_path(session, &all_slide_paths, &dirty_paths, &name)
        {
            let export_slide = shape_save.normalized_slide(slide);
            let slide = &export_slide;
            let mut original_xml = String::new();
            entry.read_to_string(&mut original_xml)?;
            let rids = slide_rids
                .get(slide.id.as_str())
                .cloned()
                .unwrap_or_default();
            let chart_rids = chart_save_state
                .chart_rids
                .get(slide.id.as_str())
                .cloned()
                .unwrap_or_default();
            let rels_path = rels_path_for(&name);
            let link_rids = slide_link_rids.get(&rels_path).cloned().unwrap_or_default();
            let xml = patch_slide_xml(slide, &original_xml, &rids, &link_rids, &chart_rids)?;
            writer.start_file(&name, options)?;
            writer.write_all(&xml)?;
        } else {
            writer.raw_copy_file(entry)?;
        }
    }

    // Write new slide parts after the copied original archive. Their optional
    // relationship files were collected by the same pre-passes as existing
    // slides, so images, charts, and hyperlinks are valid on first save.
    for slide_id in &new_slide_ids {
        let slide = session
            .deck
            .slide(slide_id)
            .expect("new slide remains in the deck while saving");
        let export_slide = shape_save.normalized_slide(slide);
        let slide = &export_slide;
        let path = all_slide_paths
            .get(slide_id)
            .expect("new slide path is allocated above");
        let rids = slide_rids.get(slide_id).cloned().unwrap_or_default();
        let chart_rids = chart_save_state
            .chart_rids
            .get(slide_id)
            .cloned()
            .unwrap_or_default();
        let link_rids = slide_link_rids
            .get(&rels_path_for(path))
            .cloned()
            .unwrap_or_default();
        let xml = patch_slide_xml(slide, BLANK_SLIDE_XML, &rids, &link_rids, &chart_rids)?;
        writer.start_file(path.as_str(), options)?;
        writer.write_all(&xml)?;
    }

    // Write brand-new media parts (inserted images), chart parts, and any slide
    // rels files that did not previously exist.
    for (part, bytes) in &new_media_parts {
        writer.start_file(part.as_str(), options)?;
        writer.write_all(bytes)?;
    }
    for (part, bytes) in &chart_save_state.new_chart_parts {
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

    if let Some(state) = presentation_save {
        if !state.presentation_seen {
            return Err(Error::MissingPart(state.presentation_path));
        }
        if !state.presentation_rels_seen {
            return Err(Error::MissingPart(state.presentation_rels_path));
        }
    }

    writer.finish()?;
    Ok(out.into_inner())
}

/// State accumulated while preparing charts for save.
struct ShapeSaveState {
    package_ids: HashMap<String, Vec<String>>,
    model_ids: HashMap<String, HashMap<String, String>>,
}

impl ShapeSaveState {
    fn normalized_slide(&self, slide: &Slide) -> Slide {
        let mut normalized = slide.clone();
        if let Some(ids) = self.package_ids.get(&slide.id) {
            for (shape, id) in normalized.shapes.iter_mut().zip(ids) {
                shape.set_id(id.clone());
            }
        }
        normalized
    }
}

/// Allocates positive numeric OOXML IDs while retaining model UUIDs in the
/// manifest. Reserve all source IDs, including children of opaque groups, so a
/// newly inserted shape cannot collide with preserved XML or animation targets.
fn prepare_shape_ids(
    session: &Session,
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    slide_paths: &HashMap<String, String>,
    dirty_slide_ids: &HashSet<String>,
) -> Result<ShapeSaveState> {
    let mut state = ShapeSaveState {
        package_ids: HashMap::new(),
        model_ids: session.shape_package_ids.clone(),
    };
    for slide in &session.deck.slides {
        if !dirty_slide_ids.contains(&slide.id) {
            continue;
        }
        let path = &slide_paths[&slide.id];
        let original = crate::load::read_entry_to_string(archive, path).unwrap_or_default();
        let original_ids = build_original_id_to_index(&original);
        let mut reserved: HashSet<u32> = HashSet::from([0, 1]);
        reserve_shape_ids(&original, &mut reserved)?;
        for shape in &slide.shapes {
            if let Shape::Passthrough(object) = shape {
                reserve_shape_ids(&String::from_utf8_lossy(&object.raw_bytes), &mut reserved)?;
            }
        }
        let mut used = reserved.clone();
        used.extend(
            slide
                .shapes
                .iter()
                .filter_map(|shape| shape.id().parse::<u32>().ok()),
        );
        if let Some(ids) = state.model_ids.get(&slide.id) {
            used.extend(ids.values().filter_map(|id| id.parse::<u32>().ok()));
        }
        let mut assigned = HashSet::new();
        let mut next_id = 2u32;
        let mut package_ids = Vec::with_capacity(slide.shapes.len());
        let model_ids = state.model_ids.entry(slide.id.clone()).or_default();
        for (index, shape) in slide.shapes.iter().enumerate() {
            let existing = model_ids
                .get(shape.id())
                .cloned()
                .or_else(|| match shape {
                    Shape::Passthrough(object) => {
                        extract_shape_id(&String::from_utf8_lossy(&object.raw_bytes))
                    }
                    _ => None,
                })
                .or_else(|| {
                    if shape.id().is_empty() {
                        original_ids
                            .iter()
                            .find(|(_, source_index)| **source_index == index)
                            .map(|(id, _)| id.clone())
                    } else {
                        shape
                            .id()
                            .parse::<u32>()
                            .ok()
                            .filter(|id| {
                                !reserved.contains(id) || original_ids.contains_key(shape.id())
                            })
                            .map(|_| shape.id().to_string())
                    }
                })
                .filter(|id| {
                    id.parse::<u32>()
                        .is_ok_and(|id| id > 0 && !assigned.contains(&id))
                });
            let package_id = if let Some(id) = existing {
                id
            } else {
                while used.contains(&next_id) {
                    next_id = next_id.checked_add(1).ok_or_else(|| {
                        Error::Save("slide has exhausted numeric shape identifiers".into())
                    })?;
                }
                next_id.to_string()
            };
            let numeric_id = package_id
                .parse::<u32>()
                .expect("allocated positive numeric ID");
            assigned.insert(numeric_id);
            used.insert(numeric_id);
            if !shape.id().is_empty() {
                model_ids.insert(shape.id().to_string(), package_id.clone());
            }
            package_ids.push(package_id);
        }
        state.package_ids.insert(slide.id.clone(), package_ids);
    }
    Ok(state)
}

fn reserve_shape_ids(xml: &str, reserved: &mut HashSet<u32>) -> Result<()> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) if qname_str(e.name()) == "cNvPr" => {
                if let Some(id) = attr_by_local_name(&e, "id").and_then(|id| id.parse::<u32>().ok())
                {
                    reserved.insert(id);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

/// State accumulated while preparing charts for save.
struct ChartSaveState {
    /// Patched bytes for dirty existing chart parts, keyed by part path.
    patched_chart_parts: HashMap<String, Vec<u8>>,
    /// (slide_id, shape_index) -> relationship id for all charts on dirty slides.
    chart_rids: HashMap<String, HashMap<usize, String>>,
    /// New chart parts to write, keyed by part path.
    new_chart_parts: Vec<(String, Vec<u8>)>,
}

/// Prepares chart parts for save: allocates new chart parts/relationships and
/// pre-patches dirty existing chart XML. Existing chart parts that are not dirty
/// are left untouched and will be copied byte-for-byte from the original archive.
fn prepare_charts(
    session: &Session,
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    slide_paths: &HashMap<String, String>,
    dirty_slide_ids: &HashSet<String>,
    dirty_paths: &HashSet<String>,
    slide_rels_additions: &mut HashMap<String, Vec<Rel>>,
    content_types: &mut crate::package::ContentTypes,
) -> Result<ChartSaveState> {
    let mut state = ChartSaveState {
        patched_chart_parts: HashMap::new(),
        chart_rids: HashMap::new(),
        new_chart_parts: Vec::new(),
    };
    let mut chart_counter = next_chart_index(archive);

    for slide_id in dirty_slide_ids {
        let Some(slide_path) = slide_paths.get(slide_id) else {
            continue;
        };
        if !dirty_paths.contains(slide_path) {
            continue;
        }
        let Some(slide) = session.deck.slides.iter().find(|s| &s.id == slide_id) else {
            continue;
        };

        let rels_path = rels_path_for(slide_path);
        let existing_rels = match crate::load::read_entry_to_string(archive, &rels_path) {
            Ok(xml) => parse_rels(&xml).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        let base_max = max_rel_number(&existing_rels);
        let additions = slide_rels_additions
            .get(&rels_path)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let additions_max = max_rel_number(additions);
        let mut max_rid = base_max.max(additions_max);

        for (shape_index, shape) in slide.shapes.iter().enumerate() {
            let Shape::Chart(chart) = shape else {
                continue;
            };

            if let Some(part_path) = session
                .chart_source_parts
                .get(slide_id)
                .and_then(|m| m.get(&chart.id))
            {
                // Existing chart: reuse relationship id.
                let rid = session
                    .slide_chart_rids
                    .get(slide_id)
                    .and_then(|m| m.get(part_path))
                    .cloned()
                    .unwrap_or_else(|| {
                        max_rid += 1;
                        let rid = format!("rId{max_rid}");
                        slide_rels_additions
                            .entry(rels_path.clone())
                            .or_default()
                            .push(Rel {
                                id: rid.clone(),
                                rel_type: REL_TYPE_CHART.to_string(),
                                target: pkgmedia::relative_target(slide_path, part_path),
                                target_mode: None,
                            });
                        rid
                    });
                state
                    .chart_rids
                    .entry(slide_id.clone())
                    .or_default()
                    .insert(shape_index, rid);

                if let Some(original) = session.original_chart_bytes.get(part_path) {
                    let original_str = String::from_utf8_lossy(original);
                    // Compare against the saved content, rather than the last
                    // executed command, so undo/redo across saves is persisted.
                    // A moved chart keeps its part byte-for-byte unchanged.
                    let unchanged =
                        parse_chart_xml(&original_str, chart.transform).is_some_and(|saved| {
                            saved.chart_type == chart.chart_type
                                && saved.data == chart.data
                                && saved.title == chart.title
                        });
                    if !unchanged {
                        match patch_chart_xml(&original_str, chart) {
                            Ok(bytes) => {
                                state.patched_chart_parts.insert(part_path.clone(), bytes);
                            }
                            Err(_) => {
                                // Fallback: generate fresh chart XML.
                                state
                                    .patched_chart_parts
                                    .insert(part_path.clone(), generate_chart_xml(chart));
                            }
                        }
                    }
                }
            } else {
                // Newly inserted chart: allocate a part and relationship.
                let part_index = chart_counter;
                chart_counter += 1;
                let part_path = chart_part_path(part_index);
                let bytes = generate_chart_xml(chart);
                max_rid += 1;
                let rid = format!("rId{max_rid}");
                state.new_chart_parts.push((part_path.clone(), bytes));
                state
                    .chart_rids
                    .entry(slide_id.clone())
                    .or_default()
                    .insert(shape_index, rid.clone());
                slide_rels_additions
                    .entry(rels_path.clone())
                    .or_default()
                    .push(Rel {
                        id: rid,
                        rel_type: REL_TYPE_CHART.to_string(),
                        target: pkgmedia::relative_target(slide_path, &part_path),
                        target_mode: None,
                    });
                content_types.ensure_override(&part_path, CT_CHART);
            }
        }
    }

    Ok(state)
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
    slide_paths: &HashMap<String, String>,
    dirty_paths: &HashSet<String>,
    path: &str,
) -> Option<&'a Slide> {
    if !dirty_paths.contains(path) {
        return None;
    }
    slide_paths
        .iter()
        .find(|(_, p)| *p == path)
        .and_then(|(id, _)| session.deck.slides.iter().find(|s| s.id == *id))
}

/// The presentation package state that changes when the modeled slide
/// structure differs from the package (insert, remove, or reorder).
struct PresentationSaveState {
    presentation_path: String,
    presentation_rels_path: String,
    rels: Vec<Rel>,
    entries: Vec<PresentationSlideEntry>,
    removed_slide_paths: HashSet<String>,
    removed_slide_rels_paths: HashSet<String>,
    presentation_seen: bool,
    presentation_rels_seen: bool,
}

#[derive(Debug, Clone)]
struct PresentationSlideEntry {
    id: u32,
    rid: String,
}

/// Produces package changes for a structurally changed deck, or no state when
/// the existing presentation ordering already matches the model.
fn prepare_presentation_save(
    session: &Session,
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    slide_paths: &HashMap<String, String>,
) -> Result<Option<PresentationSaveState>> {
    let presentation_path = session
        .package_rels
        .iter()
        .find(|rel| rel.rel_type == REL_TYPE_OFFICE_DOCUMENT)
        .and_then(|rel| rel.resolve(""))
        .ok_or_else(|| Error::MissingRelationship(REL_TYPE_OFFICE_DOCUMENT.to_string()))?;
    let presentation_rels_path = rels_path_for(&presentation_path);
    let presentation_xml = crate::load::read_entry_to_string(archive, &presentation_path)?;
    let presentation_entries = parse_presentation_slide_entries(&presentation_xml)?;
    let mut rels = parse_rels(&crate::load::read_entry_to_string(
        archive,
        &presentation_rels_path,
    )?)?;
    let base_dir = part_base_dir(&presentation_path);
    let path_by_rid: HashMap<String, String> = rels
        .iter()
        .filter(|rel| rel.rel_type == REL_TYPE_SLIDE)
        .filter_map(|rel| rel.resolve(&base_dir).map(|path| (rel.id.clone(), path)))
        .collect();
    let entry_id_by_rid: HashMap<String, u32> = presentation_entries
        .iter()
        .map(|entry| (entry.rid.clone(), entry.id))
        .collect();
    let rid_by_path: HashMap<String, String> = path_by_rid
        .iter()
        .map(|(rid, path)| (path.clone(), rid.clone()))
        .collect();
    let current_paths: Vec<String> = session
        .deck
        .slides
        .iter()
        .filter_map(|slide| slide_paths.get(&slide.id).cloned())
        .collect();
    let original_paths: Vec<String> = presentation_entries
        .iter()
        .filter_map(|entry| path_by_rid.get(&entry.rid).cloned())
        .collect();
    let active_paths: HashSet<String> = current_paths.iter().cloned().collect();
    let removed_slide_paths: HashSet<String> = original_paths
        .iter()
        .filter(|path| !active_paths.contains(*path))
        .cloned()
        .collect();
    let structural_change = current_paths != original_paths || !removed_slide_paths.is_empty();
    if !structural_change {
        return Ok(None);
    }
    let removed_slide_rels_paths = removed_slide_paths
        .iter()
        .map(|path| rels_path_for(path))
        .collect();

    // Drop relationships to slides no longer present in the model before
    // adding relationships for newly allocated parts.
    rels.retain(|rel| {
        rel.rel_type != REL_TYPE_SLIDE
            || rel
                .resolve(&base_dir)
                .is_some_and(|path| active_paths.contains(&path))
    });
    let rid_by_path: HashMap<String, String> = rid_by_path
        .into_iter()
        .filter(|(path, _)| active_paths.contains(path))
        .collect();
    let mut next_slide_id = presentation_entries
        .iter()
        .map(|entry| entry.id)
        .max()
        .unwrap_or(255)
        .saturating_add(1)
        .max(256);
    let mut entries = Vec::with_capacity(session.deck.slides.len());

    for slide in &session.deck.slides {
        let path = slide_paths
            .get(&slide.id)
            .ok_or_else(|| Error::Save(format!("missing package path for slide '{}'", slide.id)))?;
        if let Some(rid) = rid_by_path.get(path) {
            let id = entry_id_by_rid.get(rid).copied().unwrap_or_else(|| {
                let id = next_slide_id;
                next_slide_id = next_slide_id.saturating_add(1);
                id
            });
            entries.push(PresentationSlideEntry {
                id,
                rid: rid.clone(),
            });
        } else {
            let rid = next_rel_id(&rels);
            rels.push(Rel {
                id: rid.clone(),
                rel_type: REL_TYPE_SLIDE.to_string(),
                target: pkgmedia::relative_target(&presentation_path, path),
                target_mode: None,
            });
            entries.push(PresentationSlideEntry {
                id: next_slide_id,
                rid,
            });
            next_slide_id = next_slide_id.saturating_add(1);
        }
    }

    Ok(Some(PresentationSaveState {
        presentation_path,
        presentation_rels_path,
        rels,
        entries,
        removed_slide_paths,
        removed_slide_rels_paths,
        presentation_seen: false,
        presentation_rels_seen: false,
    }))
}

/// Replaces only the presentation's ordered slide-id list, retaining all other
/// presentation XML (masters, notes properties, extension lists) unchanged.
fn patch_presentation_slide_list(
    original_xml: &str,
    entries: &[PresentationSlideEntry],
) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut writer = Writer::new(&mut output);
    let mut reader = Reader::from_str(original_xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut replaced = false;

    loop {
        let event = reader.read_event_into(&mut buf)?;
        match &event {
            Event::Start(start) if qname_str(start.name()) == "sldIdLst" => {
                writer.write_event(Event::Start(start.clone().into_owned()))?;
                write_presentation_slide_entries(&mut writer, entries)?;
                skip_subtree(&mut reader, &mut buf)?;
                writer.write_event(Event::End(BytesEnd::new("p:sldIdLst")))?;
                replaced = true;
            }
            Event::Empty(start) if qname_str(start.name()) == "sldIdLst" => {
                writer.write_event(Event::Start(start.clone().into_owned()))?;
                write_presentation_slide_entries(&mut writer, entries)?;
                writer.write_event(Event::End(BytesEnd::new("p:sldIdLst")))?;
                replaced = true;
            }
            Event::Eof => break,
            _ => writer.write_event(event)?,
        }
        buf.clear();
    }

    if !replaced {
        return Err(Error::MissingPart("p:sldIdLst".to_string()));
    }
    Ok(output)
}

fn write_presentation_slide_entries<W: Write>(
    writer: &mut Writer<W>,
    entries: &[PresentationSlideEntry],
) -> Result<()> {
    for entry in entries {
        let mut element = BytesStart::new("p:sldId");
        let id = entry.id.to_string();
        element.push_attribute(("id", id.as_str()));
        element.push_attribute(("r:id", entry.rid.as_str()));
        writer.write_event(Event::Empty(element))?;
    }
    Ok(())
}

fn parse_presentation_slide_entries(xml: &str) -> Result<Vec<PresentationSlideEntry>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut entries = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(start) | Event::Empty(start) if qname_str(start.name()) == "sldId" => {
                let id = attribute_by_name(&start, "id")
                    .and_then(|value| value.parse::<u32>().ok())
                    .ok_or(Error::InvalidAttribute)?;
                let rid = attribute_by_name(&start, "r:id").ok_or(Error::InvalidAttribute)?;
                entries.push(PresentationSlideEntry { id, rid });
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(entries)
}

fn attribute_by_name(start: &BytesStart<'_>, name: &str) -> Option<String> {
    start.attributes().flatten().find_map(|attribute| {
        (attribute.key.as_ref() == name.as_bytes())
            .then(|| {
                attribute
                    .unescape_value()
                    .ok()
                    .map(|value| value.into_owned())
            })
            .flatten()
    })
}

fn part_base_dir(part: &str) -> String {
    std::path::Path::new(part)
        .parent()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

/// Returns the first unused conventional slide part index.
fn next_slide_part_index(archive: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Result<usize> {
    let mut max_index = 0usize;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if let Some(rest) = entry.name().strip_prefix("ppt/slides/slide") {
            if let Some(number) = rest.strip_suffix(".xml") {
                if let Ok(number) = number.parse::<usize>() {
                    max_index = max_index.max(number);
                }
            }
        }
    }
    Ok(max_index.saturating_add(1))
}

/// Adds a slideLayout relationship to each newly created slide by reusing the
/// nearest loaded slide's layout. OOXML requires a slide to reference a layout;
/// omitting it produces packages that some Office clients repair or reject.
fn add_new_slide_layout_relationships(
    session: &Session,
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    slide_paths: &HashMap<String, String>,
    new_slide_ids: &[String],
    additions: &mut HashMap<String, Vec<Rel>>,
) -> Result<()> {
    for new_id in new_slide_ids {
        let new_index = session
            .deck
            .slides
            .iter()
            .position(|slide| &slide.id == new_id)
            .ok_or_else(|| {
                Error::Save(format!(
                    "inserted slide '{new_id}' is missing from the deck"
                ))
            })?;
        let source_id = session.deck.slides[..new_index]
            .iter()
            .rev()
            .chain(session.deck.slides[new_index + 1..].iter())
            .find(|slide| session.slide_paths.contains_key(&slide.id))
            .map(|slide| slide.id.as_str())
            .ok_or_else(|| {
                Error::Save(
                    "cannot add a slide because this package has no existing slide layout to copy"
                        .to_string(),
                )
            })?;
        let source_path = session
            .slide_paths
            .get(source_id)
            .expect("source id was selected from the path map");
        let source_rels_path = rels_path_for(source_path);
        let source_rels = parse_rels(&crate::load::read_entry_to_string(
            archive,
            &source_rels_path,
        )?)?;
        let layout = source_rels
            .iter()
            .find(|rel| rel.rel_type == REL_TYPE_SLIDE_LAYOUT)
            .ok_or_else(|| {
                Error::Save(format!(
                    "cannot add a slide because source slide '{source_path}' has no slide layout relationship"
                ))
            })?;
        let layout_target = layout.resolve(&part_base_dir(source_path)).ok_or_else(|| {
            Error::Save("source slide layout relationship is external".to_string())
        })?;
        let new_path = slide_paths
            .get(new_id)
            .expect("new slide path was allocated before saving");
        let new_rels_path = rels_path_for(new_path);
        let existing = match crate::load::read_entry_to_string(archive, &new_rels_path) {
            Ok(xml) => parse_rels(&xml).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        let existing_additions = additions
            .get(&new_rels_path)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let next = max_rel_number(&existing).max(max_rel_number(existing_additions)) + 1;
        additions.entry(new_rels_path).or_default().push(Rel {
            id: format!("rId{next}"),
            rel_type: REL_TYPE_SLIDE_LAYOUT.to_string(),
            target: pkgmedia::relative_target(new_path, &layout_target),
            target_mode: None,
        });
    }
    Ok(())
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

fn write_manifest(
    session: &Session,
    slide_paths: &HashMap<String, String>,
    shape_ids: &HashMap<String, HashMap<String, String>>,
) -> Vec<u8> {
    let mut comments = session.deck.comments.clone();
    for thread in &mut comments {
        let (slides_core::CommentAnchor::Slide { slide_id }
        | slides_core::CommentAnchor::Shape { slide_id, .. }
        | slides_core::CommentAnchor::TextRange { slide_id, .. }) = &mut thread.anchor;
        if let Some(path) = slide_paths.get(slide_id) {
            *slide_id = path.clone();
        }
    }
    let mut sections = session.deck.sections.clone();
    for section in &mut sections {
        if let Some(path) = slide_paths.get(&section.start_slide_id) {
            section.start_slide_id = path.clone();
        }
    }
    let mut collections = Vec::new();
    if !comments.is_empty() {
        collections.push((
            "comments",
            serde_json::to_string(&comments).unwrap_or_else(|_| "[]".into()),
        ));
    }
    if !sections.is_empty() {
        collections.push((
            "sections",
            serde_json::to_string(&sections).unwrap_or_else(|_| "[]".into()),
        ));
    }
    let mut mappings = Vec::new();
    for slide in &session.deck.slides {
        if let (Some(path), Some(ids)) = (slide_paths.get(&slide.id), shape_ids.get(&slide.id)) {
            for shape in &slide.shapes {
                if let Some(package_id) = ids.get(shape.id()) {
                    mappings.push(crate::session::ShapeIdMapping {
                        slide_path: path.clone(),
                        model_id: shape.id().to_string(),
                        package_id: package_id.clone(),
                    });
                }
            }
        }
    }
    mappings.sort_by(|left, right| {
        (&left.slide_path, &left.model_id).cmp(&(&right.slide_path, &right.model_id))
    });
    if !mappings.is_empty() {
        collections.push((
            "shapeIds",
            serde_json::to_string(&mappings).unwrap_or_else(|_| "[]".into()),
        ));
    }
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

        if collections.is_empty() {
            // No comments: emit the same self-closing element as before so the
            // byte-for-byte round-trip guarantee holds for untouched decks.
            writer.write_event(Event::Empty(elem)).ok();
        } else {
            writer.write_event(Event::Start(elem)).ok();
            // Serialize the thread list to JSON and embed it verbatim inside a
            // CDATA section of <comments>. This keeps complex nested comment
            // data out of hand-written XML while staying inside the manifest.
            for (name, json) in collections {
                // Split on the CDATA terminator so a comment body containing the
                // literal `]]>` cannot corrupt the manifest XML.
                let safe = json.replace("]]>", "]]]]><![CDATA[>");
                writer.write_event(Event::Start(BytesStart::new(name))).ok();
                writer
                    .write_event(Event::CData(BytesCData::new(safe.as_str())))
                    .ok();
                writer.write_event(Event::End(BytesEnd::new(name))).ok();
            }
            writer
                .write_event(Event::End(BytesEnd::new("manifest")))
                .ok();
        }
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
    chart_rids: &HashMap<usize, String>,
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
                            write_model_shape(
                                &mut writer,
                                shape,
                                rids,
                                link_rids,
                                chart_rids,
                                index,
                            )?;
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
    chart_rids: &HashMap<usize, String>,
    index: usize,
) -> Result<()> {
    let id = cnvpr_id_for(shape, index);
    match shape {
        Shape::Image(image) => {
            if let Some(embed) = rids.get(&image.media_ref) {
                let name = format!("Picture {}", index + 1);
                let xml = pic_element_xml(image, embed, &id, &name);
                writer.get_mut().write_all(xml.as_bytes())?;
            }
        }
        Shape::Geometric(geo) => {
            let name = format!("Shape {}", index + 1);
            let xml = sp_element_xml(geo, &id, &name);
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
        Shape::Table(table) => {
            let name = format!("Table {}", index + 1);
            let xml = table_graphic_frame_xml(table, &id, &name);
            writer.get_mut().write_all(xml.as_bytes())?;
        }
        Shape::Chart(chart) => {
            if let Some(rid) = chart_rids.get(&index) {
                let name = format!("Chart {}", index + 1);
                let xml = chart_graphic_frame_xml(chart, &id, &name, rid);
                writer.get_mut().write_all(xml.as_bytes())?;
            }
        }
    }
    Ok(())
}

/// Patches a captured `<p:graphicFrame>` that represents a chart. Since all chart
/// data lives in a separate chart XML part, the slide graphicFrame is written
/// back byte-for-byte.
fn patch_chart_frame_xml<W: Write>(
    writer: &mut Writer<W>,
    captured: &[u8],
    _shape: &Shape,
    _chart_rids: &HashMap<usize, String>,
    _index: usize,
) -> Result<()> {
    writer.get_mut().write_all(captured)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Transition and animation patching (Wave 5 component 5)
// ---------------------------------------------------------------------------

/// Returns the generated `p:cNvPr` id for a model shape at `index`.
///
/// The saver emits all model shapes with ids starting at `100_000`, so the
/// generated timing XML targets the same ids to keep the loader's document-
/// order resolution self-consistent.
fn shape_id_for_index(index: usize) -> String {
    (100_000 + index as i64).to_string()
}

/// Returns the `p:cNvPr` id to emit for `shape` at `index`.
///
/// Prefers the shape's own stable id (preserved on round-trip for Magic Move
/// matching); falls back to the generated index-based id when the shape has no
/// id yet. The timing XML targets the same value via [`build_step_xml`] so build
/// steps resolve to the correct shape after a save.
fn cnvpr_id_for(shape: &Shape, index: usize) -> String {
    let id = shape.id();
    if id.is_empty() {
        shape_id_for_index(index)
    } else {
        id.to_string()
    }
}

/// Maps a model [`BuildEffect`] to the `filter` value used inside `p:animEffect`.
fn filter_for_effect(effect: BuildEffect) -> &'static str {
    match effect {
        BuildEffect::Fade => "fade",
        BuildEffect::SlideInLeft => "wipe(left)",
        BuildEffect::SlideInRight => "wipe(right)",
        BuildEffect::SlideInTop => "wipe(up)",
        BuildEffect::SlideInBottom => "wipe(down)",
        BuildEffect::Appear => "appear",
        BuildEffect::Disappear => "disappear",
        // The PPTX writer has no animEffect for motion paths yet; fall back to
        // fade so the timing XML stays well-formed. Motion-path export is out
        // of scope for the Wave 19 model component.
        BuildEffect::MotionPath => "fade",
    }
}

/// Returns true when the model transition differs from the original XML.
fn transition_changed(slide: &Slide, original_xml: &str) -> bool {
    slide.transition != extract_original_transition(original_xml)
}

/// Returns true when the model animation differs from the original XML.
fn animation_changed(slide: &Slide, original_xml: &str) -> bool {
    slide.animation != extract_original_animation(original_xml)
}

/// Extracts the first `p:transition` element from `xml` and parses it with the
/// loader's transition parser.
fn extract_original_transition(xml: &str) -> Option<Transition> {
    let transition_xml = capture_element_xml(xml, "transition")?;
    let mut ledger = crate::ledger::LossLedger::new();
    crate::transition::parse_transition(&transition_xml, "", &mut ledger)
}

/// Extracts the first `p:timing` element from `xml` and parses it with the
/// loader's simple build-in parser, resolving original `p:cNvPr` ids to
/// original shape indices.
fn extract_original_animation(xml: &str) -> Option<Animation> {
    let timing_xml = capture_element_xml(xml, "timing")?;
    let id_to_index = build_original_id_to_index(xml);
    let mut ledger = crate::ledger::LossLedger::new();
    crate::transition::parse_animation(&timing_xml, &id_to_index, "", &mut ledger)
}

/// Finds the first element with local name `target_local` and returns its raw
/// XML string.
fn capture_element_xml(xml: &str, target_local: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).ok()? {
            Event::Start(e) if qname_str(e.name()) == target_local => {
                let start = e.into_owned();
                let mut captured = Vec::new();
                let mut writer = Writer::new(&mut captured);
                copy_element(&mut reader, &start, &mut writer, &mut buf).ok()?;
                return Some(String::from_utf8_lossy(&captured).into_owned());
            }
            Event::Empty(e) if qname_str(e.name()) == target_local => {
                let mut captured = Vec::new();
                let mut writer = Writer::new(&mut captured);
                writer.write_event(Event::Empty(e)).ok()?;
                return Some(String::from_utf8_lossy(&captured).into_owned());
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Builds a map from original `p:cNvPr` id to original shape index (document
/// order) by scanning the original `p:spTree`.
fn build_original_id_to_index(xml: &str) -> HashMap<String, usize> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    let mut in_sp_tree = false;
    let mut sp_tree_depth = 0usize;
    let mut depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = qname_str(e.name());
                if local == "spTree" {
                    in_sp_tree = true;
                    sp_tree_depth = depth + 1;
                } else if in_sp_tree
                    && depth == sp_tree_depth
                    && SHAPE_ELEMENT_NAMES.contains(&local.as_str())
                {
                    let start = e.into_owned();
                    let mut captured = Vec::new();
                    let mut writer = Writer::new(&mut captured);
                    if copy_element(&mut reader, &start, &mut writer, &mut buf).is_ok() {
                        let captured_str = String::from_utf8_lossy(&captured);
                        ids.push(
                            extract_shape_id(&captured_str)
                                .unwrap_or_else(|| ids.len().to_string()),
                        );
                    }
                    buf.clear();
                    continue;
                }
                depth += 1;
            }
            Ok(Event::End(e)) => {
                if qname_str(e.name()) == "spTree" {
                    in_sp_tree = false;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    ids.into_iter().enumerate().map(|(i, id)| (id, i)).collect()
}

/// Extracts the first `id` attribute from a `<p:cNvPr>` (or similar non-visual
/// properties) element inside a captured shape.
fn extract_shape_id(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                let local = qname_str(e.name());
                if matches!(
                    local.as_str(),
                    "cNvPr" | "cNvPicPr" | "cNvGraphicFramePr" | "cNvCxnSpPr" | "cNvGrpSpPr"
                ) {
                    if let Some(id) = attr_by_local_name(&e, "id") {
                        return Some(id);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Emits a fresh `p:transition` element from a model [`Transition`].
fn emit_transition_xml(transition: &Transition) -> String {
    let child = match transition.kind {
        TransitionKind::Fade => "p:fade",
        TransitionKind::Push => "p:push",
        TransitionKind::Wipe => "p:wipe",
        TransitionKind::Slide => "p:slide",
        TransitionKind::None => return String::new(),
        TransitionKind::Morph => {
            return format!(
                r#"<p:transition spd="{}"><p:morph option="byObject"/></p:transition>"#,
                transition.duration_ms
            );
        }
    };
    format!(
        r#"<p:transition spd="{}"><{}/></p:transition>"#,
        transition.duration_ms, child
    )
}

/// Emits a fresh `p:timing` element from a model [`Animation`].
///
/// `shapes` is the slide's shape list, used to resolve each build step's target
/// shape to the same `p:cNvPr` id the saver emits for it (see [`cnvpr_id_for`]).
fn emit_timing_xml(animation: &Animation, shapes: &[Shape]) -> String {
    let mut steps = String::new();
    let mut next_id = 3u32;
    for step in &animation.steps {
        let step_xml = build_step_xml(step, next_id, shapes);
        next_id += 4;
        steps.push_str(&step_xml);
    }
    format!(
        r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>{steps}</p:childTnLst></p:cTn></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#
    )
}

/// Emits the nested `p:par`/`p:cTn` structure for a single build step.
fn build_step_xml(step: &BuildStep, id_base: u32, shapes: &[Shape]) -> String {
    let spid = shapes
        .get(step.shape_index)
        .map(|shape| cnvpr_id_for(shape, step.shape_index))
        .unwrap_or_else(|| shape_id_for_index(step.shape_index));
    let filter = filter_for_effect(step.effect);
    let dur = step.duration_ms;
    let id_a = id_base;
    let id_b = id_base + 1;
    let id_c = id_base + 2;
    let id_d = id_base + 3;
    format!(
        r#"<p:par><p:cTn id="{id_a}" fill="hold"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="{id_b}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="{id_c}" presetID="10" presetClass="entr" presetSubtype="0" fill="hold" grpId="0" nodeType="clickEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:animEffect transition="in" filter="{filter}"><p:cBhvr><p:cTn id="{id_d}" dur="{dur}"/><p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#
    )
}

/// Replaces or inserts `p:transition` and `p:timing` in `xml` when they differ
/// from the model, and removes them when the model clears them.
fn patch_transition_and_timing(
    xml: &str,
    slide: &Slide,
    patch_transition: bool,
    patch_animation: bool,
) -> Result<String> {
    let mut out = Vec::new();
    {
        let mut writer = Writer::new(&mut out);
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut in_sp_tree = false;
        let mut original_had_transition = false;
        let mut original_had_timing = false;

        loop {
            let event = reader.read_event_into(&mut buf)?;
            match &event {
                Event::Start(e) => {
                    let local = qname_str(e.name());
                    if local == "spTree" {
                        in_sp_tree = true;
                    } else if !in_sp_tree && local == "transition" {
                        original_had_transition = true;
                        if patch_transition {
                            let start = e.clone().into_owned();
                            let mut captured = Vec::new();
                            let mut capture_writer = Writer::new(&mut captured);
                            copy_element(&mut reader, &start, &mut capture_writer, &mut buf)?;
                            if let Some(ref transition) = slide.transition {
                                writer
                                    .get_mut()
                                    .write_all(emit_transition_xml(transition).as_bytes())?;
                            }
                            buf.clear();
                            continue;
                        }
                    } else if !in_sp_tree && local == "timing" {
                        original_had_timing = true;
                        if patch_animation {
                            let start = e.clone().into_owned();
                            let mut captured = Vec::new();
                            let mut capture_writer = Writer::new(&mut captured);
                            copy_element(&mut reader, &start, &mut capture_writer, &mut buf)?;
                            if let Some(ref animation) = slide.animation {
                                writer.get_mut().write_all(
                                    emit_timing_xml(animation, &slide.shapes).as_bytes(),
                                )?;
                            }
                            buf.clear();
                            continue;
                        }
                    }
                    writer.write_event(event)?;
                }
                Event::End(e) => {
                    let local = qname_str(e.name());
                    if local == "spTree" {
                        in_sp_tree = false;
                    }
                    if local == "cSld" && patch_transition && !original_had_transition {
                        if let Some(ref transition) = slide.transition {
                            writer
                                .get_mut()
                                .write_all(emit_transition_xml(transition).as_bytes())?;
                        }
                    }
                    if local == "sld" && patch_animation && !original_had_timing {
                        if let Some(ref animation) = slide.animation {
                            writer
                                .get_mut()
                                .write_all(emit_timing_xml(animation, &slide.shapes).as_bytes())?;
                        }
                    }
                    writer.write_event(event)?;
                }
                Event::Eof => break,
                _ => writer.write_event(event)?,
            }
            buf.clear();
        }
    }
    String::from_utf8(out).map_err(|e| Error::Save(e.to_string()))
}

/// Patches a single slide XML document in place.
///
/// Editable text boxes have their paragraphs rewritten; modeled images,
/// geometric shapes, and tables have their model-driven content regenerated
/// while non-modeled attributes survive. Chart graphicFrames have their
/// transform updated while the chart relationship id is preserved so the chart
/// XML part can be patched separately. Shapes added since load are appended to
/// the shape tree.
///
/// For dirty slides, `p:transition` and `p:timing` are regenerated when the
/// model differs from the original XML, removed when the model clears them, and
/// copied through verbatim when unchanged.
fn patch_slide_xml(
    slide: &Slide,
    original_xml: &str,
    rids: &HashMap<String, String>,
    link_rids: &HashMap<String, String>,
    chart_rids: &HashMap<usize, String>,
) -> Result<Vec<u8>> {
    let patch_transition = transition_changed(slide, original_xml);
    let patch_animation = animation_changed(slide, original_xml);
    let xml = if patch_transition || patch_animation {
        patch_transition_and_timing(original_xml, slide, patch_transition, patch_animation)?
    } else {
        original_xml.to_string()
    };

    // The positional patch can append shapes, but cannot safely replace,
    // reorder, or insert before an existing shape. Stable IDs also catch undo
    // restoring a deleted shape after a save, even when the shape count grows.
    let original_ids = build_original_id_to_index(&xml);
    let identities_changed = original_ids.iter().any(|(id, index)| {
        slide
            .shapes
            .get(*index)
            .is_some_and(|shape| !shape.id().is_empty() && shape.id() != id)
    });
    if identities_changed || count_top_level_shapes(&xml)? > slide.shapes.len() {
        return regenerate_sp_tree(slide, &xml, rids, link_rids, chart_rids);
    }

    let mut out = Vec::new();
    {
        let mut writer = Writer::new(&mut out);
        let mut reader = Reader::from_str(&xml);
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
                            Some(shape)
                                if matches!(shape, Shape::Table(_)) && local == "graphicFrame" =>
                            {
                                if let Shape::Table(table) = shape {
                                    let frame_id = cnvpr_id_for(shape, shape_idx);
                                    let name = format!("Table {}", shape_idx + 1);
                                    let xml = table_graphic_frame_xml(table, &frame_id, &name);
                                    writer.get_mut().write_all(xml.as_bytes())?;
                                }
                                Some(shape)
                            }
                            Some(shape)
                                if matches!(shape, Shape::Chart(_))
                                    && local == "graphicFrame"
                                    && is_chart_frame(&String::from_utf8_lossy(&captured)) =>
                            {
                                patch_chart_frame_xml(
                                    &mut writer,
                                    &captured,
                                    shape,
                                    chart_rids,
                                    shape_idx,
                                )?;
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
                                chart_rids,
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
    chart_rids: &HashMap<usize, String>,
    index: usize,
) -> Result<()> {
    let id = cnvpr_id_for(shape, index);
    match shape {
        Shape::Image(image) => {
            let Some(embed) = rids.get(&image.media_ref) else {
                return Ok(());
            };
            let name = format!("Picture {}", index + 1);
            let xml = pic_element_xml(image, embed, &id, &name);
            writer.get_mut().write_all(xml.as_bytes())?;
        }
        Shape::Geometric(geo) => {
            let name = format!("Shape {}", index + 1);
            let xml = sp_element_xml(geo, &id, &name);
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
        Shape::Table(table) => {
            let name = format!("Table {}", index + 1);
            let xml = table_graphic_frame_xml(table, &id, &name);
            writer.get_mut().write_all(xml.as_bytes())?;
        }
        Shape::Chart(chart) => {
            if let Some(rid) = chart_rids.get(&index) {
                let name = format!("Chart {}", index + 1);
                let xml = chart_graphic_frame_xml(chart, &id, &name, rid);
                writer.get_mut().write_all(xml.as_bytes())?;
            }
        }
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
    format!(
        "<a:ln w=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:prstDash val=\"{}\"/></a:ln>",
        emu(o.width_emu),
        hex_color(&o.color),
        dash_prst(&o.dash)
    )
}

/// Maps a model [`DashStyle`] to its OOXML `prstDash` value.
fn dash_prst(dash: &DashStyle) -> &'static str {
    match dash {
        DashStyle::Solid => "solid",
        DashStyle::Dash => "dash",
        DashStyle::Dot => "dot",
        DashStyle::DashDot => "dashDot",
    }
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
fn pic_element_xml(image: &ImageShape, embed: &str, id: &str, name: &str) -> String {
    format!(
        "<p:pic><p:nvPicPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvPicPr><a:picLocks/></p:cNvPicPr><p:nvPr/></p:nvPicPr>{}{}</p:pic>",
        blip_fill_xml(embed, image.crop.as_ref()),
        image_sp_pr_xml(image)
    )
}

/// Builds a complete `<p:sp>` element for an inserted geometric shape.
fn sp_element_xml(geo: &GeometricShape, id: &str, name: &str) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr><a:spLocks/></p:cNvSpPr><p:nvPr/></p:nvSpPr>{}<p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp>",
        geometric_sp_pr_xml(geo)
    )
}

const TABLE_GRAPHIC_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/table";

/// Builds a complete `<p:graphicFrame>` element for a table shape.
///
/// The frame carries a `p:xfrm` (off/ext plus rotation), and a `a:graphic`/
/// `a:graphicData` block containing the full `a:tbl`. Tables are emitted only
/// for dirty slides, so untouched frames stay byte-for-byte identical (§4.9).
fn table_graphic_frame_xml(table: &TableShape, id: &str, name: &str) -> String {
    let f = table.transform.frame;
    let rot = if table.transform.rotation != 0.0 {
        format!(
            " rot=\"{}\"",
            (table.transform.rotation * 60_000.0).round() as i64
        )
    } else {
        String::new()
    };
    let first_row = if table.header_row { "1" } else { "0" };

    let mut grid = String::new();
    for &w in &table.column_widths {
        grid.push_str(&format!("<a:gridCol w=\"{}\"/>", emu(w)));
    }

    let mut rows_xml = String::new();
    for row in &table.rows {
        rows_xml.push_str(&format!("<a:tr h=\"{}\">", emu(row.height)));
        for cell in &row.cells {
            rows_xml.push_str(&table_cell_xml(cell));
        }
        rows_xml.push_str("</a:tr>");
    }

    format!(
        "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"{id}\" name=\"{name}\"/>\
         <p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>\
         <p:xfrm{rot}><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></p:xfrm>\
         <a:graphic><a:graphicData uri=\"{TABLE_GRAPHIC_URI}\">\
         <a:tbl><a:tblPr firstRow=\"{first_row}\"/><a:tblGrid>{grid}</a:tblGrid>{rows_xml}</a:tbl>\
         </a:graphicData></a:graphic></p:graphicFrame>",
        emu(f.x),
        emu(f.y),
        emu(f.width),
        emu(f.height)
    )
}

/// Builds a single `<a:tc>` element: a plain-text `a:txBody` and an `a:tcPr`
/// with the cell's fill and per-edge border overrides.
fn table_cell_xml(cell: &TableCell) -> String {
    let ppr = match cell.align {
        CellAlign::Left => String::new(),
        CellAlign::Center => "<a:pPr algn=\"ctr\"/>".to_string(),
        CellAlign::Right => "<a:pPr algn=\"r\"/>".to_string(),
    };
    let text = escape_xml_text(&cell.text);
    let txbody = format!(
        "<a:txBody><a:bodyPr/><a:lstStyle/><a:p>{ppr}\
         <a:r><a:rPr/><a:t xml:space=\"preserve\">{text}</a:t></a:r></a:p></a:txBody>"
    );
    let tcpr = table_tc_pr_xml(cell);
    format!("<a:tc>{txbody}{tcpr}</a:tc>")
}

/// Builds the `<a:tcPr>` element for a cell. Schema order is borders first
/// (`a:lnL`, `a:lnR`, `a:lnT`, `a:lnB`) then the fill (`a:solidFill`). A cell
/// with no explicit borders omits the border children (inheriting the table
/// default); a cell with no fill omits the fill.
fn table_tc_pr_xml(cell: &TableCell) -> String {
    let mut inner = String::new();
    if let Some(borders) = &cell.borders {
        if let Some(e) = &borders.left {
            inner.push_str(&table_ln_xml("a:lnL", e));
        }
        if let Some(e) = &borders.right {
            inner.push_str(&table_ln_xml("a:lnR", e));
        }
        if let Some(e) = &borders.top {
            inner.push_str(&table_ln_xml("a:lnT", e));
        }
        if let Some(e) = &borders.bottom {
            inner.push_str(&table_ln_xml("a:lnB", e));
        }
    }
    if let Some(Fill::Solid(color)) = &cell.fill {
        inner.push_str(&format!(
            "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            hex_color(color)
        ));
    }
    format!("<a:tcPr>{inner}</a:tcPr>")
}

/// Builds a single border-edge `<a:lnX>` element for a cell.
fn table_ln_xml(tag: &str, edge: &BorderEdge) -> String {
    format!(
        "<{tag} w=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:prstDash val=\"{}\"/></{tag}>",
        emu(edge.width_emu),
        hex_color(&edge.color),
        dash_prst(&edge.dash)
    )
}

/// Escapes a text string for inclusion as XML character data.
fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
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
    let font_size;
    if let Some(size) = run.font_size.filter(|size| size.is_finite() && *size > 0.0) {
        font_size = (size / 127.0).round().clamp(100.0, 400_000.0).to_string();
        rpr.push_attribute(("sz", font_size.as_str()));
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
    let needs_rpr_inner = run.font_family.is_some() || has_link || run.color.is_some();
    if needs_rpr_inner {
        writer.write_event(Event::Start(rpr))?;
        if let Some(color) = run.color {
            let alpha = if color.a == 255 {
                String::new()
            } else {
                format!(
                    r#"<a:alpha val="{}"/>"#,
                    (f64::from(color.a) * 100_000.0 / 255.0).round()
                )
            };
            writer.get_mut().write_all(format!(r#"<a:solidFill><a:srgbClr val="{:02X}{:02X}{:02X}">{alpha}</a:srgbClr></a:solidFill>"#, color.r, color.g, color.b).as_bytes())?;
        }
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
