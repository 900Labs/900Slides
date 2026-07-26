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
    Color, DashStyle, Fill, GeometricShape, Geometry, ImageShape, ListStyle, MediaStore, Outline,
    Paragraph, PassthroughObject, Rect, Run, Shadow, Shape, Style, TextBox, Theme,
};

/// Horizontal padding inside a text box, in EMU (0.1 inch).
const TEXT_PADDING_EMU: f64 = 91_440.0;
/// Body font size, in EMU (18 pt = 18 * 12700).
const TEXT_FONT_SIZE_EMU: f64 = 228_600.0;
/// Constant estimated line height between paragraphs, in EMU (~0.4 inch).
const TEXT_LINE_HEIGHT_EMU: f64 = 360_000.0;

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
    match outline.dash {
        DashStyle::Solid => {}
        DashStyle::Dash => out.push_str(" stroke-dasharray=\"300000,150000\""),
        DashStyle::Dot => out.push_str(" stroke-dasharray=\"60000,60000\""),
        DashStyle::DashDot => out.push_str(" stroke-dasharray=\"300000,150000,60000,150000\""),
    }
}

/// Renders a text box as a `<g>` containing one `<text>` per paragraph.
fn render_text_box(text_box: &TextBox, theme: &Theme, out: &mut String) {
    out.push_str("<g>");
    let font = escape_xml(&theme.body_font);
    let font_size = fnum(TEXT_FONT_SIZE_EMU);
    for (index, paragraph) in text_box.paragraphs.iter().enumerate() {
        let x = fnum(text_box.frame.x + TEXT_PADDING_EMU);
        let y = fnum(text_box.frame.y + TEXT_LINE_HEIGHT_EMU * (index as f64 + 1.0));
        out.push_str(&format!(
            "<text x=\"{x}\" y=\"{y}\" font-family=\"{font}\" font-size=\"{font_size}\">"
        ));
        push_list_marker(paragraph, index, out);
        for run in &paragraph.runs {
            push_run(run, out);
        }
        out.push_str("</text>");
    }
    out.push_str("</g>");
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

/// Pushes a run as a `<tspan>` with run-level bold/italic/underline.
fn push_run(run: &Run, out: &mut String) {
    out.push_str("<tspan");
    if run.bold {
        out.push_str(" font-weight=\"bold\"");
    }
    if run.italic {
        out.push_str(" font-style=\"italic\"");
    }
    if run.underline {
        out.push_str(" text-decoration=\"underline\"");
    }
    out.push('>');
    out.push_str(&escape_xml(&run.text));
    out.push_str("</tspan>");
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

#[cfg(test)]
mod tests {
    use super::*;
    use slides_core::{GeometricShape, ImageShape, MediaEntry, Run, TextBox, Transform};

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
}
