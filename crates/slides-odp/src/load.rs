//! ODP reader: convert an ODP archive into a slides-core [`Deck`].
//!
//! The reader is intentionally forgiving: unknown elements are preserved as
//! [`Shape::Passthrough`] instead of aborting the load.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::QName;
use quick_xml::{Reader, Writer};
use slides_core::{
    Color, Deck, Fill, GeometricShape, Geometry, ImageShape, MediaEntry, Outline, Paragraph,
    PassthroughObject, Rect, Run, Shape, Slide, SlideSize, Style, TextBox, Theme, Transform,
};

use crate::error::{Error, Result};

/// EMU per centimeter: `1cm = 360000 EMU`.
const EMU_PER_CM: f64 = 360_000.0;
/// EMU per millimeter.
const EMU_PER_MM: f64 = 36_000.0;
/// EMU per inch.
const EMU_PER_IN: f64 = 914_400.0;

/// Parsed text style properties we care about.
#[derive(Debug, Default, Clone)]
struct TextStyleProps {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    font_family: Option<String>,
}

/// Position/size attributes from a `draw:frame` or geometric element.
#[derive(Debug, Default, Clone, Copy)]
struct FrameAttrs {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<FrameAttrs> for Rect {
    fn from(value: FrameAttrs) -> Self {
        Rect::new(value.x, value.y, value.width, value.height)
    }
}

impl From<FrameAttrs> for Transform {
    fn from(value: FrameAttrs) -> Self {
        Transform {
            frame: value.into(),
            rotation: 0.0,
        }
    }
}

/// Opens an ODP file and converts it to the slides-core [`Deck`] model.
///
/// ODP-specific structures without a clean model equivalent are preserved as
/// passthrough shapes rather than panicking or aborting the load.
pub fn load(odp_bytes: &[u8]) -> Result<Deck> {
    let mut archive = open_archive(odp_bytes)?;

    let content_xml = read_entry_to_string(&mut archive, "content.xml")?;
    let styles_xml = read_entry_to_string(&mut archive, "styles.xml").ok();

    let mut deck = Deck::new();

    if let Some(styles) = styles_xml {
        let (theme, slide_size) = parse_styles(&styles)?;
        deck.theme = theme;
        deck.slide_size = slide_size;
    }

    parse_content(&content_xml, &mut deck, &mut archive)?;

    Ok(deck)
}

// ---------------------------------------------------------------------------
// ZIP helpers
// ---------------------------------------------------------------------------

fn open_archive(bytes: &[u8]) -> Result<zip::ZipArchive<std::io::Cursor<&[u8]>>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;

    const MAX_ENTRY: u64 = 50 * 1024 * 1024;
    const MAX_TOTAL: u64 = 500 * 1024 * 1024;
    const MAX_ENTRIES: usize = 65_536;

    if archive.len() > MAX_ENTRIES {
        return Err(Error::ArchiveTooLarge);
    }

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

fn read_entry_to_string(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    path: &str,
) -> Result<String> {
    let mut file = archive.by_name(path)?;
    let mut buf = Vec::with_capacity(file.size() as usize);
    std::io::copy(&mut file, &mut buf)?;
    String::from_utf8(buf).map_err(|_| Error::UnsupportedFormat(format!("non-utf8 part: {path}")))
}

fn read_entry_to_bytes(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    path: &str,
) -> Result<Vec<u8>> {
    let mut file = archive.by_name(path)?;
    let mut buf = Vec::with_capacity(file.size() as usize);
    std::io::copy(&mut file, &mut buf)?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// styles.xml parsing (slide dimensions + theme background)
// ---------------------------------------------------------------------------

fn parse_styles(xml: &str) -> Result<(Theme, Option<SlideSize>)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut theme = Theme::default();
    let mut slide_size: Option<SlideSize> = None;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => {
                if qname_str(e.name()) == "page-layout-properties" {
                    if let Some(width) = parse_length_attr(&e, "page-width") {
                        if let Some(height) = parse_length_attr(&e, "page-height") {
                            slide_size = Some(SlideSize {
                                width_emu: width,
                                height_emu: height,
                            });
                        }
                    }
                    if let Some(color) = parse_color_attr(&e, "background-color") {
                        theme.background = color;
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok((theme, slide_size))
}

// ---------------------------------------------------------------------------
// content.xml parsing
// ---------------------------------------------------------------------------

fn parse_content(
    xml: &str,
    deck: &mut Deck,
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Result<()> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    // Element-name stack for tracking ancestry without namespace prefixes.
    let mut stack: Vec<String> = Vec::new();

    // Automatic text styles collected before / while we read the body.
    let mut styles: HashMap<String, TextStyleProps> = HashMap::new();

    // State for a currently open `style:style` with `style:family="text"`.
    let mut current_style_name: Option<String> = None;
    let mut current_style_props: Option<TextStyleProps> = None;

    // Pending media entry produced by an image parse, inserted into the deck
    // once the mutable archive borrow is released.
    let mut pending_media: Vec<(String, MediaEntry)> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let local = qname_str(e.name());

                // Capture shapes that are direct children of `draw:page`.
                let capture_as_shape = (parent_is(&stack, "page")
                    || parent_is(&stack, "drawing-page"))
                    && matches!(local.as_str(), "frame" | "rect" | "ellipse" | "line");

                if capture_as_shape {
                    let attrs = match local.as_str() {
                        "frame" | "rect" | "ellipse" => parse_frame_attrs(&e),
                        "line" => FrameAttrs::default(),
                        _ => FrameAttrs::default(),
                    };
                    let start = e.into_owned();
                    let mut captured = Vec::new();
                    let mut writer = Writer::new(&mut captured);
                    copy_element(&mut reader, &start, &mut writer, &mut buf)?;
                    let shape = parse_captured_element(
                        &captured,
                        local,
                        attrs,
                        &styles,
                        archive,
                        &mut pending_media,
                    );
                    if let Some(slide) = deck.slides.last_mut() {
                        slide.shapes.push(shape);
                    }
                    // The captured element is fully consumed; do not push it
                    // onto the stack.
                    continue;
                }

                if (local == "page" || local == "drawing-page") && parent_is(&stack, "presentation")
                {
                    let name = attr_by_local_name(&e, "name").unwrap_or_default();
                    deck.slides.push(Slide {
                        id: name,
                        ..Slide::default()
                    });
                }

                if local == "style"
                    && parent_is(&stack, "automatic-styles")
                    && attr_by_local_name(&e, "family").as_deref() == Some("text")
                {
                    current_style_name = attr_by_local_name(&e, "name");
                    current_style_props = Some(TextStyleProps::default());
                }

                stack.push(local);
            }
            Event::Empty(e) => {
                let local = qname_str(e.name());

                if local == "text-properties" && current_style_name.is_some() {
                    if let Some(props) = current_style_props.as_mut() {
                        apply_text_properties(&e, props);
                    }
                }

                if (local == "page" || local == "drawing-page") && parent_is(&stack, "presentation")
                {
                    let name = attr_by_local_name(&e, "name").unwrap_or_default();
                    deck.slides.push(Slide {
                        id: name,
                        ..Slide::default()
                    });
                }
            }
            Event::End(e) => {
                let local = qname_str(e.name());
                if local == "style" {
                    if let (Some(name), Some(props)) =
                        (current_style_name.take(), current_style_props.take())
                    {
                        styles.insert(name, props);
                    }
                }
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    for (key, entry) in pending_media {
        deck.media.insert(key, entry);
    }

    Ok(())
}

fn parent_is(stack: &[String], name: &str) -> bool {
    stack.last().map(|s| s.as_str()) == Some(name)
}

fn apply_text_properties(e: &BytesStart<'_>, props: &mut TextStyleProps) {
    if let Some(v) = attr_by_local_name(e, "font-weight") {
        props.bold = v == "bold";
    }
    if let Some(v) = attr_by_local_name(e, "font-style") {
        props.italic = v == "italic";
    }
    if let Some(v) = attr_by_local_name(e, "text-underline-style") {
        props.underline = v == "solid";
    }
    if let Some(v) = attr_by_local_name(e, "text-line-through-style") {
        props.strikethrough = v == "solid";
    }
    if props.font_family.is_none() {
        props.font_family = attr_by_local_name(e, "font-family");
    }
}

// ---------------------------------------------------------------------------
// Element capture / passthrough
// ---------------------------------------------------------------------------

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
            Event::Eof => return Err(Error::MissingPart("truncated XML".into())),
        }
        buf.clear();
    }
    Ok(())
}

fn parse_captured_element(
    raw: &[u8],
    local: String,
    attrs: FrameAttrs,
    styles: &HashMap<String, TextStyleProps>,
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    pending_media: &mut Vec<(String, MediaEntry)>,
) -> Shape {
    match local.as_str() {
        "frame" => parse_frame(raw, attrs, styles, archive, pending_media),
        "rect" => parse_rect(raw, attrs),
        "ellipse" => parse_ellipse(raw, attrs),
        "line" => parse_line(raw),
        _ => passthrough_shape(raw, &local, Some(attrs.into())),
    }
}

fn parse_frame(
    raw: &[u8],
    attrs: FrameAttrs,
    styles: &HashMap<String, TextStyleProps>,
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    pending_media: &mut Vec<(String, MediaEntry)>,
) -> Shape {
    let kind = detect_child_kind(raw);
    match kind {
        Some(FrameChild::TextBox) => parse_text_box(raw, attrs, styles)
            .map(Shape::TextBox)
            .unwrap_or_else(|_| passthrough_shape(raw, "frame", Some(attrs.into()))),
        Some(FrameChild::Image) => match parse_image(raw, attrs, archive, pending_media) {
            Some(image) => Shape::Image(image),
            None => passthrough_shape(raw, "frame", Some(attrs.into())),
        },
        Some(FrameChild::Rect) => parse_rect(raw, attrs),
        Some(FrameChild::Ellipse) => parse_ellipse(raw, attrs),
        None => passthrough_shape(raw, "frame", Some(attrs.into())),
    }
}

#[derive(Debug, Clone, Copy)]
enum FrameChild {
    TextBox,
    Image,
    Rect,
    Ellipse,
}

fn detect_child_kind(raw: &[u8]) -> Option<FrameChild> {
    let mut reader = Reader::from_reader(raw);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                if depth == 1 {
                    match qname_str(e.name()).as_str() {
                        "text-box" => return Some(FrameChild::TextBox),
                        "image" => return Some(FrameChild::Image),
                        "rect" => return Some(FrameChild::Rect),
                        "ellipse" => return Some(FrameChild::Ellipse),
                        _ => {}
                    }
                }
                depth += 1;
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

fn passthrough_shape(raw: &[u8], label: &str, frame: Option<Rect>) -> Shape {
    Shape::Passthrough(PassthroughObject {
        id: String::new(),
        label: label.to_string(),
        source_part: "content.xml".to_string(),
        raw_bytes: raw.to_vec(),
        frame,
    })
}

// ---------------------------------------------------------------------------
// Shape parsers
// ---------------------------------------------------------------------------

fn parse_text_box(
    raw: &[u8],
    attrs: FrameAttrs,
    styles: &HashMap<String, TextStyleProps>,
) -> Result<TextBox> {
    let mut reader = Reader::from_reader(raw);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut current_runs: Option<Vec<Run>> = None;
    let mut current_run_text = String::new();
    let mut current_run_style: Option<String> = None;
    let mut in_text_box = false;
    let mut in_paragraph = false;
    let mut in_span = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "text-box" => in_text_box = true,
                    "p" if in_text_box => {
                        in_paragraph = true;
                        current_runs = Some(Vec::new());
                    }
                    "span" if in_paragraph => {
                        in_span = true;
                        current_run_text.clear();
                        current_run_style = attr_by_local_name(&e, "style-name");
                    }
                    _ => {}
                }
            }
            Event::Empty(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "p" if in_text_box => {
                        paragraphs.push(Paragraph::default());
                    }
                    "span" if in_paragraph => {
                        let style = attr_by_local_name(&e, "style-name");
                        let run = build_run("", style.as_deref(), styles);
                        if let Some(runs) = current_runs.as_mut() {
                            runs.push(run);
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(t) => {
                if in_span {
                    current_run_text.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Event::End(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "text-box" => in_text_box = false,
                    "p" if in_text_box => {
                        in_paragraph = false;
                        if let Some(runs) = current_runs.take() {
                            paragraphs.push(Paragraph {
                                runs,
                                ..Paragraph::default()
                            });
                        }
                    }
                    "span" if in_paragraph => {
                        in_span = false;
                        let run =
                            build_run(&current_run_text, current_run_style.as_deref(), styles);
                        if let Some(runs) = current_runs.as_mut() {
                            runs.push(run);
                        }
                        current_run_text.clear();
                        current_run_style = None;
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(TextBox {
        id: String::new(),
        frame: attrs.into(),
        paragraphs,
    })
}

fn build_run(
    text: &str,
    style_name: Option<&str>,
    styles: &HashMap<String, TextStyleProps>,
) -> Run {
    let mut run = Run::new(text);
    if let Some(name) = style_name {
        if let Some(props) = styles.get(name) {
            run.bold = props.bold;
            run.italic = props.italic;
            run.underline = props.underline;
            run.strikethrough = props.strikethrough;
            if let Some(family) = &props.font_family {
                run.font_family = Some(family.clone());
            }
        }
    }
    run
}

fn parse_image(
    raw: &[u8],
    attrs: FrameAttrs,
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    pending_media: &mut Vec<(String, MediaEntry)>,
) -> Option<ImageShape> {
    let href = extract_image_href(raw)?;
    let path = href.trim_start_matches("./").to_string();
    let bytes = read_entry_to_bytes(archive, &path).ok()?;

    let media_ref = path.strip_prefix("Pictures/").unwrap_or(&path).to_string();
    let mime = infer_mime(&path, &bytes);
    let (width, height) = image_dimensions(&bytes);

    pending_media.push((
        media_ref.clone(),
        MediaEntry {
            mime,
            bytes,
            width,
            height,
        },
    ));

    Some(ImageShape {
        id: String::new(),
        transform: attrs.into(),
        media_ref,
        crop: None,
        alt_text: None,
    })
}

fn extract_image_href(raw: &[u8]) -> Option<String> {
    let mut reader = Reader::from_reader(raw);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                if qname_str(e.name()) == "image" {
                    if let Some(href) = attr_by_local_name(&e, "href") {
                        return Some(href);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

fn parse_rect(raw: &[u8], attrs: FrameAttrs) -> Shape {
    let style = parse_geometric_style(raw);
    Shape::Geometric(GeometricShape {
        id: String::new(),
        transform: attrs.into(),
        geometry: Geometry::Rectangle,
        style,
    })
}

fn parse_ellipse(raw: &[u8], attrs: FrameAttrs) -> Shape {
    let style = parse_geometric_style(raw);
    Shape::Geometric(GeometricShape {
        id: String::new(),
        transform: attrs.into(),
        geometry: Geometry::Ellipse,
        style,
    })
}

fn parse_line(raw: &[u8]) -> Shape {
    let mut reader = Reader::from_reader(raw);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut x1 = 0.0f64;
    let mut y1 = 0.0f64;
    let mut x2 = 0.0f64;
    let mut y2 = 0.0f64;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                if qname_str(e.name()) == "line" {
                    x1 = parse_length_attr(&e, "x1").unwrap_or(0.0);
                    y1 = parse_length_attr(&e, "y1").unwrap_or(0.0);
                    x2 = parse_length_attr(&e, "x2").unwrap_or(0.0);
                    y2 = parse_length_attr(&e, "y2").unwrap_or(0.0);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let frame = Rect::new(x1, y1, x2 - x1, y2 - y1);
    Shape::Geometric(GeometricShape {
        id: String::new(),
        transform: Transform {
            frame,
            rotation: 0.0,
        },
        geometry: Geometry::Line,
        style: Style::default(),
    })
}

fn parse_geometric_style(raw: &[u8]) -> Style {
    let mut reader = Reader::from_reader(raw);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut fill: Option<Fill> = None;
    let mut outline: Option<Outline> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                let local = qname_str(e.name());
                if local == "rect" || local == "ellipse" {
                    if let Some(color) = parse_color_attr(&e, "fill-color") {
                        fill = Some(Fill::Solid(color));
                    }
                    if let Some(color) = parse_color_attr(&e, "stroke-color") {
                        let width = parse_length_attr(&e, "stroke-width").unwrap_or(0.0);
                        outline = Some(Outline {
                            color,
                            width_emu: width,
                            dash: slides_core::DashStyle::Solid,
                        });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Style {
        fill,
        outline,
        shadow: None,
    }
}

// ---------------------------------------------------------------------------
// Attribute helpers
// ---------------------------------------------------------------------------

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

fn parse_frame_attrs(e: &BytesStart<'_>) -> FrameAttrs {
    FrameAttrs {
        x: parse_length_attr(e, "x").unwrap_or(0.0),
        y: parse_length_attr(e, "y").unwrap_or(0.0),
        width: parse_length_attr(e, "width").unwrap_or(0.0),
        height: parse_length_attr(e, "height").unwrap_or(0.0),
    }
}

fn parse_length_attr(e: &BytesStart<'_>, name: &str) -> Option<f64> {
    let value = attr_by_local_name(e, name)?;
    parse_length(&value)
}

fn parse_length(value: &str) -> Option<f64> {
    let value = value.trim();
    if let Some(cm) = value.strip_suffix("cm") {
        cm.trim().parse::<f64>().ok().map(|v| v * EMU_PER_CM)
    } else if let Some(mm) = value.strip_suffix("mm") {
        mm.trim().parse::<f64>().ok().map(|v| v * EMU_PER_MM)
    } else if let Some(inch) = value.strip_suffix("in") {
        inch.trim().parse::<f64>().ok().map(|v| v * EMU_PER_IN)
    } else {
        // Plain numeric value assumed to be EMU.
        value.parse::<f64>().ok()
    }
}

fn parse_color_attr(e: &BytesStart<'_>, name: &str) -> Option<Color> {
    let value = attr_by_local_name(e, name)?;
    parse_color(&value)
}

fn parse_color(value: &str) -> Option<Color> {
    let hex = value.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Color::rgb(r, g, b))
    } else {
        None
    }
}

fn infer_mime(path: &str, bytes: &[u8]) -> String {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") || bytes.starts_with(b"\x89PNG") {
        "image/png".to_string()
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

fn image_dimensions(bytes: &[u8]) -> (u32, u32) {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        // PNG IHDR dimensions are at offsets 16-23.
        if bytes.len() >= 24 {
            let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
            return (width, height);
        }
    }
    (0, 0)
}
