//! PPTX loading: convert an OOXML package into a `slides-core` deck.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::QName;
use quick_xml::{Reader, Writer};
use slides_core::{
    Color, Deck, ListStyle, Paragraph, PassthroughObject, Rect, Run, Shape, Slide, TextBox, Theme,
};

use crate::error::{Error, Result};
use crate::ledger::{LossLedger, LossWarning};
use crate::package::{
    find_rel_by_type, parse_rels, Rel, REL_TYPE_MANIFEST, REL_TYPE_OFFICE_DOCUMENT, REL_TYPE_SLIDE,
    REL_TYPE_THEME,
};

const SHAPE_ELEMENT_NAMES: &[&str] =
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

/// Result of loading a PPTX package.
#[derive(Debug)]
pub(crate) struct LoadResult {
    pub deck: Deck,
    pub package_rels: Vec<Rel>,
    pub slide_paths: HashMap<String, String>,
    pub manifest_path: Option<String>,
    pub loss_ledger: LossLedger,
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
        let slide_xml = read_entry_to_string(&mut archive, slide_path)?;
        let mut slide = parse_slide(&slide_xml, slide_path, &mut ledger)?;
        slide.id = slide_path.to_string();
        slide_paths.insert(slide_path.to_string(), slide_path.to_string());
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
    })
}

fn base_dir(part: &str) -> String {
    std::path::Path::new(part)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn rels_path_for(part: &str) -> String {
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

fn parse_slide(xml: &str, slide_id: &str, ledger: &mut LossLedger) -> Result<Slide> {
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
                    let captured_str = String::from_utf8_lossy(&captured);

                    if local == "sp" && captured_str.contains("<p:txBody") {
                        match parse_text_box(&captured_str) {
                            Ok(text_box) => shapes.push(Shape::TextBox(text_box)),
                            Err(err) => {
                                ledger.add(LossWarning::new(
                                    slide_id,
                                    format!("failed to parse text box: {err}"),
                                ));
                                shapes.push(Shape::Passthrough(PassthroughObject {
                                    id: extract_id(&captured_str, shapes.len()),
                                    label: local,
                                    source_part: slide_id.to_string(),
                                    raw_bytes: captured,
                                }));
                            }
                        }
                    } else {
                        ledger.add(LossWarning::new(
                            slide_id,
                            format!("preserved {local} as opaque object; not editable"),
                        ));
                        shapes.push(Shape::Passthrough(PassthroughObject {
                            id: extract_id(&captured_str, shapes.len()),
                            label: local,
                            source_part: slide_id.to_string(),
                            raw_bytes: captured,
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

fn copy_element<R, W>(
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
                            paragraphs.push(Paragraph { runs, list_style });
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
