//! ODP writer: export a slides-core [`Deck`] to an ODP (`.odp`) archive.
//!
//! The archive follows the OpenDocument 1.2 Presentation packaging rules:
//!
//! - `mimetype` is stored uncompressed as the very first entry (a hard ODF
//!   requirement so consumers can sniff the format by reading the leading bytes
//!   of the zip stream).
//! - `content.xml` holds the document body — one `draw:page` per slide — plus
//!   the automatic styles the body references.
//! - `styles.xml` declares the page layout (slide dimensions + background) and
//!   the `Default` master page every `draw:page` binds to.
//! - `meta.xml` carries fixed, deterministic metadata.
//! - `Pictures/<key>` holds each image referenced by an `ImageShape`.
//! - `META-INF/manifest.xml` enumerates every file in the package.
//!
//! Output is byte-for-byte deterministic: metadata dates, entry order, and
//! image ordering (by media key) are all fixed.

use std::collections::BTreeSet;
use std::io::{Cursor, Write};

use slides_core::{
    CellAlign, Color, Deck, Fill, GeometricShape, Geometry, ImageShape, Outline, PassthroughObject,
    Run, Shape, SlideSize, TableShape, TextBox,
};
use thiserror::Error;
use zip::write::FileOptions;
use zip::{CompressionMethod, DateTime};

/// ODP presentation MIME type, written uncompressed as the first entry.
const ODP_MIMETYPE: &str = "application/vnd.oasis.opendocument.presentation";

/// EMU per centimeter: `1cm = 360000 EMU`.
const EMU_PER_CM: f64 = 360_000.0;

/// Fixed metadata date (ODF ISO-8601 form) so output is deterministic.
const META_DATE: &str = "2026-01-01T00:00:00";

/// Fixed generator string so output is deterministic.
const GENERATOR: &str = "900Slides";

/// ODF namespaces shared by `content.xml` and `styles.xml`.
const DOC_NAMESPACES: &str = concat!(
    "xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" ",
    "xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" ",
    "xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" ",
    "xmlns:draw=\"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0\" ",
    "xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" ",
    "xmlns:svg=\"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0\" ",
    "xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" ",
    "xmlns:xlink=\"http://www.w3.org/1999/xlink\"",
);

/// Namespaces used by `meta.xml`.
const META_NAMESPACES: &str = concat!(
    "xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" ",
    "xmlns:meta=\"urn:oasis:names:tc:opendocument:xmlns:meta:1.0\" ",
    "xmlns:dc=\"http://purl.org/dc/elements/1.1/\"",
);

/// Namespaces used by `META-INF/manifest.xml`.
const MANIFEST_NAMESPACES: &str =
    "xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\"";

/// Errors returned by [`save`].
#[derive(Debug, Error)]
pub enum Error {
    /// A ZIP read/write error.
    #[error("odp zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    /// A generic I/O error.
    #[error("odp io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for ODP writing.
pub type Result<T> = std::result::Result<T, Error>;

/// Exports a [`Deck`] to the bytes of an ODP (`.odp`) archive.
///
/// Shapes map to the nearest ODP equivalent: text boxes, images, geometric
/// shapes, and tables are emitted directly; charts become a labeled text
/// frame; passthrough objects are emitted verbatim only when they already
/// contain ODF markup.
pub fn save(deck: &Deck) -> Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut out);

        // 1. `mimetype` — uncompressed, stored, first entry (ODF requirement).
        //    All entries use a fixed timestamp (1980-01-01) so the archive is
        //    deterministic across runs and platforms.
        let zero_time = DateTime::default();
        let stored = FileOptions::<()>::default()
            .compression_method(CompressionMethod::Stored)
            .last_modified_time(zero_time);
        writer.start_file("mimetype", stored)?;
        writer.write_all(ODP_MIMETYPE.as_bytes())?;

        let deflated = FileOptions::<()>::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(zero_time);

        // 2. `content.xml` — the deck body.
        let content = build_content_xml(deck);
        writer.start_file("content.xml", deflated)?;
        writer.write_all(content.as_bytes())?;

        // 3. `styles.xml` — page layout + master page.
        let styles = build_styles_xml(deck);
        writer.start_file("styles.xml", deflated)?;
        writer.write_all(styles.as_bytes())?;

        // 4. `meta.xml` — fixed, deterministic metadata.
        let meta = build_meta_xml(deck);
        writer.start_file("meta.xml", deflated)?;
        writer.write_all(meta.as_bytes())?;

        // 5. `Pictures/<key>` — image bytes referenced by ImageShapes, ordered
        //    by media key so the archive is stable across runs.
        for key in picture_keys(deck) {
            if let Some(entry) = deck.media.get(&key) {
                writer.start_file(format!("Pictures/{key}").as_str(), deflated)?;
                writer.write_all(&entry.bytes)?;
            }
        }

        // 6. `META-INF/manifest.xml` — enumerated last, after every file is
        //    known.
        let manifest = build_manifest_xml(deck);
        writer.start_file("META-INF/manifest.xml", deflated)?;
        writer.write_all(manifest.as_bytes())?;

        writer.finish()?;
    }
    Ok(out.into_inner())
}

// ---------------------------------------------------------------------------
// content.xml
// ---------------------------------------------------------------------------

/// Builds the `content.xml` document: automatic text styles plus the body.
fn build_content_xml(deck: &Deck) -> String {
    let mut auto_styles: Vec<String> = Vec::new();
    let mut style_counter = 0u32;

    let mut body = String::from("<office:body><office:presentation>");
    for (index, slide) in deck.slides.iter().enumerate() {
        body.push_str(&format!(
            r#"<draw:page draw:name="slide{index}" draw:master-page-name="Default">"#
        ));
        for shape in &slide.shapes {
            match shape {
                Shape::TextBox(text_box) => body.push_str(&draw_text_box_xml(
                    text_box,
                    &mut style_counter,
                    &mut auto_styles,
                )),
                Shape::Image(image) => body.push_str(&draw_image_xml(image)),
                Shape::Geometric(geometric) => body.push_str(&draw_geometric_xml(geometric)),
                Shape::Table(table) => body.push_str(&draw_table_xml(table)),
                Shape::Chart(chart) => body.push_str(&draw_chart_placeholder(chart)),
                Shape::Passthrough(object) => {
                    if let Some(raw) = passthrough_xml(object) {
                        body.push_str(&raw);
                    }
                }
            }
        }
        body.push_str("</draw:page>");
    }
    body.push_str("</office:presentation></office:body>");

    let automatic_styles = if auto_styles.is_empty() {
        String::from("<office:automatic-styles/>")
    } else {
        format!(
            "<office:automatic-styles>{}</office:automatic-styles>",
            auto_styles.join("")
        )
    };

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <office:document-content {ns} office:version=\"1.2\">\
         {automatic_styles}\
         {body}\
         </office:document-content>",
        ns = DOC_NAMESPACES,
    )
}

/// Emits a `draw:frame/draw:text-box` for a [`TextBox`], generating one
/// automatic text style per formatted run.
fn draw_text_box_xml(
    text_box: &TextBox,
    style_counter: &mut u32,
    auto_styles: &mut Vec<String>,
) -> String {
    let geom = frame_geom(&text_box.frame);
    let mut out = format!("<draw:frame {geom}><draw:text-box>");
    for paragraph in &text_box.paragraphs {
        out.push_str("<text:p>");
        for run in &paragraph.runs {
            out.push_str(&text_span_xml(run, style_counter, auto_styles));
        }
        out.push_str("</text:p>");
    }
    out.push_str("</draw:text-box></draw:frame>");
    out
}

/// Emits a `text:span` for a run, registering an automatic style when the run
/// carries bold/italic/underline/strikethrough formatting.
fn text_span_xml(run: &Run, counter: &mut u32, auto_styles: &mut Vec<String>) -> String {
    let text = esc(&run.text);
    let has_formatting = run.bold || run.italic || run.underline || run.strikethrough;
    if !has_formatting {
        return format!("<text:span>{text}</text:span>");
    }
    let name = format!("T{counter}");
    *counter += 1;
    let mut props = String::new();
    if run.bold {
        props.push_str(r#" fo:font-weight="bold""#);
    }
    if run.italic {
        props.push_str(r#" fo:font-style="italic""#);
    }
    if run.underline {
        props.push_str(r#" style:text-underline-style="solid""#);
    }
    if run.strikethrough {
        props.push_str(r#" style:text-line-through-style="solid""#);
    }
    auto_styles.push(format!(
        "<style:style style:name=\"{name}\" style:family=\"text\">\
         <style:text-properties{props}/>\
         </style:style>"
    ));
    format!(r#"<text:span text:style-name="{name}">{text}</text:span>"#)
}

/// Emits a `draw:frame/draw:image` for an [`ImageShape`], pointing at the
/// stored `Pictures/<key>` entry.
fn draw_image_xml(image: &ImageShape) -> String {
    let geom = frame_geom(&image.transform.frame);
    let href = esc(&format!("Pictures/{}", image.media_ref));
    format!(
        r#"<draw:frame {geom}><draw:image xlink:href="{href}" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"/></draw:frame>"#
    )
}

/// Emits a geometric shape: `draw:rect`, `draw:ellipse`, or `draw:line`.
/// Unknown geometries fall back to a `draw:rect`.
fn draw_geometric_xml(geometric: &GeometricShape) -> String {
    let frame = &geometric.transform.frame;
    let fill = fill_attrs(&geometric.style.fill);
    let stroke = stroke_attrs(geometric.style.outline.as_ref());
    match geometric.geometry {
        Geometry::Rectangle | Geometry::RoundedRectangle { .. } => {
            let geom = frame_geom(frame);
            format!(r#"<draw:rect {geom} {fill} {stroke}/>"#)
        }
        Geometry::Ellipse => {
            let geom = frame_geom(frame);
            format!(r#"<draw:ellipse {geom} {fill} {stroke}/>"#)
        }
        Geometry::Line => {
            let x1 = cm(frame.x);
            let y1 = cm(frame.y);
            let x2 = cm(frame.x + frame.width);
            let y2 = cm(frame.y + frame.height);
            format!(
                r#"<draw:line {fill} {stroke} svg:x1="{x1}cm" svg:y1="{y1}cm" svg:x2="{x2}cm" svg:y2="{y2}cm"/>"#
            )
        }
        Geometry::Triangle | Geometry::Arrow | Geometry::RightArrowCallout | Geometry::Star5 => {
            let geom = frame_geom(frame);
            format!(r#"<draw:rect {geom} {fill} {stroke}/>"#)
        }
    }
}

/// Emits a `table:table` inside a `draw:frame`, one column per model column
/// and one row per model row.
fn draw_table_xml(table: &TableShape) -> String {
    let geom = frame_geom(&table.transform.frame);
    let mut out = format!("<draw:frame {geom}><table:table>");
    for _ in &table.column_widths {
        out.push_str("<table:table-column/>");
    }
    for row in &table.rows {
        out.push_str("<table:table-row>");
        for cell in &row.cells {
            let align = match cell.align {
                CellAlign::Left => "start",
                CellAlign::Center => "center",
                CellAlign::Right => "end",
            };
            let text = esc(&cell.text);
            out.push_str(&format!(
                r#"<table:table-cell table:style-name="ce1" office:value-type="string"><text:p table:style-name="p1" fo:text-align="{align}"><text:span>{text}</text:span></text:p></table:table-cell>"#
            ));
        }
        out.push_str("</table:table-row>");
    }
    out.push_str("</table:table></draw:frame>");
    out
}

/// Emits a labeled text frame as a chart placeholder (ODP charts are complex;
/// the writer does not model them).
fn draw_chart_placeholder(chart: &slides_core::ChartShape) -> String {
    let geom = frame_geom(&chart.transform.frame);
    let label = esc(chart.title.as_deref().unwrap_or("Chart"));
    format!(
        r#"<draw:frame {geom}><draw:text-box><text:p><text:span>{label}</text:span></text:p></draw:text-box></draw:frame>"#
    )
}

/// Emits a passthrough object verbatim only when it already looks like ODF
/// markup; non-ODF (e.g. OOXML) fragments are skipped.
fn passthrough_xml(object: &PassthroughObject) -> Option<String> {
    let raw = std::str::from_utf8(&object.raw_bytes).ok()?;
    if raw.contains("<draw:") || raw.contains("<text:") || raw.contains("<table:") {
        Some(raw.to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// styles.xml
// ---------------------------------------------------------------------------

/// Builds `styles.xml`: a page layout with the deck's slide size and
/// background, plus the `Default` master page every slide binds to.
fn build_styles_xml(deck: &Deck) -> String {
    let (width, height) = slide_size_cm(deck);
    let background = color_hex(&deck.theme.background);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <office:document-styles {ns} office:version=\"1.2\">\
         <office:styles/>\
         <office:automatic-styles>\
         <style:page-layout style:name=\"Mpm1\">\
         <style:page-layout-properties fo:page-width=\"{width:.4}cm\" \
         fo:page-height=\"{height:.4}cm\" fo:background-color=\"{background}\"/>\
         </style:page-layout>\
         </office:automatic-styles>\
         <office:master-styles>\
         <style:master-page style:name=\"Default\" style:page-layout-name=\"Mpm1\"/>\
         </office:master-styles>\
         </office:document-styles>",
        ns = DOC_NAMESPACES,
    )
}

// ---------------------------------------------------------------------------
// meta.xml
// ---------------------------------------------------------------------------

/// Builds `meta.xml` with fixed, deterministic generator and date fields.
fn build_meta_xml(deck: &Deck) -> String {
    let page_count = deck.slides.len();
    let image_count = deck
        .slides
        .iter()
        .flat_map(|slide| slide.shapes.iter())
        .filter(|shape| matches!(shape, Shape::Image(_)))
        .count();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <office:document-meta {ns} office:version=\"1.2\">\
         <office:meta>\
         <meta:generator>{gen}</meta:generator>\
         <meta:creation-date>{date}</meta:creation-date>\
         <dc:date>{date}</dc:date>\
         <meta:document-statistic meta:table-count=\"0\" meta:image-count=\"{image_count}\" \
         meta:object-count=\"0\" meta:page-count=\"{page_count}\"/>\
         </office:meta>\
         </office:document-meta>",
        ns = META_NAMESPACES,
        gen = GENERATOR,
        date = META_DATE,
    )
}

// ---------------------------------------------------------------------------
// META-INF/manifest.xml
// ---------------------------------------------------------------------------

/// Builds `META-INF/manifest.xml`, listing every file in the package.
fn build_manifest_xml(deck: &Deck) -> String {
    let mut entries = String::new();
    entries.push_str(
        r#"<manifest:file-entry manifest:media-type="application/vnd.oasis.opendocument.presentation" manifest:version="1.2" manifest:full-path="/"/>"#,
    );
    entries.push_str(
        r#"<manifest:file-entry manifest:media-type="text/xml" manifest:full-path="content.xml"/>"#,
    );
    entries.push_str(
        r#"<manifest:file-entry manifest:media-type="text/xml" manifest:full-path="styles.xml"/>"#,
    );
    entries.push_str(
        r#"<manifest:file-entry manifest:media-type="text/xml" manifest:full-path="meta.xml"/>"#,
    );
    for key in picture_keys(deck) {
        let mime = deck
            .media
            .get(&key)
            .map(|entry| entry.mime.as_str())
            .unwrap_or("");
        let path = format!("Pictures/{key}");
        entries.push_str(&format!(
            r#"<manifest:file-entry manifest:media-type="{mime}" manifest:full-path="{path}"/>"#,
        ));
    }
    entries.push_str(
        r#"<manifest:file-entry manifest:media-type="text/xml" manifest:full-path="META-INF/manifest.xml"/>"#,
    );
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <manifest:manifest {ns} manifest:version=\"1.2\">{entries}</manifest:manifest>",
        ns = MANIFEST_NAMESPACES,
    )
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Returns the slide size in centimeters, defaulting to 16:9 widescreen.
fn slide_size_cm(deck: &Deck) -> (f64, f64) {
    let size = deck
        .slide_size
        .clone()
        .unwrap_or_else(SlideSize::widescreen_16_9);
    (emu_to_cm(size.width_emu), emu_to_cm(size.height_emu))
}

/// Media keys referenced by `ImageShape`s whose bytes exist in the deck's
/// `MediaStore`, ordered for deterministic output.
fn picture_keys(deck: &Deck) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for slide in &deck.slides {
        for shape in &slide.shapes {
            if let Shape::Image(image) = shape {
                if deck.media.contains_key(&image.media_ref) {
                    keys.insert(image.media_ref.clone());
                }
            }
        }
    }
    keys
}

/// Formats a frame's position and size as `svg:x/y/width/height` attributes.
fn frame_geom(frame: &slides_core::Rect) -> String {
    format!(
        r#"svg:x="{x}cm" svg:y="{y}cm" svg:width="{w}cm" svg:height="{h}cm""#,
        x = cm(frame.x),
        y = cm(frame.y),
        w = cm(frame.width),
        h = cm(frame.height),
    )
}

/// Builds `draw:fill` attributes for an optional fill.
fn fill_attrs(fill: &Option<Fill>) -> String {
    match fill {
        Some(Fill::Solid(color)) => {
            format!(
                r#"draw:fill="solid" draw:fill-color="{}""#,
                color_hex(color)
            )
        }
        None => r#"draw:fill="none""#.to_string(),
    }
}

/// Builds `draw:stroke` attributes for an optional outline.
fn stroke_attrs(outline: Option<&Outline>) -> String {
    match outline {
        Some(outline) => format!(
            r#"draw:stroke="solid" draw:stroke-color="{}" draw:stroke-width="{:.4}cm""#,
            color_hex(&outline.color),
            emu_to_cm(outline.width_emu),
        ),
        None => r#"draw:stroke="none""#.to_string(),
    }
}

/// Converts EMU to centimeters.
fn emu_to_cm(emu: f64) -> f64 {
    emu / EMU_PER_CM
}

/// Formats an EMU length as a fixed-precision centimeter string.
fn cm(emu: f64) -> String {
    format!("{:.4}", emu_to_cm(emu))
}

/// Formats a color as an ODF `#RRGGBB` hex string (alpha is ignored).
fn color_hex(color: &Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

/// Escapes XML special characters for use in text content or attribute values.
fn esc(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}
