//! Deterministic slide rendering to SVG.
//!
//! The renderer is pure: it takes immutable references to a slide, theme, and
//! media store and produces an SVG string plus a stable hash. It performs no
//! I/O, makes no network calls, and uses no global state, so the same inputs
//! always produce byte-identical output (`PRODUCT_SPEC.md` §6.5).
//!
//! SVG user units are EMU directly: the `viewBox` width and height are the
//! slide dimensions in EMU, so all coordinates and sizes below are in EMU
//! (`PRODUCT_SPEC.md` §7.3).

use std::hash::Hasher;

use base64::Engine as _;
use slides_core::{
    CellAlign, ChartShape, Color, DashStyle, Fill, GeometricShape, Geometry, HeadingLevel,
    ImageShape, ListStyle, MediaStore, Outline, Paragraph, PassthroughObject, Rect, Run, Shadow,
    Shape, Style, TableBorders, TableCell, TableShape, TextBox, Theme, VerticalAlign,
};

/// Horizontal padding inside a text box, in EMU (0.1 inch).
const TEXT_PADDING_EMU: f64 = 91_440.0;
/// Body font size, in EMU (18 pt = 18 * 12700).
const TEXT_FONT_SIZE_EMU: f64 = 228_600.0;
/// Constant estimated line height between paragraphs, in EMU (~0.4 inch).
const TEXT_LINE_HEIGHT_EMU: f64 = 360_000.0;
/// Horizontal indent per level, in EMU (~0.25 inch).
const INDENT_EMU: f64 = 360_000.0;
/// Extra left indent for blockquote paragraphs, in EMU.
const BLOCKQUOTE_INDENT_EMU: f64 = 180_000.0;
/// Width of the blockquote left border, in EMU.
const BLOCKQUOTE_BORDER_WIDTH_EMU: f64 = 30_000.0;
/// Color of the blockquote left border.
const BLOCKQUOTE_BORDER_COLOR: &str = "#cccccc";
/// Font stack used for code blocks and inline code.
const CODE_BLOCK_FONT: &str = "Courier New, monospace";
/// Background color for code blocks.
const CODE_BLOCK_BACKGROUND: &str = "#f5f5f5";
/// Relative font size for superscript/subscript runs.
const SCRIPT_FONT_SIZE: &str = "0.7em";
/// Fill color used for the header row when a header cell has no explicit fill.
const TABLE_HEADER_FILL: &str = "#d9e1f2";

/// Options controlling the dimensions of a rendered slide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    /// Slide width, in EMU.
    pub width_emu: f64,
    /// Slide height, in EMU.
    pub height_emu: f64,
}

impl Default for RenderOptions {
    /// The PPTX default 16:9 slide: 12,192,000 x 6,858,000 EMU.
    fn default() -> Self {
        Self {
            width_emu: 12_192_000.0,
            height_emu: 6_858_000.0,
        }
    }
}

/// A rendered slide: its SVG markup and a stable hash of that markup.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedSlide {
    /// The SVG markup for the slide, as a single `<svg>` document.
    pub svg: String,
    /// Stable 64-bit hash of the `svg` bytes (see [`render_slide`]).
    pub hash: u64,
}

/// Renders a single slide to deterministic SVG.
///
/// The output is one `<svg viewBox="0 0 W H" xmlns="http://www.w3.org/2000/svg">`
/// element where `W` and `H` are the slide dimensions in EMU. The theme
/// background is painted first, then shapes are emitted in `slide.shapes`
/// order. Shadow filters are defined in a `<defs>` block with stable ids
/// `sh0`, `sh1`, ... assigned by shape render order.
///
/// The `hash` is computed with [`twox_hash::XxHash64`] seeded with `0` over
/// the final `svg` bytes. xxHash is a fixed, seed-documented algorithm (unlike
/// `std::collections::hash_map::DefaultHasher`, whose output is not stable
/// across Rust versions), so the same inputs yield the same `svg` and `hash`
/// across runs and machines.
pub fn render_slide(
    slide: &slides_core::Slide,
    theme: &Theme,
    media: &MediaStore,
    opts: &RenderOptions,
) -> RenderedSlide {
    let width = opts.width_emu;
    let height = opts.height_emu;

    let mut defs = String::new();
    let mut body = String::new();
    let mut shadow_counter: usize = 0;

    for shape in &slide.shapes {
        match shape {
            Shape::TextBox(text_box) => render_text_box(text_box, theme, &mut body),
            Shape::Passthrough(object) => render_passthrough(object, &mut body),
            Shape::Image(image) => render_image(image, media, &mut body),
            Shape::Geometric(geometric) => {
                let filter_id = geometric.style.shadow.as_ref().map(|shadow| {
                    let id = shadow_counter;
                    shadow_counter += 1;
                    push_shadow_filter(id, shadow, &mut defs);
                    id
                });
                render_geometric(geometric, filter_id, &mut body);
            }
            Shape::Table(table) => render_table(table, theme, &mut body),
            Shape::Chart(chart) => render_chart(chart, &mut body),
        }
    }

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg viewBox=\"0 0 {w} {h}\" xmlns=\"http://www.w3.org/2000/svg\">",
        w = fnum(width),
        h = fnum(height)
    ));
    svg.push_str(&format!(
        "<rect width=\"{w}\" height=\"{h}\" fill=\"{fill}\"/>",
        w = fnum(width),
        h = fnum(height),
        fill = hex_color(&theme.background)
    ));
    if !defs.is_empty() {
        svg.push_str("<defs>");
        svg.push_str(&defs);
        svg.push_str("</defs>");
    }
    svg.push_str(&body);
    svg.push_str("</svg>");

    let mut hasher = twox_hash::XxHash64::default();
    hasher.write(svg.as_bytes());
    let hash = hasher.finish();

    RenderedSlide { svg, hash }
}

/// Renders a chart by delegating to [`slides_chart::render_chart_svg`] and
/// embedding the result as a nested `<svg>` positioned at the chart's frame.
///
/// The chart crate emits a standalone `<svg viewBox="0 0 W H">` document sized
/// to the dimensions it is given. To place it on the slide without it expanding
/// to fill the entire viewport, the `x`, `y`, `width`, and `height` attributes
/// are injected into the root tag, turning it into a nested viewport. This is
/// safe because the chart output contains exactly one `<svg` token (its root);
/// every chart primitive is a `<rect>`/`<text>`/`<path>`/`<circle>`/`<polyline>`.
fn render_chart(chart: &ChartShape, out: &mut String) {
    let frame = &chart.transform.frame;
    let mut svg = slides_chart::render_chart_svg(chart, frame.width, frame.height);
    let header = format!(
        "<svg x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" ",
        x = fnum(frame.x),
        y = fnum(frame.y),
        w = fnum(frame.width),
        h = fnum(frame.height),
    );
    svg = svg.replacen("<svg ", &header, 1);
    out.push_str(&svg);
}

/// Formats an EMU/coordinate value as a deterministic string.
///
/// Whole numbers render without a decimal point (e.g. `12192000`); fractional
/// values use Rust's shortest round-trip representation. The output depends
/// only on the input value, never on locale or formatting flags.
fn fnum(value: f64) -> String {
    format!("{value}")
}

/// Formats a [`Color`] as an opaque `#rrggbb` hex string (alpha is ignored).
fn hex_color(color: &Color) -> String {
    format!(
        "#{r:02x}{g:02x}{b:02x}",
        r = color.r,
        g = color.g,
        b = color.b
    )
}

/// Escapes the five XML special characters (`& < > " '`) for use in text
/// content or attribute values.
fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Pushes a drop-shadow `<filter>` definition with the given stable id.
fn push_shadow_filter(id: usize, shadow: &Shadow, out: &mut String) {
    let sid = format!("sh{id}");
    let dx = fnum(shadow.offset_x);
    let dy = fnum(shadow.offset_y);
    let blur = fnum(shadow.blur);
    let color = hex_color(&shadow.color);
    let opacity = fnum(shadow.opacity);
    out.push_str(&format!(
        "<filter id=\"{sid}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\">"
    ));
    out.push_str(&format!(
        "<feDropShadow dx=\"{dx}\" dy=\"{dy}\" stdDeviation=\"{blur}\" flood-color=\"{color}\" flood-opacity=\"{opacity}\"/>"
    ));
    out.push_str("</filter>");
}

/// Pushes a `transform="rotate(rot, cx, cy)"` attribute when rotation is non-zero.
fn push_transform(out: &mut String, rotation: f64, cx: f64, cy: f64) {
    if rotation != 0.0 {
        let rot = fnum(rotation);
        let cx = fnum(cx);
        let cy = fnum(cy);
        out.push_str(&format!(" transform=\"rotate({rot},{cx},{cy})\""));
    }
}

/// Pushes a `filter="url(#shN)"` attribute when a shadow filter was assigned.
fn push_filter(out: &mut String, filter_id: Option<usize>) {
    if let Some(id) = filter_id {
        out.push_str(&format!(" filter=\"url(#sh{id})\""));
    }
}

/// Pushes the shared style attributes (fill, stroke, stroke-width,
/// stroke-dasharray) in a fixed order.
fn push_style(out: &mut String, style: &Style) {
    match &style.fill {
        Some(Fill::Solid(color)) => {
            let fill = hex_color(color);
            out.push_str(&format!(" fill=\"{fill}\""));
        }
        None => out.push_str(" fill=\"none\""),
    }
    if let Some(outline) = &style.outline {
        push_outline(out, outline);
    }
}

/// Pushes `stroke`, `stroke-width`, and (optionally) `stroke-dasharray`.
fn push_outline(out: &mut String, outline: &Outline) {
    let stroke = hex_color(&outline.color);
    let width = fnum(outline.width_emu);
    out.push_str(&format!(" stroke=\"{stroke}\" stroke-width=\"{width}\""));
    push_dash_array(out, &outline.dash);
}

/// Pushes a `stroke-dasharray` attribute for a non-solid dash style.
fn push_dash_array(out: &mut String, dash: &DashStyle) {
    match dash {
        DashStyle::Solid => {}
        DashStyle::Dash => out.push_str(" stroke-dasharray=\"300000,150000\""),
        DashStyle::Dot => out.push_str(" stroke-dasharray=\"60000,60000\""),
        DashStyle::DashDot => out.push_str(" stroke-dasharray=\"300000,150000,60000,150000\""),
    }
}

/// Renders a text box as a `<g>` containing one `<text>` per paragraph.
fn render_text_box(text_box: &TextBox, theme: &Theme, out: &mut String) {
    out.push_str("<g>");
    let base_x = text_box.frame.x + TEXT_PADDING_EMU;
    let line_height = TEXT_LINE_HEIGHT_EMU;
    for (index, paragraph) in text_box.paragraphs.iter().enumerate() {
        let style = &paragraph.style;
        let logical_x = base_x + style.indent_level as f64 * INDENT_EMU;
        let x = logical_x
            + if style.blockquote {
                BLOCKQUOTE_INDENT_EMU
            } else {
                0.0
            };
        let y = text_box.frame.y + line_height * (index as f64 + 1.0);

        if style.code_block {
            render_code_block_background(text_box, logical_x, y, line_height, out);
        }

        if style.blockquote {
            render_blockquote_border(logical_x, y, line_height, out);
        }

        let (font, font_size, font_weight, font_style) = paragraph_font(paragraph, theme);
        let x = fnum(x);
        let y = fnum(y);
        let font_size = fnum(font_size);
        let font = escape_xml(font);
        out.push_str(&format!(
            "<text x=\"{x}\" y=\"{y}\" font-family=\"{font}\" font-size=\"{font_size}\" font-weight=\"{font_weight}\" font-style=\"{font_style}\">"
        ));

        if !style.code_block {
            push_list_marker(paragraph, index, out);
        }

        for run in &paragraph.runs {
            push_run(run, out);
        }
        out.push_str("</text>");
    }
    out.push_str("</g>");
}

/// Returns the font family, size, weight, and style for a paragraph.
fn paragraph_font<'a>(
    paragraph: &'a Paragraph,
    theme: &'a Theme,
) -> (&'a str, f64, &'static str, &'static str) {
    if paragraph.style.code_block {
        return (CODE_BLOCK_FONT, TEXT_FONT_SIZE_EMU, "normal", "normal");
    }
    match paragraph.style.heading {
        Some(level) => {
            let size = heading_size(level);
            (&theme.heading_font, size, "bold", "normal")
        }
        None => {
            let style = if paragraph.style.blockquote {
                "italic"
            } else {
                "normal"
            };
            (&theme.body_font, TEXT_FONT_SIZE_EMU, "normal", style)
        }
    }
}

/// Font size for a heading level, in EMU.
fn heading_size(level: HeadingLevel) -> f64 {
    match level {
        HeadingLevel::H1 => TEXT_FONT_SIZE_EMU * 2.0,
        HeadingLevel::H2 => TEXT_FONT_SIZE_EMU * 1.5,
        HeadingLevel::H3 => TEXT_FONT_SIZE_EMU * 1.25,
        HeadingLevel::H4 => TEXT_FONT_SIZE_EMU * 1.1,
        HeadingLevel::H5 => TEXT_FONT_SIZE_EMU,
        HeadingLevel::H6 => TEXT_FONT_SIZE_EMU * 0.9,
    }
}

/// Renders a light background rectangle behind a code-block paragraph.
fn render_code_block_background(
    text_box: &TextBox,
    logical_x: f64,
    y: f64,
    line_height: f64,
    out: &mut String,
) {
    let rect_x = logical_x - TEXT_PADDING_EMU / 2.0;
    let rect_y = y - line_height * 0.8;
    let width = text_box.frame.width - (logical_x - text_box.frame.x) - TEXT_PADDING_EMU / 2.0;
    let height = line_height;
    let rect_x = fnum(rect_x);
    let rect_y = fnum(rect_y);
    let width = fnum(width);
    let height = fnum(height);
    out.push_str(&format!(
        "<rect x=\"{rect_x}\" y=\"{rect_y}\" width=\"{width}\" height=\"{height}\" fill=\"{CODE_BLOCK_BACKGROUND}\" stroke=\"none\"/>"
    ));
}

/// Renders a vertical left border for a blockquote paragraph.
fn render_blockquote_border(logical_x: f64, y: f64, line_height: f64, out: &mut String) {
    let rect_x = logical_x;
    let rect_y = y - line_height * 0.8;
    let width = BLOCKQUOTE_BORDER_WIDTH_EMU;
    let height = line_height;
    let rect_x = fnum(rect_x);
    let rect_y = fnum(rect_y);
    let width = fnum(width);
    let height = fnum(height);
    out.push_str(&format!(
        "<rect x=\"{rect_x}\" y=\"{rect_y}\" width=\"{width}\" height=\"{height}\" fill=\"{BLOCKQUOTE_BORDER_COLOR}\" stroke=\"none\"/>"
    ));
}

/// Pushes the list marker `<tspan>` for a paragraph, if any.
fn push_list_marker(paragraph: &Paragraph, index: usize, out: &mut String) {
    match paragraph.list_style {
        ListStyle::None => {}
        ListStyle::Unordered => out.push_str("<tspan>\u{2022} </tspan>"),
        ListStyle::Ordered => {
            let n = index + 1;
            out.push_str(&format!("<tspan>{n}.</tspan> "))
        }
    }
}

/// Pushes a run as a `<tspan>` with run-level formatting.
fn push_run(run: &Run, out: &mut String) {
    if let Some(link) = &run.link {
        let href = escape_xml(&link.url);
        out.push_str(&format!("<a href=\"{href}\">"));
    }

    out.push_str("<tspan");
    if run.bold {
        out.push_str(" font-weight=\"bold\"");
    }
    if run.italic {
        out.push_str(" font-style=\"italic\"");
    }
    match (run.underline, run.strikethrough) {
        (true, true) => out.push_str(" text-decoration=\"underline line-through\""),
        (true, false) => out.push_str(" text-decoration=\"underline\""),
        (false, true) => out.push_str(" text-decoration=\"line-through\""),
        (false, false) => {}
    }
    match run.vertical_align {
        VerticalAlign::Baseline => {}
        VerticalAlign::Superscript => {
            out.push_str(" baseline-shift=\"super\"");
            out.push_str(&format!(" font-size=\"{SCRIPT_FONT_SIZE}\""));
        }
        VerticalAlign::Subscript => {
            out.push_str(" baseline-shift=\"sub\"");
            out.push_str(&format!(" font-size=\"{SCRIPT_FONT_SIZE}\""));
        }
    }
    if run.code {
        let family = run.font_family.as_deref().unwrap_or(CODE_BLOCK_FONT);
        let family = escape_xml(family);
        out.push_str(&format!(" font-family=\"{family}\" font-style=\"normal\""));
    } else if let Some(family) = &run.font_family {
        let family = escape_xml(family);
        out.push_str(&format!(" font-family=\"{family}\""));
    }
    out.push('>');
    out.push_str(&escape_xml(&run.text));
    out.push_str("</tspan>");

    if run.link.is_some() {
        out.push_str("</a>");
    }
}

/// Renders a geometric shape based on its [`Geometry`].
fn render_geometric(geometric: &GeometricShape, filter_id: Option<usize>, out: &mut String) {
    let frame = geometric.transform.frame;
    let cx = frame.x + frame.width / 2.0;
    let cy = frame.y + frame.height / 2.0;
    let style = &geometric.style;
    let rotation = geometric.transform.rotation;

    match geometric.geometry {
        Geometry::Rectangle => {
            push_rect_open(out, frame.x, frame.y, frame.width, frame.height, None);
            push_style(out, style);
            push_transform(out, rotation, cx, cy);
            push_filter(out, filter_id);
            out.push_str("/>");
        }
        Geometry::RoundedRectangle { radius } => {
            push_rect_open(
                out,
                frame.x,
                frame.y,
                frame.width,
                frame.height,
                Some(radius),
            );
            push_style(out, style);
            push_transform(out, rotation, cx, cy);
            push_filter(out, filter_id);
            out.push_str("/>");
        }
        Geometry::Ellipse => {
            let cx_s = fnum(cx);
            let cy_s = fnum(cy);
            let rx = fnum(frame.width / 2.0);
            let ry = fnum(frame.height / 2.0);
            out.push_str(&format!(
                "<ellipse cx=\"{cx_s}\" cy=\"{cy_s}\" rx=\"{rx}\" ry=\"{ry}\""
            ));
            push_style(out, style);
            push_transform(out, rotation, cx, cy);
            push_filter(out, filter_id);
            out.push_str("/>");
        }
        Geometry::Triangle => {
            let x1 = fnum(frame.x);
            let y1 = fnum(frame.y + frame.height);
            let x2 = fnum(frame.x + frame.width);
            let y2 = fnum(frame.y + frame.height);
            let x3 = fnum(frame.x + frame.width / 2.0);
            let y3 = fnum(frame.y);
            out.push_str(&format!(
                "<polygon points=\"{x1},{y1} {x2},{y2} {x3},{y3}\""
            ));
            push_style(out, style);
            push_transform(out, rotation, cx, cy);
            push_filter(out, filter_id);
            out.push_str("/>");
        }
        Geometry::Line => {
            // Horizontal line across the frame at its vertical center.
            let x1 = fnum(frame.x);
            let y1 = fnum(cy);
            let x2 = fnum(frame.x + frame.width);
            let y2 = fnum(cy);
            out.push_str(&format!(
                "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\""
            ));
            push_style(out, style);
            push_transform(out, rotation, cx, cy);
            push_filter(out, filter_id);
            out.push_str("/>");
        }
        Geometry::Arrow => {
            let d = arrow_path(frame);
            out.push_str(&format!("<path d=\"{d}\""));
            push_style(out, style);
            push_transform(out, rotation, cx, cy);
            push_filter(out, filter_id);
            out.push_str("/>");
        }
        Geometry::RightArrowCallout => {
            let d = right_arrow_callout_path(frame);
            out.push_str(&format!("<path d=\"{d}\""));
            push_style(out, style);
            push_transform(out, rotation, cx, cy);
            push_filter(out, filter_id);
            out.push_str("/>");
        }
        Geometry::Star5 => {
            let d = star5_path(frame);
            out.push_str(&format!("<path d=\"{d}\""));
            push_style(out, style);
            push_transform(out, rotation, cx, cy);
            push_filter(out, filter_id);
            out.push_str("/>");
        }
    }
}

/// Pushes the opening of a `<rect>` element up to (but not including) the
/// style/transform/filter attributes.
fn push_rect_open(out: &mut String, x: f64, y: f64, width: f64, height: f64, radius: Option<f64>) {
    let x = fnum(x);
    let y = fnum(y);
    let width = fnum(width);
    let height = fnum(height);
    out.push_str(&format!(
        "<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\""
    ));
    if let Some(radius) = radius {
        let radius = fnum(radius);
        out.push_str(&format!(" rx=\"{radius}\" ry=\"{radius}\""));
    }
}

/// Builds a simple right-pointing block-arrow path filling `frame`.
///
/// The arrow has a rectangular shaft occupying the middle third of the height
/// and a triangular head spanning the full frame height.
fn arrow_path(frame: Rect) -> String {
    let x = frame.x;
    let y = frame.y;
    let w = frame.width;
    let h = frame.height;
    let shaft_top = fnum(y + h / 3.0);
    let shaft_bot = fnum(y + 2.0 * h / 3.0);
    let head_base = fnum(x + 2.0 * w / 3.0);
    let tip = fnum(x + w);
    let mid = fnum(y + h / 2.0);
    let left = fnum(x);
    let top = fnum(y);
    let bottom = fnum(y + h);
    format!(
        "M {left},{shaft_top} L {head_base},{shaft_top} L {head_base},{top} L {tip},{mid} L {head_base},{bottom} L {head_base},{shaft_bot} L {left},{shaft_bot} Z"
    )
}

/// Builds a right-arrow callout path: a rectangle body filling the left
/// two-thirds of `frame`, with a triangular point on the right edge.
fn right_arrow_callout_path(frame: Rect) -> String {
    let x = frame.x;
    let y = frame.y;
    let w = frame.width;
    let h = frame.height;
    let body_right = fnum(x + 2.0 * w / 3.0);
    let tip = fnum(x + w);
    let mid = fnum(y + h / 2.0);
    let q1 = fnum(y + h / 4.0);
    let q3 = fnum(y + 3.0 * h / 4.0);
    let left = fnum(x);
    let top = fnum(y);
    let bottom = fnum(y + h);
    format!(
        "M {left},{top} L {body_right},{top} L {body_right},{q1} L {tip},{mid} L {body_right},{q3} L {body_right},{bottom} L {left},{bottom} Z"
    )
}

/// Builds a five-pointed star path filling `frame` using the canonical
/// outer/inner radius ratio (inner = outer * 0.381966, the golden-ratio
/// complement).
fn star5_path(frame: Rect) -> String {
    let cx = frame.x + frame.width / 2.0;
    let cy = frame.y + frame.height / 2.0;
    let outer = frame.width.min(frame.height) / 2.0;
    let inner = outer * 0.381_966_011_250_105_1;
    let mut d = String::from("M");
    for i in 0..10 {
        let angle = (-90.0 + i as f64 * 36.0).to_radians();
        let radius = if i % 2 == 0 { outer } else { inner };
        let px = fnum(cx + radius * angle.cos());
        let py = fnum(cy + radius * angle.sin());
        d.push_str(&format!(" {px},{py}"));
    }
    d.push_str(" Z");
    d
}

/// Renders an image, embedding its bytes as a base64 data URI. Missing media
/// renders a labelled placeholder instead of panicking.
fn render_image(image: &ImageShape, media: &MediaStore, out: &mut String) {
    let frame = image.transform.frame;
    let cx = frame.x + frame.width / 2.0;
    let cy = frame.y + frame.height / 2.0;

    let Some(entry) = media.get(&image.media_ref) else {
        render_missing_image(&image.media_ref, frame, out);
        return;
    };

    let b64 = base64::engine::general_purpose::STANDARD.encode(&entry.bytes);
    let href = format!("data:{mime};base64,{b64}", mime = entry.mime);
    let native_w = fnum(entry.width as f64);
    let native_h = fnum(entry.height as f64);

    match &image.crop {
        Some(crop) => {
            let left_px = fnum(crop.left * entry.width as f64);
            let top_px = fnum(crop.top * entry.height as f64);
            let vis_w = fnum((1.0 - crop.left - crop.right) * entry.width as f64);
            let vis_h = fnum((1.0 - crop.top - crop.bottom) * entry.height as f64);
            let x = fnum(frame.x);
            let y = fnum(frame.y);
            let width = fnum(frame.width);
            let height = fnum(frame.height);
            out.push_str(&format!(
                "<svg x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" viewBox=\"{left_px} {top_px} {vis_w} {vis_h}\" preserveAspectRatio=\"none\""
            ));
            push_transform(out, image.transform.rotation, cx, cy);
            out.push('>');
            out.push_str(&format!(
                "<image x=\"0\" y=\"0\" width=\"{native_w}\" height=\"{native_h}\" href=\"{href}\"/>"
            ));
            out.push_str("</svg>");
        }
        None => {
            let x = fnum(frame.x);
            let y = fnum(frame.y);
            let width = fnum(frame.width);
            let height = fnum(frame.height);
            out.push_str(&format!(
                "<image x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" href=\"{href}\""
            ));
            push_transform(out, image.transform.rotation, cx, cy);
            out.push_str(" preserveAspectRatio=\"none\"/>");
        }
    }
}

/// Renders a placeholder for an image whose `media_ref` is absent from the store.
fn render_missing_image(media_ref: &str, frame: Rect, out: &mut String) {
    let x = fnum(frame.x);
    let y = fnum(frame.y);
    let width = fnum(frame.width);
    let height = fnum(frame.height);
    out.push_str(&format!(
        "<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" fill=\"#cccccc\" stroke=\"#666666\" stroke-width=\"20000\"/>"
    ));
    let label = escape_xml(media_ref);
    let text_x = fnum(frame.x + TEXT_PADDING_EMU);
    let text_y = fnum(frame.y + TEXT_LINE_HEIGHT_EMU);
    out.push_str(&format!(
        "<text x=\"{text_x}\" y=\"{text_y}\" font-family=\"sans-serif\" font-size=\"150000\">Missing image: {label}</text>"
    ));
}

/// Renders a passthrough object as a dashed bounding-box `<rect>` with its label.
fn render_passthrough(object: &PassthroughObject, out: &mut String) {
    let frame = object
        .frame
        .unwrap_or(Rect::new(0.0, 0.0, 1_000_000.0, 500_000.0));
    let x = fnum(frame.x);
    let y = fnum(frame.y);
    let width = fnum(frame.width);
    let height = fnum(frame.height);
    out.push_str(&format!(
        "<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" fill=\"none\" stroke=\"#999999\" stroke-width=\"20000\" stroke-dasharray=\"60000,60000\"/>"
    ));
    let label = escape_xml(&object.label);
    let text_x = fnum(frame.x + TEXT_PADDING_EMU);
    let text_y = fnum(frame.y + TEXT_LINE_HEIGHT_EMU);
    out.push_str(&format!(
        "<text x=\"{text_x}\" y=\"{text_y}\" font-family=\"sans-serif\" font-size=\"150000\">{label}</text>"
    ));
}

/// Renders a table as a grid of `<rect>` cells, one per cell, plus per-cell
/// border lines and a `<text>` element for each non-empty cell.
///
/// Cells are laid out from the table frame's origin using `column_widths` and
/// each row's `height`. The first row is rendered bold with a distinct fill
/// when `header_row` is set. Cell-level fills and borders override the table
/// defaults.
fn render_table(table: &TableShape, theme: &Theme, out: &mut String) {
    let frame = table.transform.frame;
    let cx = frame.x + frame.width / 2.0;
    let cy = frame.y + frame.height / 2.0;

    out.push_str("<g");
    push_transform(out, table.transform.rotation, cx, cy);
    out.push('>');

    if table.rows.is_empty() {
        let x = fnum(frame.x);
        let y = fnum(frame.y);
        let width = fnum(frame.width);
        let height = fnum(frame.height);
        out.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" fill=\"none\" stroke=\"none\"/>"
        ));
        out.push_str("</g>");
        return;
    }

    // Left x of each column, accumulated from the frame origin.
    let mut col_x = Vec::with_capacity(table.column_widths.len() + 1);
    let mut acc = frame.x;
    for &w in &table.column_widths {
        col_x.push(acc);
        acc += w;
    }
    col_x.push(acc);

    // Top y of each row, accumulated from the frame origin.
    let mut row_y = Vec::with_capacity(table.rows.len() + 1);
    let mut acc = frame.y;
    for row in &table.rows {
        row_y.push(acc);
        acc += row.height;
    }
    row_y.push(acc);

    for (ri, row) in table.rows.iter().enumerate() {
        let is_header = table.header_row && ri == 0;
        let top = row_y[ri];
        let bottom = row_y[ri + 1];
        for (ci, cell) in row.cells.iter().enumerate() {
            let left = col_x[ci];
            let right = col_x[ci + 1];
            let cell_rect = Rect::new(left, top, right - left, bottom - top);
            render_cell_rect(cell, cell_rect, is_header, out);
            render_cell_borders(cell, &table.default_borders, cell_rect, out);
            render_cell_text(cell, cell_rect, is_header, theme, out);
        }
    }

    out.push_str("</g>");
}

/// Renders the background `<rect>` for a single cell.
fn render_cell_rect(cell: &TableCell, cell_rect: Rect, is_header: bool, out: &mut String) {
    let x = fnum(cell_rect.x);
    let y = fnum(cell_rect.y);
    let width = fnum(cell_rect.width);
    let height = fnum(cell_rect.height);
    let fill = cell_fill(cell, is_header);
    out.push_str(&format!(
        "<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" fill=\"{fill}\" stroke=\"none\"/>"
    ));
}

/// Returns the fill color string for a cell, honoring cell-level fill, then the
/// header default, then `none`.
fn cell_fill(cell: &TableCell, is_header: bool) -> String {
    if let Some(Fill::Solid(color)) = &cell.fill {
        return hex_color(color);
    }
    if is_header {
        return TABLE_HEADER_FILL.to_string();
    }
    "none".to_string()
}

/// Renders the four border edges of a cell as `<line>` elements. A cell with no
/// explicit `borders` inherits the table `default_borders`.
fn render_cell_borders(
    cell: &TableCell,
    default_borders: &TableBorders,
    cell_rect: Rect,
    out: &mut String,
) {
    let borders = cell.borders.as_ref().unwrap_or(default_borders);
    let left = cell_rect.x;
    let top = cell_rect.y;
    let right = cell_rect.x + cell_rect.width;
    let bottom = cell_rect.y + cell_rect.height;
    if let Some(edge) = &borders.top {
        push_border_line(out, edge, left, top, right, top);
    }
    if let Some(edge) = &borders.bottom {
        push_border_line(out, edge, left, bottom, right, bottom);
    }
    if let Some(edge) = &borders.left {
        push_border_line(out, edge, left, top, left, bottom);
    }
    if let Some(edge) = &borders.right {
        push_border_line(out, edge, right, top, right, bottom);
    }
}

/// Pushes a single border `<line>` with color, width, and dash style.
fn push_border_line(
    out: &mut String,
    edge: &slides_core::BorderEdge,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
) {
    let x1 = fnum(x1);
    let y1 = fnum(y1);
    let x2 = fnum(x2);
    let y2 = fnum(y2);
    let stroke = hex_color(&edge.color);
    let width = fnum(edge.width_emu);
    out.push_str(&format!(
        "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{stroke}\" stroke-width=\"{width}\""
    ));
    push_dash_array(out, &edge.dash);
    out.push_str("/>");
}

/// Renders the cell's text, horizontally aligned per [`CellAlign`] and vertically
/// centered. Empty cells emit no `<text>` element.
fn render_cell_text(
    cell: &TableCell,
    cell_rect: Rect,
    is_header: bool,
    theme: &Theme,
    out: &mut String,
) {
    if cell.text.is_empty() {
        return;
    }
    let (anchor, tx) = match cell.align {
        CellAlign::Left => ("start", cell_rect.x + TEXT_PADDING_EMU),
        CellAlign::Center => ("middle", cell_rect.x + cell_rect.width / 2.0),
        CellAlign::Right => ("end", cell_rect.x + cell_rect.width - TEXT_PADDING_EMU),
    };
    let ty = cell_rect.y + cell_rect.height / 2.0;
    let font_size = fnum(TEXT_FONT_SIZE_EMU);
    let font = escape_xml(&theme.body_font);
    let weight = if is_header { "bold" } else { "normal" };
    let tx = fnum(tx);
    let ty = fnum(ty);
    out.push_str(&format!(
        "<text x=\"{tx}\" y=\"{ty}\" text-anchor=\"{anchor}\" dominant-baseline=\"central\" font-family=\"{font}\" font-size=\"{font_size}\" font-weight=\"{weight}\">"
    ));
    out.push_str(&escape_xml(&cell.text));
    out.push_str("</text>");
}

#[cfg(test)]
mod tests {
    use super::*;
    use slides_core::{
        BorderEdge, CategorySeries, CellAlign, ChartData, ChartType, GeometricShape, HeadingLevel,
        ImageShape, MediaEntry, ParagraphStyle, Run, TableBorders, TableShape, TextBox, Transform,
    };

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, w, h)
    }

    fn render(slide: &slides_core::Slide) -> RenderedSlide {
        render_slide(
            slide,
            &Theme::default(),
            &MediaStore::new(),
            &RenderOptions::default(),
        )
    }

    #[test]
    fn text_box_renders_run_formatting_and_escapes_xml() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(100_000.0, 100_000.0, 4_000_000.0, 1_000_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![
                    Run {
                        text: "Bold & <tag>".to_string(),
                        bold: true,
                        italic: false,
                        underline: false,
                        ..Default::default()
                    },
                    Run {
                        text: "italic".to_string(),
                        bold: false,
                        italic: true,
                        underline: false,
                        ..Default::default()
                    },
                    Run {
                        text: "under".to_string(),
                        bold: false,
                        italic: false,
                        underline: true,
                        ..Default::default()
                    },
                ],
                list_style: ListStyle::None,
                ..Default::default()
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("font-weight=\"bold\""));
        assert!(out.svg.contains("font-style=\"italic\""));
        assert!(out.svg.contains("text-decoration=\"underline\""));
        assert!(out.svg.contains("Bold &amp; &lt;tag&gt;"));
        assert!(!out.svg.contains("<tag>"));
    }

    #[test]
    fn rectangle_renders_fill() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Geometric(GeometricShape {
            transform: Transform {
                frame: rect(100_000.0, 100_000.0, 2_000_000.0, 1_000_000.0),
                rotation: 0.0,
            },
            geometry: Geometry::Rectangle,
            style: Style {
                fill: Some(Fill::Solid(Color::rgb(255, 0, 0))),
                outline: None,
                shadow: None,
            },
        }));

        let out = render(&slide);
        assert!(out.svg.contains("<rect "));
        assert!(out.svg.contains("fill=\"#ff0000\""));
    }

    #[test]
    fn ellipse_renders() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Geometric(GeometricShape {
            transform: Transform {
                frame: rect(0.0, 0.0, 1_000_000.0, 1_000_000.0),
                rotation: 0.0,
            },
            geometry: Geometry::Ellipse,
            style: Style::default(),
        }));

        let out = render(&slide);
        assert!(out.svg.contains("<ellipse "));
    }

    #[test]
    fn image_renders_data_uri_and_missing_placeholder() {
        let mut media = MediaStore::new();
        media.insert(
            "present",
            MediaEntry {
                mime: "image/png".to_string(),
                bytes: vec![0x89, 0x50, 0x4e, 0x47],
                width: 10,
                height: 10,
            },
        );

        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Image(ImageShape {
            transform: Transform {
                frame: rect(0.0, 0.0, 1_000_000.0, 1_000_000.0),
                rotation: 0.0,
            },
            media_ref: "present".to_string(),
            crop: None,
        }));
        slide.shapes.push(Shape::Image(ImageShape {
            transform: Transform {
                frame: rect(0.0, 0.0, 1_000_000.0, 1_000_000.0),
                rotation: 0.0,
            },
            media_ref: "absent".to_string(),
            crop: None,
        }));

        let out = render_slide(&slide, &Theme::default(), &media, &RenderOptions::default());
        assert!(out.svg.contains("data:image/png;base64,"));
        assert!(out.svg.contains("Missing image: absent"));
        assert!(out.svg.contains("fill=\"#cccccc\""));
    }

    #[test]
    fn chart_renders_positioned_and_deterministic() {
        let chart = ChartShape::new(
            Transform {
                frame: rect(500_000.0, 500_000.0, 6_000_000.0, 3_000_000.0),
                rotation: 0.0,
            },
            ChartType::Column,
            ChartData::Category {
                categories: vec!["Q1".to_string(), "Q2".to_string(), "Q3".to_string()],
                series: vec![CategorySeries {
                    name: "Sales".to_string(),
                    values: vec![10.0, 20.0, 30.0],
                }],
            },
            Some("Quarterly Sales".to_string()),
        )
        .expect("valid chart");

        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Chart(chart));

        let out = render(&slide);

        // The chart is embedded as a nested `<svg>` positioned at its frame,
        // not stretched across the whole slide viewport.
        assert!(
            out.svg
                .contains("<svg x=\"500000\" y=\"500000\" width=\"6000000\" height=\"3000000\""),
            "chart must be positioned at its frame via a nested svg"
        );
        // Column charts render bars as rectangles.
        assert!(out.svg.contains("<rect"), "column chart should render bars");
        // Title text is rendered.
        assert!(out.svg.contains("Quarterly Sales"));
        // Deterministic: same input yields identical svg and hash.
        let again = render(&slide);
        assert_eq!(out.svg, again.svg);
        assert_eq!(out.hash, again.hash);
    }

    #[test]
    fn passthrough_renders_labelled_placeholder() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Passthrough(PassthroughObject {
            id: "obj-1".to_string(),
            label: "Chart X".to_string(),
            source_part: "ppt/slides/slide1.xml".to_string(),
            raw_bytes: Vec::new(),
            frame: Some(rect(0.0, 0.0, 1_000_000.0, 500_000.0)),
        }));

        let out = render(&slide);
        assert!(out.svg.contains("stroke-dasharray=\"60000,60000\""));
        assert!(out.svg.contains("Chart X"));
    }

    #[test]
    fn rendering_is_deterministic() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("hello")],
                list_style: ListStyle::Unordered,
                ..Default::default()
            }],
        }));
        slide.shapes.push(Shape::Geometric(GeometricShape {
            transform: Transform {
                frame: rect(0.0, 0.0, 500_000.0, 500_000.0),
                rotation: 0.0,
            },
            geometry: Geometry::Star5,
            style: Style {
                fill: Some(Fill::Solid(Color::rgb(0, 0, 0))),
                outline: None,
                shadow: None,
            },
        }));

        let first = render(&slide);
        let second = render(&slide);
        assert_eq!(first.svg, second.svg);
        assert_eq!(first.hash, second.hash);
    }

    #[test]
    fn theme_background_is_painted_first() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Geometric(GeometricShape {
            transform: Transform {
                frame: rect(100_000.0, 100_000.0, 500_000.0, 500_000.0),
                rotation: 0.0,
            },
            geometry: Geometry::Rectangle,
            style: Style {
                fill: Some(Fill::Solid(Color::rgb(255, 0, 0))),
                outline: None,
                shadow: None,
            },
        }));

        let out = render(&slide);
        let bg = out.svg.find("<rect width=").expect("background rect");
        let shape = out.svg.find("<rect x=").expect("shape rect");
        assert!(bg < shape);
    }

    #[test]
    fn rotation_emits_transform() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Geometric(GeometricShape {
            transform: Transform {
                frame: rect(0.0, 0.0, 1_000_000.0, 1_000_000.0),
                rotation: 45.0,
            },
            geometry: Geometry::Rectangle,
            style: Style::default(),
        }));

        let out = render(&slide);
        assert!(out.svg.contains("transform=\"rotate(45,"));
    }

    #[test]
    fn shadow_emits_filter_def_and_reference() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Geometric(GeometricShape {
            transform: Transform {
                frame: rect(0.0, 0.0, 1_000_000.0, 1_000_000.0),
                rotation: 0.0,
            },
            geometry: Geometry::Rectangle,
            style: Style {
                fill: Some(Fill::Solid(Color::rgb(0, 0, 0))),
                outline: None,
                shadow: Some(Shadow {
                    offset_x: 50_000.0,
                    offset_y: 50_000.0,
                    blur: 80_000.0,
                    color: Color::rgb(0, 0, 0),
                    opacity: 0.5,
                }),
            },
        }));

        let out = render(&slide);
        assert!(out.svg.contains("<filter id=\"sh0\""));
        assert!(out.svg.contains("<feDropShadow"));
        assert!(out.svg.contains("filter=\"url(#sh0)\""));
    }

    #[test]
    fn default_options_are_pptx_16by9() {
        let opts = RenderOptions::default();
        assert_eq!(opts.width_emu, 12_192_000.0);
        assert_eq!(opts.height_emu, 6_858_000.0);
    }

    #[test]
    fn strikethrough_renders() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run {
                    text: "deleted".to_string(),
                    strikethrough: true,
                    ..Default::default()
                }],
                list_style: ListStyle::None,
                ..Default::default()
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("text-decoration=\"line-through\""));
    }

    #[test]
    fn underline_and_strikethrough_combine() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run {
                    text: "both".to_string(),
                    underline: true,
                    strikethrough: true,
                    ..Default::default()
                }],
                list_style: ListStyle::None,
                ..Default::default()
            }],
        }));

        let out = render(&slide);
        assert!(out
            .svg
            .contains("text-decoration=\"underline line-through\""));
    }

    #[test]
    fn superscript_renders() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("x").superscript()],
                list_style: ListStyle::None,
                ..Default::default()
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("baseline-shift=\"super\""));
        assert!(out.svg.contains("font-size=\"0.7em\""));
    }

    #[test]
    fn subscript_renders() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("2").subscript()],
                list_style: ListStyle::None,
                ..Default::default()
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("baseline-shift=\"sub\""));
        assert!(out.svg.contains("font-size=\"0.7em\""));
    }

    #[test]
    fn link_renders_anchor() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("click").link("#slide-2").unwrap()],
                list_style: ListStyle::None,
                ..Default::default()
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("<a href=\"#slide-2\">"));
        assert!(out.svg.contains("click"));
        assert!(out.svg.contains("</a>"));
    }

    #[test]
    fn link_url_is_escaped() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("params")
                    .link("mailto:a@b.com?subject=1&2")
                    .unwrap()],
                list_style: ListStyle::None,
                ..Default::default()
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("href=\"mailto:a@b.com?subject=1&amp;2\""));
        assert!(!out.svg.contains("subject=1&2\""));
    }

    #[test]
    fn inline_code_renders_monospace() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("code").code()],
                list_style: ListStyle::None,
                ..Default::default()
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("font-family=\"Courier New, monospace\""));
    }

    #[test]
    fn inline_code_uses_run_font_family() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("code").font("Fira Code").code()],
                list_style: ListStyle::None,
                ..Default::default()
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("font-family=\"Fira Code\""));
    }

    #[test]
    fn heading_renders_larger_bold_font() {
        let mut slide = slides_core::Slide::default();
        let style = ParagraphStyle {
            heading: Some(HeadingLevel::H1),
            ..Default::default()
        };
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("Title")],
                list_style: ListStyle::None,
                style,
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("font-weight=\"bold\""));
        assert!(out.svg.contains("font-size=\"457200\""));
        assert!(out.svg.contains("font-family=\"Calibri\""));
    }

    #[test]
    fn blockquote_renders_border_and_italic() {
        let mut slide = slides_core::Slide::default();
        let style = ParagraphStyle {
            blockquote: true,
            ..Default::default()
        };
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("quote")],
                list_style: ListStyle::None,
                style,
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("font-style=\"italic\""));
        assert!(out.svg.contains("fill=\"#cccccc\""));
        assert!(out.svg.contains("<rect"));
    }

    #[test]
    fn code_block_renders_background_and_monospace() {
        let mut slide = slides_core::Slide::default();
        let style = ParagraphStyle {
            code_block: true,
            ..Default::default()
        };
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("fn main() {}")],
                list_style: ListStyle::Unordered,
                style,
            }],
        }));

        let out = render(&slide);
        assert!(out.svg.contains("font-family=\"Courier New, monospace\""));
        assert!(out.svg.contains("fill=\"#f5f5f5\""));
        assert!(!out.svg.contains("\u{2022}"));
    }

    #[test]
    fn indent_level_shifts_paragraph() {
        let mut slide = slides_core::Slide::default();
        let style = ParagraphStyle {
            indent_level: 2,
            ..Default::default()
        };
        slide.shapes.push(Shape::TextBox(TextBox {
            frame: rect(0.0, 0.0, 1_000_000.0, 500_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new("indented")],
                list_style: ListStyle::None,
                style,
            }],
        }));

        let out = render(&slide);
        // x = TEXT_PADDING_EMU + 2 * INDENT_EMU = 91440 + 720000 = 811440
        assert!(out.svg.contains("x=\"811440\""));
    }

    /// Builds a 2x2 table at the origin filling a 2,000,000 x 1,000,000 frame.
    fn sample_table() -> TableShape {
        let mut table = TableShape::default_grid(2, 2, rect(0.0, 0.0, 2_000_000.0, 1_000_000.0));
        table.cell_mut(0, 0).unwrap().text = "A".to_string();
        table.cell_mut(0, 1).unwrap().text = "B".to_string();
        table.cell_mut(1, 0).unwrap().text = "C".to_string();
        table.cell_mut(1, 1).unwrap().text = "D".to_string();
        table
    }

    #[test]
    fn table_renders_rect_per_cell_and_text() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(sample_table()));

        let out = render(&slide);
        // One background rect + one rect per cell (4).
        assert_eq!(out.svg.matches("<rect ").count(), 5);
        assert!(out.svg.contains(">A<"));
        assert!(out.svg.contains(">B<"));
        assert!(out.svg.contains(">C<"));
        assert!(out.svg.contains(">D<"));
    }

    #[test]
    fn table_header_row_is_bold_and_filled() {
        let mut table = sample_table();
        table.header_row = true;
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(table));

        let out = render(&slide);
        // Header fill appears on the first row's cells.
        assert!(out.svg.contains(TABLE_HEADER_FILL));
        // Header text is bold; body text is normal. At least one bold weight.
        assert!(out.svg.contains("font-weight=\"bold\""));
        // Both header cells (A, B) share the same bold opening text element.
        assert!(out.svg.contains("font-weight=\"bold\">A<"));
        assert!(out.svg.contains("font-weight=\"bold\">B<"));
        // Non-header cells render normal weight.
        assert!(out.svg.contains("font-weight=\"normal\">C<"));
    }

    #[test]
    fn table_cell_fill_overrides_header_default() {
        let mut table = sample_table();
        table.header_row = true;
        table.cell_mut(0, 0).unwrap().fill = Some(Fill::Solid(Color::rgb(255, 0, 0)));
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(table));

        let out = render(&slide);
        // The red cell fill appears before the default header fill in document order.
        let red = out.svg.find("#ff0000").expect("red fill");
        let header = out
            .svg
            .find(TABLE_HEADER_FILL)
            .expect("header default fill");
        assert!(red < header);
    }

    #[test]
    fn table_alignment_left_center_right() {
        let mut table = sample_table();
        table.cell_mut(0, 0).unwrap().align = CellAlign::Left;
        table.cell_mut(0, 1).unwrap().align = CellAlign::Center;
        table.cell_mut(1, 0).unwrap().align = CellAlign::Right;
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(table));

        let out = render(&slide);
        assert!(out.svg.contains("text-anchor=\"start\""));
        assert!(out.svg.contains("text-anchor=\"middle\""));
        assert!(out.svg.contains("text-anchor=\"end\""));
    }

    #[test]
    fn table_text_is_vertically_centered() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(sample_table()));

        let out = render(&slide);
        assert!(out.svg.contains("dominant-baseline=\"central\""));
    }

    #[test]
    fn table_default_borders_render_as_lines() {
        let mut table = sample_table();
        let edge = BorderEdge {
            color: Color::rgb(17, 17, 17),
            width_emu: 9_525.0,
            dash: DashStyle::Solid,
        };
        table.default_borders = TableBorders {
            top: Some(edge.clone()),
            bottom: Some(edge.clone()),
            left: Some(edge.clone()),
            right: Some(edge),
        };
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(table));

        let out = render(&slide);
        // 4 cells x 4 edges = 16 border lines.
        assert_eq!(out.svg.matches("<line ").count(), 16);
        assert!(out.svg.contains("stroke=\"#111111\""));
        assert!(out.svg.contains("stroke-width=\"9525\""));
    }

    #[test]
    fn table_cell_border_override_replaces_default() {
        let mut table = sample_table();
        // Table default: no borders anywhere.
        // One cell overrides with a single top edge.
        table.cell_mut(1, 1).unwrap().borders = Some(TableBorders {
            top: Some(BorderEdge {
                color: Color::rgb(0, 128, 0),
                width_emu: 9_525.0,
                dash: DashStyle::Dash,
            }),
            ..Default::default()
        });
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(table));

        let out = render(&slide);
        // Only the overridden cell's single top edge is drawn.
        assert_eq!(out.svg.matches("<line ").count(), 1);
        assert!(out.svg.contains("stroke=\"#008000\""));
        assert!(out.svg.contains("stroke-dasharray=\"300000,150000\""));
    }

    #[test]
    fn table_empty_cell_emits_no_text() {
        let mut table = TableShape::default_grid(1, 2, rect(0.0, 0.0, 2_000_000.0, 500_000.0));
        table.cell_mut(0, 0).unwrap().text = "only".to_string();
        // (0,1) left empty.
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(table));

        let out = render(&slide);
        // Two cell rects, exactly one text element.
        assert_eq!(out.svg.matches("<rect ").count(), 3); // bg + 2 cells
        assert_eq!(out.svg.matches("<text").count(), 1);
        assert!(out.svg.contains(">only<"));
    }

    #[test]
    fn table_text_is_xml_escaped() {
        let mut table = TableShape::default_grid(1, 1, rect(0.0, 0.0, 1_000_000.0, 500_000.0));
        table.cell_mut(0, 0).unwrap().text = "<b> & \"x\"".to_string();
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(table));

        let out = render(&slide);
        assert!(out.svg.contains("&lt;b&gt; &amp; &quot;x&quot;"));
        assert!(!out.svg.contains("<b>"));
    }

    #[test]
    fn table_rotation_emits_transform_on_group() {
        let mut table = sample_table();
        table.transform.rotation = 30.0;
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(table));

        let out = render(&slide);
        // The group opens with the rotate transform first.
        assert!(out.svg.contains("<g transform=\"rotate(30,"));
    }

    #[test]
    fn table_renders_before_following_shapes() {
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(sample_table()));
        slide.shapes.push(Shape::Geometric(GeometricShape {
            transform: Transform {
                frame: rect(0.0, 0.0, 500_000.0, 500_000.0),
                rotation: 0.0,
            },
            geometry: Geometry::Rectangle,
            style: Style {
                fill: Some(Fill::Solid(Color::rgb(0, 0, 0))),
                outline: None,
                shadow: None,
            },
        }));

        let out = render(&slide);
        let table_group = out.svg.find("<g>").expect("table group");
        let geo = out.svg.find("<rect x=").expect("geometric rect");
        // The table's first cell rect (`<rect x=`) comes before the geometric shape.
        let first_cell = out.svg.find("<rect x=").expect("cell rect");
        assert!(table_group < first_cell);
        assert!(first_cell < geo || table_group < geo);
    }

    #[test]
    fn table_rendering_is_deterministic() {
        let mut slide = slides_core::Slide::default();
        let mut table = sample_table();
        table.header_row = true;
        let edge = BorderEdge {
            color: Color::rgb(0, 0, 0),
            width_emu: 9_525.0,
            dash: DashStyle::Solid,
        };
        table.default_borders = TableBorders {
            top: Some(edge.clone()),
            bottom: Some(edge.clone()),
            left: Some(edge.clone()),
            right: Some(edge),
        };
        slide.shapes.push(Shape::Table(table));

        let first = render(&slide);
        let second = render(&slide);
        assert_eq!(first.svg, second.svg);
        assert_eq!(first.hash, second.hash);
    }

    #[test]
    fn empty_table_renders_frame_rect_without_panicking() {
        let table = TableShape {
            transform: Transform {
                frame: rect(0.0, 0.0, 2_000_000.0, 1_000_000.0),
                rotation: 0.0,
            },
            rows: Vec::new(),
            column_widths: Vec::new(),
            default_borders: TableBorders::default(),
            header_row: false,
        };
        let mut slide = slides_core::Slide::default();
        slide.shapes.push(Shape::Table(table));

        let out = render(&slide);
        // Exactly one cell rect (the frame) plus the background rect.
        assert_eq!(out.svg.matches("<rect ").count(), 2);
        assert!(out.svg.contains("width=\"2000000\""));
        assert!(out.svg.contains("height=\"1000000\""));
    }
}
