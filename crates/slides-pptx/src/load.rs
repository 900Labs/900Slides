//! PPTX loading: convert an OOXML package into a `slides-core` deck.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::QName;
use quick_xml::{Reader, Writer};
use slides_core::{
    Color, Crop, DashStyle, Deck, Fill, GeometricShape, ImageShape, ListStyle, MediaEntry,
    MediaStore, Outline, Paragraph, PassthroughObject, Rect, Run, Shadow, Shape, Slide, Style,
    TextBox, Theme, Transform,
};

use crate::error::{Error, Result};
use crate::geometry;
use crate::ledger::{LossLedger, LossWarning};
use crate::media as pkgmedia;
use crate::package::{
    find_rel_by_type, parse_rels, Rel, REL_TYPE_IMAGE, REL_TYPE_MANIFEST, REL_TYPE_NOTES_SLIDE,
    REL_TYPE_OFFICE_DOCUMENT, REL_TYPE_SLIDE, REL_TYPE_THEME,
};

pub(crate) const SHAPE_ELEMENT_NAMES: &[&str] =
    &["sp", "pic", "graphicFrame", "cxnSp", "grpSp", "contentPart"];

/// Opens and validates a PPTX ZIP archive.
pub fn open_and_validate(bytes: &[u8]) -> Result<zip::ZipArchive<std::io::Cursor<&[u8]>>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;

    const MAX_ENTRY: u64 = 50 * 1024 * 1024;
    const MAX_TOTAL: u64 = 500 * 1024 * 1024;

    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name();
        if !is_safe_zip_path(name) {
            return Err(Error::UnsafePath(name.to_string()));
        }
        if file.size() > MAX_ENTRY {
            return Err(Error::EntryTooLarge);
        }
        total += file.size();
        if total > MAX_TOTAL {
            return Err(Error::ArchiveTooLarge);
        }
    }
    Ok(archive)
}

fn is_safe_zip_path(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    use std::path::Component;
    let path = std::path::Path::new(name);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return false,
        }
    }
    true
}

/// Reads a ZIP entry by path into a UTF-8 string.
pub fn read_entry_to_string(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    path: &str,
) -> Result<String> {
    let mut file = archive.by_name(path)?;
    let mut buf = Vec::with_capacity(file.size() as usize);
    std::io::copy(&mut file, &mut buf)?;
    String::from_utf8(buf).map_err(|_| Error::UnsupportedFormat(format!("non-utf8 part: {path}")))
}

/// Reads a ZIP entry by path into raw bytes.
pub fn read_entry_to_bytes(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    path: &str,
) -> Result<Vec<u8>> {
    let mut file = archive.by_name(path)?;
    let mut buf = Vec::with_capacity(file.size() as usize);
    std::io::copy(&mut file, &mut buf)?;
    Ok(buf)
}

/// Resolved media for a slide: a relationship-id view (used while parsing the
/// slide XML) plus the per-content-key relationship id (used by the saver).
#[derive(Debug, Default, Clone)]
pub(crate) struct SlideMedia {
    /// `r:embed` relationship id -> media content key (into `deck.media`).
    pub by_rid: HashMap<String, String>,
    /// media content key -> relationship id (the inverse of [`Self::by_rid`]).
    pub rid_by_media: HashMap<String, String>,
}

/// Loads every image referenced by a slide's relationships into the deck media
/// store, returning the resolved relationship mapping.
///
/// Each image part is ingested through `slides_media` (MIME sniff, EXIF strip,
/// size/dimension caps) and stored under a content-addressed key. A failure to
/// ingest a single part is recorded as a loss warning rather than aborting the
/// whole load.
fn load_slide_media(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    slide_path: &str,
    media: &mut MediaStore,
    ledger: &mut LossLedger,
) -> Result<SlideMedia> {
    let rels_path = rels_path_for(slide_path);
    let rels_xml = match read_entry_to_string(archive, &rels_path) {
        Ok(xml) => xml,
        Err(_) => return Ok(SlideMedia::default()),
    };
    let rels = parse_rels(&rels_xml)?;
    let base = base_dir(slide_path);
    let mut out = SlideMedia::default();

    for rel in rels.iter().filter(|r| r.rel_type == REL_TYPE_IMAGE) {
        let Some(part_path) = rel.resolve(&base) else {
            continue;
        };
        let bytes = match read_entry_to_bytes(archive, &part_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                ledger.add(LossWarning::new(
                    slide_path,
                    format!("could not read media part {part_path}: {err}"),
                ));
                continue;
            }
        };
        let ingested = match slides_media::ingest(&bytes, &slides_media::IngestOptions::default()) {
            Ok(img) => img,
            Err(err) => {
                ledger.add(LossWarning::new(
                    slide_path,
                    format!("image ingest failed for {part_path}: {err}"),
                ));
                continue;
            }
        };
        let key = pkgmedia::media_key(&ingested.bytes);
        if !media.contains_key(&key) {
            media.insert(
                key.clone(),
                MediaEntry {
                    mime: ingested.mime.to_string(),
                    bytes: ingested.bytes,
                    width: ingested.width,
                    height: ingested.height,
                },
            );
        }
        out.by_rid.insert(rel.id.clone(), key.clone());
        out.rid_by_media.insert(key, rel.id.clone());
    }

    Ok(out)
}

/// Result of loading a PPTX package.
#[derive(Debug)]
pub(crate) struct LoadResult {
    pub deck: Deck,
    pub package_rels: Vec<Rel>,
    pub slide_paths: HashMap<String, String>,
    pub manifest_path: Option<String>,
    pub loss_ledger: LossLedger,
    /// For each slide (keyed by its part path), a map from a media content key
    /// (into `deck.media`) to the OOXML relationship id that resolves it. Used
    /// by the saver to emit `<a:blip r:embed="...">` for modeled images.
    pub slide_media_rids: HashMap<String, HashMap<String, String>>,
}

/// Loads a PPTX package into a [`Deck`] plus package metadata.
pub fn load(bytes: &[u8]) -> Result<LoadResult> {
    let mut archive = open_and_validate(bytes)?;

    let package_rels_xml = read_entry_to_string(&mut archive, "_rels/.rels")?;
    let package_rels = parse_rels(&package_rels_xml)?;

    let presentation_rel = find_rel_by_type(&package_rels, REL_TYPE_OFFICE_DOCUMENT)
        .ok_or_else(|| Error::MissingRelationship(REL_TYPE_OFFICE_DOCUMENT.to_string()))?;
    let presentation_path = presentation_rel
        .resolve("")
        .ok_or_else(|| Error::MissingPart("ppt/presentation.xml".to_string()))?;

    let presentation_rels_path = rels_path_for(&presentation_path);
    let presentation_rels_xml = read_entry_to_string(&mut archive, &presentation_rels_path)?;
    let presentation_rels = parse_rels(&presentation_rels_xml)?;

    let slide_rids = parse_presentation(&read_entry_to_string(&mut archive, &presentation_path)?)?;
    let slide_path_by_rid: HashMap<String, String> = presentation_rels
        .iter()
        .filter(|r| r.rel_type == REL_TYPE_SLIDE)
        .filter_map(|r| {
            r.resolve(&base_dir(&presentation_path))
                .map(|path| (r.id.clone(), path))
        })
        .collect();

    let theme_path = presentation_rels
        .iter()
        .find(|r| r.rel_type == REL_TYPE_THEME)
        .and_then(|r| r.resolve(&base_dir(&presentation_path)));

    let mut deck = Deck::new();
    let mut ledger = LossLedger::new();
    let mut slide_paths = HashMap::new();
    let mut slide_media_rids: HashMap<String, HashMap<String, String>> = HashMap::new();

    if let Some(path) = theme_path {
        if let Ok(xml) = read_entry_to_string(&mut archive, &path) {
            deck.theme = parse_theme(&xml).unwrap_or_default();
        }
    }

    for rid in slide_rids {
        let Some(slide_path) = slide_path_by_rid.get(&rid) else {
            ledger.add(LossWarning::new(
                &rid,
                format!("could not resolve slide relationship {rid}"),
            ));
            continue;
        };
        let slide_media = load_slide_media(&mut archive, slide_path, &mut deck.media, &mut ledger)?;
        let slide_xml = read_entry_to_string(&mut archive, slide_path)?;
        let mut slide = parse_slide(&slide_xml, slide_path, &slide_media.by_rid, &mut ledger)?;
        slide.id = slide_path.to_string();
        if let Ok(notes) = load_slide_notes(&mut archive, slide_path) {
            slide.notes = notes;
        }
        slide_paths.insert(slide_path.to_string(), slide_path.to_string());
        slide_media_rids.insert(slide_path.to_string(), slide_media.rid_by_media);
        deck.slides.push(slide);
    }

    let manifest_path = package_rels
        .iter()
        .find(|r| r.rel_type == REL_TYPE_MANIFEST)
        .and_then(|r| r.resolve(""));

    Ok(LoadResult {
        deck,
        package_rels,
        slide_paths,
        manifest_path,
        loss_ledger: ledger,
        slide_media_rids,
    })
}

fn base_dir(part: &str) -> String {
    std::path::Path::new(part)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

pub(crate) fn rels_path_for(part: &str) -> String {
    let base = base_dir(part);
    let file_name = std::path::Path::new(part)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    if base.is_empty() {
        "_rels/.rels".to_string()
    } else {
        format!("{base}/_rels/{file_name}.rels")
    }
}

fn parse_presentation(xml: &str) -> Result<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut rids = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => {
                if qname_str(e.name()) == "sldId" {
                    if let Some(rid) = rel_attribute(&e, "id") {
                        rids.push(rid);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(rids)
}

fn parse_slide(
    xml: &str,
    slide_id: &str,
    slide_media: &HashMap<String, String>,
    ledger: &mut LossLedger,
) -> Result<Slide> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut shapes: Vec<Shape> = Vec::new();
    let mut in_sp_tree = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let local = qname_str(e.name());
                if local == "spTree" {
                    in_sp_tree = true;
                } else if in_sp_tree && SHAPE_ELEMENT_NAMES.contains(&local.as_str()) {
                    let start = e.into_owned();
                    let mut captured = Vec::new();
                    let mut writer = Writer::new(&mut captured);
                    copy_element(&mut reader, &start, &mut writer, &mut buf)?;
                    let captured_str = String::from_utf8_lossy(&captured).into_owned();

                    if local == "pic" {
                        match parse_pic(&captured_str, slide_media) {
                            Some(image) => shapes.push(Shape::Image(image)),
                            None => {
                                ledger.add(LossWarning::new(
                                    slide_id,
                                    format!("preserved {local} as opaque object; not editable"),
                                ));
                                let frame = parse_frame(&captured_str);
                                shapes.push(Shape::Passthrough(PassthroughObject {
                                    id: extract_id(&captured_str, shapes.len()),
                                    label: local.clone(),
                                    source_part: slide_id.to_string(),
                                    raw_bytes: captured,
                                    frame,
                                }));
                            }
                        }
                    } else if local == "sp" {
                        if let Some(geometric) = parse_geometric(&captured_str) {
                            shapes.push(Shape::Geometric(geometric));
                        } else if captured_str.contains("<p:txBody") {
                            match parse_text_box(&captured_str) {
                                Ok(text_box) => shapes.push(Shape::TextBox(text_box)),
                                Err(err) => {
                                    ledger.add(LossWarning::new(
                                        slide_id,
                                        format!("failed to parse text box: {err}"),
                                    ));
                                    let frame = parse_frame(&captured_str);
                                    shapes.push(Shape::Passthrough(PassthroughObject {
                                        id: extract_id(&captured_str, shapes.len()),
                                        label: local,
                                        source_part: slide_id.to_string(),
                                        raw_bytes: captured,
                                        frame,
                                    }));
                                }
                            }
                        } else {
                            ledger.add(LossWarning::new(
                                slide_id,
                                format!("preserved {local} as opaque object; not editable"),
                            ));
                            let frame = parse_frame(&captured_str);
                            shapes.push(Shape::Passthrough(PassthroughObject {
                                id: extract_id(&captured_str, shapes.len()),
                                label: local,
                                source_part: slide_id.to_string(),
                                raw_bytes: captured,
                                frame,
                            }));
                        }
                    } else {
                        ledger.add(LossWarning::new(
                            slide_id,
                            format!("preserved {local} as opaque object; not editable"),
                        ));
                        let frame = parse_frame(&captured_str);
                        shapes.push(Shape::Passthrough(PassthroughObject {
                            id: extract_id(&captured_str, shapes.len()),
                            label: local,
                            source_part: slide_id.to_string(),
                            raw_bytes: captured,
                            frame,
                        }));
                    }
                }
            }
            Event::End(e) => {
                if qname_str(e.name()) == "spTree" {
                    in_sp_tree = false;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(Slide {
        id: String::new(),
        notes: String::new(),
        shapes,
        animation: None,
        transition: None,
    })
}

pub(crate) fn copy_element<R, W>(
    reader: &mut Reader<R>,
    start: &BytesStart<'_>,
    writer: &mut Writer<W>,
    buf: &mut Vec<u8>,
) -> Result<()>
where
    R: BufRead,
    W: Write,
{
    let target = start.name();
    let mut depth = 0usize;
    writer.write_event(Event::Start(start.clone()))?;

    loop {
        match reader.read_event_into(buf)? {
            Event::Start(e) => {
                if e.name() == target {
                    depth += 1;
                }
                writer.write_event(Event::Start(e))?;
            }
            Event::End(e) => {
                let matches = e.name() == target;
                writer.write_event(Event::End(e))?;
                if matches && depth == 0 {
                    break;
                }
                if matches {
                    depth -= 1;
                }
            }
            Event::Empty(e) => writer.write_event(Event::Empty(e))?,
            Event::Text(t) => writer.write_event(Event::Text(t))?,
            Event::CData(c) => writer.write_event(Event::CData(c))?,
            Event::Comment(c) => writer.write_event(Event::Comment(c))?,
            Event::PI(p) => writer.write_event(Event::PI(p))?,
            Event::Decl(d) => writer.write_event(Event::Decl(d))?,
            Event::DocType(d) => writer.write_event(Event::DocType(d))?,
            Event::Eof => return Err(Error::MissingPart("truncated slide XML".into())),
        }
        buf.clear();
    }
    Ok(())
}

fn parse_text_box(xml: &str) -> Result<TextBox> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut paragraphs = Vec::new();
    let mut frame: Option<Rect> = None;

    let mut current_paragraph: Option<(Vec<Run>, ListStyle)> = None;
    let mut current_run_text = String::new();
    let mut current_run_bold = false;
    let mut current_run_italic = false;
    let mut current_run_underline = false;

    let mut in_xfrm = false;
    let mut in_ppr = false;
    let mut in_text = false;

    let mut off: Option<(f64, f64)> = None;
    let mut ext: Option<(f64, f64)> = None;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "xfrm" => in_xfrm = true,
                    "off" if in_xfrm => {
                        off = Some((
                            parse_attr_f64(&e, "x").unwrap_or(0.0),
                            parse_attr_f64(&e, "y").unwrap_or(0.0),
                        ));
                    }
                    "ext" if in_xfrm => {
                        ext = Some((
                            parse_attr_f64(&e, "cx").unwrap_or(0.0),
                            parse_attr_f64(&e, "cy").unwrap_or(0.0),
                        ));
                    }
                    "p" => current_paragraph = Some((Vec::new(), ListStyle::None)),
                    "pPr" => in_ppr = true,
                    "buAutoNum" if in_ppr => {
                        if let Some(ref mut para) = current_paragraph {
                            para.1 = ListStyle::Ordered;
                        }
                    }
                    "buChar" if in_ppr => {
                        if let Some(ref mut para) = current_paragraph {
                            para.1 = ListStyle::Unordered;
                        }
                    }
                    "buNone" if in_ppr => {
                        if let Some(ref mut para) = current_paragraph {
                            para.1 = ListStyle::None;
                        }
                    }
                    "r" => {
                        current_run_text.clear();
                        current_run_bold = false;
                        current_run_italic = false;
                        current_run_underline = false;
                    }
                    "rPr" => {
                        current_run_bold = parse_bool_attr(&e, "b").unwrap_or(false);
                        current_run_italic = parse_bool_attr(&e, "i").unwrap_or(false);
                        current_run_underline = parse_underline_attr(&e);
                    }
                    "t" => in_text = true,
                    _ => {}
                }
            }
            Event::Empty(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "off" if in_xfrm => {
                        off = Some((
                            parse_attr_f64(&e, "x").unwrap_or(0.0),
                            parse_attr_f64(&e, "y").unwrap_or(0.0),
                        ));
                    }
                    "ext" if in_xfrm => {
                        ext = Some((
                            parse_attr_f64(&e, "cx").unwrap_or(0.0),
                            parse_attr_f64(&e, "cy").unwrap_or(0.0),
                        ));
                    }
                    "p" => {
                        paragraphs.push(Paragraph {
                            runs: Vec::new(),
                            list_style: ListStyle::None,
                            ..Default::default()
                        });
                    }
                    "buAutoNum" if in_ppr => {
                        if let Some(ref mut para) = current_paragraph {
                            para.1 = ListStyle::Ordered;
                        }
                    }
                    "buChar" if in_ppr => {
                        if let Some(ref mut para) = current_paragraph {
                            para.1 = ListStyle::Unordered;
                        }
                    }
                    "buNone" if in_ppr => {
                        if let Some(ref mut para) = current_paragraph {
                            para.1 = ListStyle::None;
                        }
                    }
                    "r" => {
                        let bold = parse_bool_attr(&e, "b").unwrap_or(false);
                        let italic = parse_bool_attr(&e, "i").unwrap_or(false);
                        let underline = parse_underline_attr(&e);
                        if let Some(ref mut para) = current_paragraph {
                            para.0.push(Run {
                                text: String::new(),
                                bold,
                                italic,
                                underline,
                                ..Default::default()
                            });
                        }
                    }
                    "rPr" => {
                        current_run_bold = parse_bool_attr(&e, "b").unwrap_or(false);
                        current_run_italic = parse_bool_attr(&e, "i").unwrap_or(false);
                        current_run_underline = parse_underline_attr(&e);
                    }
                    _ => {}
                }
            }
            Event::Text(t) => {
                if in_text {
                    current_run_text.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Event::End(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "xfrm" => {
                        in_xfrm = false;
                        if let (Some((x, y)), Some((cx, cy))) = (off, ext) {
                            frame = Some(Rect::new(x, y, cx, cy));
                        }
                        off = None;
                        ext = None;
                    }
                    "p" => {
                        if let Some((runs, list_style)) = current_paragraph.take() {
                            paragraphs.push(Paragraph {
                                runs,
                                list_style,
                                ..Default::default()
                            });
                        }
                    }
                    "pPr" => in_ppr = false,
                    "r" => {
                        if let Some(ref mut para) = current_paragraph {
                            para.0.push(Run {
                                text: std::mem::take(&mut current_run_text),
                                bold: current_run_bold,
                                italic: current_run_italic,
                                underline: current_run_underline,
                                ..Default::default()
                            });
                        }
                    }
                    "t" => in_text = false,
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(TextBox {
        frame: frame.unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0)),
        paragraphs,
    })
}

fn parse_frame(xml: &str) -> Option<Rect> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut in_xfrm = false;
    let mut off: Option<(f64, f64)> = None;
    let mut ext: Option<(f64, f64)> = None;

    loop {
        match reader.read_event_into(&mut buf).ok()? {
            Event::Start(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "xfrm" => in_xfrm = true,
                    "off" if in_xfrm => {
                        off = Some((
                            parse_attr_f64(&e, "x").unwrap_or(0.0),
                            parse_attr_f64(&e, "y").unwrap_or(0.0),
                        ));
                    }
                    "ext" if in_xfrm => {
                        ext = Some((
                            parse_attr_f64(&e, "cx").unwrap_or(0.0),
                            parse_attr_f64(&e, "cy").unwrap_or(0.0),
                        ));
                    }
                    _ => {}
                }
            }
            Event::Empty(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "off" if in_xfrm => {
                        off = Some((
                            parse_attr_f64(&e, "x").unwrap_or(0.0),
                            parse_attr_f64(&e, "y").unwrap_or(0.0),
                        ));
                    }
                    "ext" if in_xfrm => {
                        ext = Some((
                            parse_attr_f64(&e, "cx").unwrap_or(0.0),
                            parse_attr_f64(&e, "cy").unwrap_or(0.0),
                        ));
                    }
                    _ => {}
                }
            }
            Event::End(e) => {
                if qname_str(e.name()) == "xfrm" {
                    in_xfrm = false;
                    if let (Some((x, y)), Some((cx, cy))) = (off, ext) {
                        return Some(Rect::new(x, y, cx, cy));
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    None
}

/// Parsed visual properties of a captured `p:sp` or `p:pic` element.
#[derive(Default)]
struct ShapeProps {
    transform: Option<Transform>,
    prst: Option<String>,
    adj_fraction: Option<f64>,
    fill: Option<Fill>,
    outline: Option<Outline>,
    shadow: Option<Shadow>,
    src_rect: Option<Crop>,
    blip_embed: Option<String>,
}

/// Where a `<a:solidFill>` is currently being read.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FillTarget {
    ShapeFill,
    Outline,
}

/// Scans a captured shape element and collects its modeled visual properties
/// (transform, preset geometry, fill, outline, shadow, image crop, and blip
/// relationship id).
fn parse_shape_props(captured: &str) -> ShapeProps {
    let mut reader = Reader::from_str(captured);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut props = ShapeProps::default();

    let mut in_sp_pr = false;
    let mut in_blip_fill = false;
    let mut in_ln = false;
    let mut in_effect_lst = false;
    let mut in_outer_shdw = false;
    let mut solid_fill_target: Option<FillTarget> = None;

    let mut xfrm_rot: Option<f64> = None;
    let mut off: Option<(f64, f64)> = None;
    let mut ext: Option<(f64, f64)> = None;

    let mut shadow_blur: Option<f64> = None;
    let mut shadow_dist: Option<f64> = None;
    let mut shadow_dir: Option<f64> = None;
    let mut shadow_color: Option<Color> = None;
    let mut shadow_alpha: Option<f64> = None;

    let mut outline_w: Option<f64> = None;
    let mut outline_color: Option<Color> = None;
    let mut outline_dash: Option<DashStyle> = None;

    loop {
        let Ok(event) = reader.read_event_into(&mut buf) else {
            break;
        };
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "spPr" => in_sp_pr = true,
                    "blipFill" => in_blip_fill = true,
                    "ln" if in_sp_pr => {
                        in_ln = true;
                        outline_w = parse_attr_f64(e, "w");
                    }
                    "effectLst" if in_sp_pr => in_effect_lst = true,
                    "outerShdw" if in_effect_lst => {
                        in_outer_shdw = true;
                        shadow_blur = parse_attr_f64(e, "blurRad");
                        shadow_dist = parse_attr_f64(e, "dist");
                        shadow_dir = parse_attr_f64(e, "dir");
                    }
                    "xfrm" => {
                        xfrm_rot = parse_attr_f64(e, "rot");
                    }
                    "off" => {
                        off = Some((
                            parse_attr_f64(e, "x").unwrap_or(0.0),
                            parse_attr_f64(e, "y").unwrap_or(0.0),
                        ));
                    }
                    "ext" => {
                        ext = Some((
                            parse_attr_f64(e, "cx").unwrap_or(0.0),
                            parse_attr_f64(e, "cy").unwrap_or(0.0),
                        ));
                    }
                    "prstGeom" => {
                        props.prst = attr_by_local_name(e, "prst");
                    }
                    "gd" if attr_by_local_name(e, "name").as_deref() == Some("adj") => {
                        if let Some(fmla) = attr_by_local_name(e, "fmla") {
                            props.adj_fraction = parse_adj_fmla(&fmla);
                        }
                    }
                    "solidFill" => {
                        if in_ln {
                            solid_fill_target = Some(FillTarget::Outline);
                        } else if in_sp_pr {
                            solid_fill_target = Some(FillTarget::ShapeFill);
                        }
                    }
                    "noFill" => {
                        if in_ln {
                            outline_dash = outline_dash.or(Some(DashStyle::Solid));
                            // noFill inside ln means no outline; mark color absent.
                            outline_color = None;
                        } else if in_sp_pr && !in_ln {
                            props.fill = None;
                        }
                    }
                    "srgbClr" => {
                        if let Some(hex) = attr_by_local_name(e, "val") {
                            let color = parse_hex_color(&hex);
                            if in_outer_shdw {
                                shadow_color = color;
                            } else if let Some(target) = solid_fill_target {
                                match target {
                                    FillTarget::ShapeFill => props.fill = color.map(Fill::Solid),
                                    FillTarget::Outline => outline_color = color,
                                }
                            }
                        }
                    }
                    "alpha" if in_outer_shdw => {
                        if let Some(val) = parse_attr_f64(e, "val") {
                            shadow_alpha = Some(val / 100_000.0);
                        }
                    }
                    "prstDash" if in_ln => {
                        outline_dash = attr_by_local_name(e, "val").as_deref().map(parse_dash);
                    }
                    "srcRect" if in_blip_fill => {
                        props.src_rect = Some(Crop {
                            left: parse_attr_f64(e, "l").map(|v| v / 100_000.0).unwrap_or(0.0),
                            top: parse_attr_f64(e, "t").map(|v| v / 100_000.0).unwrap_or(0.0),
                            right: parse_attr_f64(e, "r").map(|v| v / 100_000.0).unwrap_or(0.0),
                            bottom: parse_attr_f64(e, "b").map(|v| v / 100_000.0).unwrap_or(0.0),
                        });
                    }
                    "blip" => {
                        props.blip_embed = rel_attribute(e, "embed");
                    }
                    _ => {}
                }
            }
            Event::End(ref e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "spPr" => {
                        in_sp_pr = false;
                        if let (Some((x, y)), Some((cx, cy))) = (off, ext) {
                            props.transform = Some(Transform {
                                frame: Rect::new(x, y, cx, cy),
                                rotation: xfrm_rot.map(|r| r / 60_000.0).unwrap_or(0.0),
                            });
                        }
                        off = None;
                        ext = None;
                        xfrm_rot = None;
                    }
                    "blipFill" => in_blip_fill = false,
                    "ln" => {
                        if in_ln {
                            in_ln = false;
                            if let Some(color) = outline_color.take() {
                                props.outline = Some(Outline {
                                    color,
                                    width_emu: outline_w.unwrap_or(0.0),
                                    dash: outline_dash.unwrap_or_default(),
                                });
                            }
                            outline_w = None;
                            outline_dash = None;
                        }
                    }
                    "effectLst" => in_effect_lst = false,
                    "outerShdw" => {
                        in_outer_shdw = false;
                        if let Some(color) = shadow_color.take() {
                            let dist = shadow_dist.unwrap_or(0.0);
                            let dir = shadow_dir.unwrap_or(0.0);
                            let theta = dir.to_radians() / 60_000.0;
                            props.shadow = Some(Shadow {
                                offset_x: dist * theta.cos(),
                                offset_y: dist * theta.sin(),
                                blur: shadow_blur.unwrap_or(0.0),
                                color,
                                opacity: shadow_alpha.unwrap_or(1.0),
                            });
                        }
                        shadow_blur = None;
                        shadow_dist = None;
                        shadow_dir = None;
                        shadow_alpha = None;
                    }
                    "solidFill" => {
                        solid_fill_target = None;
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    props
}

/// Parses an OOXML adjustment formula (`val <n>`) into a fraction.
fn parse_adj_fmla(fmla: &str) -> Option<f64> {
    let n = fmla.strip_prefix("val")?.trim();
    let n: f64 = n.parse().ok()?;
    Some(n / 100_000.0)
}

/// Maps an OOXML preset dash name to the model [`DashStyle`].
fn parse_dash(val: &str) -> DashStyle {
    match val {
        "dash" | "sysDash" | "lgDash" => DashStyle::Dash,
        "dot" | "sysDot" => DashStyle::Dot,
        "dashDot" | "sysDashDot" => DashStyle::DashDot,
        _ => DashStyle::Solid,
    }
}

/// Returns `true` when a captured shape element contains a non-empty `<a:t>`
/// text run (i.e. it carries editable text).
fn has_editable_text(captured: &str) -> bool {
    let bytes = captured.as_bytes();
    let mut i = 0usize;
    while let Some(rel) = find_subslice(bytes, b"<a:t", i) {
        i = rel + 4;
        // Require a tag-terminating delimiter so we don't match `<a:tbl`.
        let next = bytes.get(rel + 4).copied();
        if !matches!(
            next,
            Some(b'>') | Some(b' ') | Some(b'/') | Some(b'\t') | Some(b'\n')
        ) {
            continue;
        }
        let Some(gt) = find_subslice(bytes, b">", rel) else {
            break;
        };
        let close = match find_subslice(bytes, b"</a:t>", gt + 1) {
            Some(c) => c,
            None => break,
        };
        let text = &captured[gt + 1..close];
        if !text.trim().is_empty() {
            return true;
        }
        i = close + 5;
    }
    false
}

/// Finds the next occurrence of `needle` in `haystack` at or after `from`.
fn find_subslice(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Parses a captured `p:pic` element into an [`ImageShape`].
///
/// Returns `None` when the picture has no resolvable `r:embed` relationship
/// (for example, an empty placeholder), so the caller can preserve it opaquely.
fn parse_pic(captured: &str, slide_media: &HashMap<String, String>) -> Option<ImageShape> {
    let props = parse_shape_props(captured);
    let embed = props.blip_embed.as_deref()?;
    let media_ref = slide_media.get(embed)?;
    Some(ImageShape {
        transform: props.transform.unwrap_or_default(),
        media_ref: media_ref.clone(),
        crop: props.src_rect,
    })
}

/// Parses a captured `p:sp` element into a [`GeometricShape`] when it carries a
/// recognized preset geometry and no editable text.
///
/// Returns `None` for shapes that should be modeled as text boxes (rectangles
/// with text) or preserved opaquely (unsupported or custom geometry).
fn parse_geometric(captured: &str) -> Option<GeometricShape> {
    let props = parse_shape_props(captured);
    let prst = props.prst.as_deref()?;
    if !geometry::is_supported_prst(prst) {
        return None;
    }
    let transform = props.transform.unwrap_or_default();
    let geometry = geometry::geometry_from_prst(prst, props.adj_fraction, transform.frame)?;
    // A shape carrying editable text is modeled as a text box so its text stays
    // editable. The geometry is still preserved: the saver patches the original
    // `<p:spPr>` (including the preset geometry) verbatim and only rewrites the
    // text body. Applied to every geometry, not just rectangles, so that e.g.
    // a text-bearing ellipse does not silently lose its text.
    if has_editable_text(captured) {
        return None;
    }
    Some(GeometricShape {
        transform,
        geometry,
        style: Style {
            fill: props.fill,
            outline: props.outline,
            shadow: props.shadow,
        },
    })
}

/// Extracts the `r:embed` relationship id from a captured picture element.
pub(crate) fn extract_blip_embed(captured: &str) -> Option<String> {
    parse_shape_props(captured).blip_embed
}

fn load_slide_notes(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    slide_path: &str,
) -> Result<String> {
    let rels_path = rels_path_for(slide_path);
    let rels_xml = read_entry_to_string(archive, &rels_path)?;
    let rels = parse_rels(&rels_xml)?;
    let Some(notes_rel) = find_rel_by_type(&rels, REL_TYPE_NOTES_SLIDE) else {
        return Ok(String::new());
    };
    let base = base_dir(slide_path);
    let notes_path = notes_rel
        .resolve(&base)
        .ok_or_else(|| Error::MissingPart("notes slide".to_string()))?;
    let notes_xml = read_entry_to_string(archive, &notes_path)?;
    Ok(extract_notes_text(&notes_xml))
}

fn extract_notes_text(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut in_tx_body = false;
    let mut tx_body_depth = 0usize;
    let mut paragraph_texts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_text = false;
    let mut depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                let local = qname_str(e.name());
                if local == "txBody" {
                    in_tx_body = true;
                    tx_body_depth = depth;
                } else if local == "p" && in_tx_body && depth == tx_body_depth + 1 {
                    current.clear();
                } else if local == "t" {
                    in_text = true;
                }
            }
            Ok(Event::Empty(e)) => {
                let local = qname_str(e.name());
                if local == "p" && in_tx_body && depth == tx_body_depth + 1 {
                    paragraph_texts.push(String::new());
                }
            }
            Ok(Event::Text(t)) => {
                if in_text {
                    current.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => {
                let local = qname_str(e.name());
                if local == "txBody" && depth == tx_body_depth {
                    in_tx_body = false;
                } else if local == "p" && in_tx_body && depth == tx_body_depth + 1 {
                    paragraph_texts.push(std::mem::take(&mut current));
                } else if local == "t" {
                    in_text = false;
                }
                depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    paragraph_texts.join("\n")
}

fn parse_theme(xml: &str) -> Result<Theme> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut colors: HashMap<String, Color> = HashMap::new();
    let mut major_font: Option<String> = None;
    let mut minor_font: Option<String> = None;

    let mut current_scheme_color: Option<String> = None;
    let mut in_major_font = false;
    let mut in_minor_font = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "dk1" | "lt1" | "dk2" | "lt2" | "accent1" | "accent2" | "accent3"
                    | "accent4" | "accent5" | "accent6" | "hlink" | "folHlink" | "tx1" | "tx2"
                    | "bg1" | "bg2" => {
                        current_scheme_color = Some(local);
                    }
                    "srgbClr" => {
                        if let Some(key) = &current_scheme_color {
                            if let Some(hex) = attr_by_local_name(&e, "val") {
                                if let Some(color) = parse_hex_color(&hex) {
                                    colors.insert(key.clone(), color);
                                }
                            }
                        }
                    }
                    "sysClr" => {
                        if let Some(key) = &current_scheme_color {
                            if let Some(hex) = attr_by_local_name(&e, "lastClr") {
                                if let Some(color) = parse_hex_color(&hex) {
                                    colors.insert(key.clone(), color);
                                }
                            }
                        }
                    }
                    "majorFont" => in_major_font = true,
                    "minorFont" => in_minor_font = true,
                    "latin" => {
                        if let Some(face) = attr_by_local_name(&e, "typeface") {
                            if in_major_font && major_font.is_none() {
                                major_font = Some(face);
                            } else if in_minor_font && minor_font.is_none() {
                                minor_font = Some(face);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) => {
                let local = qname_str(e.name());
                if local == "clrScheme" || current_scheme_color.as_deref() == Some(local.as_str()) {
                    current_scheme_color = None;
                }
                if local == "majorFont" {
                    in_major_font = false;
                }
                if local == "minorFont" {
                    in_minor_font = false;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let background = colors
        .get("lt1")
        .or_else(|| colors.get("bg1"))
        .copied()
        .unwrap_or(Color::white());
    let accent = colors
        .get("accent1")
        .copied()
        .unwrap_or(Color::rgb(0, 112, 192));

    Ok(Theme {
        background,
        heading_font: major_font.unwrap_or_else(|| "Calibri".to_string()),
        body_font: minor_font.unwrap_or_else(|| "Calibri".to_string()),
        accent_color: accent,
    })
}

fn extract_id(xml: &str, fallback_index: usize) -> String {
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
                        return id;
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    format!("shape-{fallback_index}")
}

fn qname_str(q: QName) -> String {
    String::from_utf8_lossy(q.local_name().as_ref()).into_owned()
}

fn attr_by_local_name(e: &BytesStart<'_>, name: &str) -> Option<String> {
    for attr in e.attributes() {
        let attr = attr.ok()?;
        if attr.key.local_name().as_ref() == name.as_bytes() {
            return Some(attr.unescape_value().ok()?.into_owned());
        }
    }
    None
}

fn rel_attribute(e: &BytesStart<'_>, name: &str) -> Option<String> {
    for attr in e.attributes() {
        let attr = attr.ok()?;
        let key_bytes = attr.key.as_ref();
        let expected = format!("r:{name}").into_bytes();
        if key_bytes == expected.as_slice() {
            return Some(attr.unescape_value().ok()?.into_owned());
        }
        if attr.key.local_name().as_ref() == name.as_bytes() && attr.key.prefix().is_some() {
            return Some(attr.unescape_value().ok()?.into_owned());
        }
    }
    None
}

fn parse_attr_f64(e: &BytesStart<'_>, name: &str) -> Option<f64> {
    attr_by_local_name(e, name)?.parse().ok()
}

fn parse_bool_attr(e: &BytesStart<'_>, name: &str) -> Option<bool> {
    let value = attr_by_local_name(e, name)?;
    Some(value != "0" && !value.eq_ignore_ascii_case("false"))
}

fn parse_underline_attr(e: &BytesStart<'_>) -> bool {
    attr_by_local_name(e, "u")
        .map(|v| !v.eq_ignore_ascii_case("none"))
        .unwrap_or(false)
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Color::rgb(r, g, b))
    } else {
        None
    }
}
