//! Deterministic export of slides to standalone SVG, PNG, and multi-page PDF.
//!
//! - [`export_slide_svg`] is a thin wrapper around
//!   [`slides_render::render_slide`], returning the standalone SVG document.
//! - [`export_slide_png`] rasterizes that SVG to RGBA via `resvg`/`usvg`/
//!   `tiny-skia` and encodes the pixels as PNG.
//! - [`export_deck_pdf`] rasterizes each slide and embeds it as a full-page
//!   image in a `printpdf` document, one page per slide.
//!
//! Every export is deterministic: identical inputs yield byte-identical output.
//! PNG bytes depend only on the SVG (resvg, usvg and tiny-skia are pure
//! functions of their input, and the [`image`] encoder writes no timestamps).
//! PDF output depends neither on the wall clock nor on a random generator: the
//! metadata dates are pinned to the Unix epoch and the trailer `/ID`, which
//! `printpdf` fills with random strings, is rewritten to a fixed value via the
//! `lopdf` library that `printpdf` is built on. No telemetry, no network.

use std::io::Cursor;

use printpdf::lopdf::{Document as LopdfDocument, Object, StringFormat};
use printpdf::{
    ColorBits, ColorSpace, Image, ImageTransform, ImageXObject, Mm, OffsetDateTime, PdfDocument, Px,
};
use slides_core::{Deck, MediaStore, Slide, Theme};
use slides_render::{render_slide, RenderOptions};

/// Boxed dynamic error, the crate's single error type.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// EMU per CSS pixel at 96 DPI (1 in = 96 px = 914400 EMU).
const EMU_PER_PX: f64 = 9525.0;
/// EMU per PostScript point (1 in = 72 pt = 914400 EMU).
const EMU_PER_PT: f64 = 12700.0;
/// EMU per millimetre (1 in = 25.4 mm = 914400 EMU).
const EMU_PER_MM: f64 = 36000.0;
/// Rasterization scale used for the PDF background image (retina).
const PDF_RASTER_SCALE: f64 = 2.0;
/// DPI assumed by `printpdf`'s image transform (its default).
const PDF_DPI: f32 = 300.0;
/// Fixed value written into the PDF trailer `/ID` array for determinism.
const FIXED_PDF_ID: &[u8] = b"900slides-export";

/// Exports a single slide as a standalone SVG document.
///
/// Deterministic: the same inputs always produce the same SVG string.
pub fn export_slide_svg(
    slide: &Slide,
    theme: &Theme,
    media: &MediaStore,
    opts: &RenderOptions,
) -> String {
    render_slide(slide, theme, media, opts).svg
}

/// Exports a single slide as a PNG image at the given scale.
///
/// `scale = 1.0` renders at native EMU resolution (96 DPI); `2.0` is retina.
/// Deterministic: the same inputs always produce identical PNG bytes.
pub fn export_slide_png(
    slide: &Slide,
    theme: &Theme,
    media: &MediaStore,
    opts: &RenderOptions,
    scale: f64,
) -> Result<Vec<u8>> {
    let svg = render_slide(slide, theme, media, opts).svg;
    let (width, height, rgba) = rasterize_straight_rgba(&svg, opts, scale)?;
    let buffer = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or("PNG export: RGBA buffer length does not match the slide dimensions")?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(buffer)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

/// Exports an entire deck as a multi-page PDF.
///
/// Each slide is rasterized at retina resolution and placed as a full-page
/// image on a page sized to the slide dimensions. One page is produced per
/// slide. Deterministic: the same deck always produces identical PDF bytes.
pub fn export_deck_pdf(deck: &Deck, opts: &RenderOptions) -> Result<Vec<u8>> {
    let width_mm = Mm((opts.width_emu / EMU_PER_MM) as f32);
    let height_mm = Mm((opts.height_emu / EMU_PER_MM) as f32);

    let (doc, first_page, first_layer) = PdfDocument::new("Slides", width_mm, height_mm, "Layer 1");
    // Pin every metadata date to the Unix epoch so output never depends on the
    // current time. (printpdf defaults all three to "now".)
    let epoch = OffsetDateTime::UNIX_EPOCH;
    let doc = doc
        .with_creation_date(epoch)
        .with_mod_date(epoch)
        .with_metadata_date(epoch);

    for (index, slide) in deck.slides.iter().enumerate() {
        let (page_index, layer_index) = if index == 0 {
            (first_page, first_layer)
        } else {
            doc.add_page(width_mm, height_mm, "Layer 1")
        };
        let layer = doc.get_page(page_index).get_layer(layer_index);

        let svg = render_slide(slide, &deck.theme, &deck.media, opts).svg;
        let (px_w, px_h, rgba) = rasterize_straight_rgba(&svg, opts, PDF_RASTER_SCALE)?;

        let image = Image::from(ImageXObject {
            width: Px(px_w as usize),
            height: Px(px_h as usize),
            color_space: ColorSpace::Rgb,
            bits_per_component: ColorBits::Bit8,
            interpolate: true,
            image_data: flatten_over_white(&rgba),
            image_filter: None,
            smask: None,
            clipping_bbox: None,
        });

        // Scale the image so its physical size matches the page exactly. The
        // page and the raster share the slide's aspect ratio, so the two scale
        // factors are equal; both are computed for robustness.
        let natural_w_pt = px_w as f32 * 72.0 / PDF_DPI;
        let natural_h_pt = px_h as f32 * 72.0 / PDF_DPI;
        let target_w_pt = (opts.width_emu / EMU_PER_PT) as f32;
        let target_h_pt = (opts.height_emu / EMU_PER_PT) as f32;
        let transform = ImageTransform {
            translate_x: Some(Mm(0.0)),
            translate_y: Some(Mm(0.0)),
            rotate: None,
            scale_x: Some(target_w_pt / natural_w_pt),
            scale_y: Some(target_h_pt / natural_h_pt),
            dpi: Some(PDF_DPI),
        };
        image.add_to_layer(layer, transform);
    }

    let raw = doc.save_to_bytes()?;
    pin_pdf_id(&raw)
}

/// Rasterizes an SVG string to straight (non-premultiplied) RGBA pixels at the
/// given scale, returning `(width_px, height_px, rgba)`.
///
/// `usvg`'s default options use an empty font database, so the result never
/// depends on which fonts are installed on the host: it is a pure function of
/// the SVG input.
fn rasterize_straight_rgba(
    svg: &str,
    opts: &RenderOptions,
    scale: f64,
) -> Result<(u32, u32, Vec<u8>)> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())?;
    let width = emu_to_px(opts.width_emu, scale);
    let height = emu_to_px(opts.height_emu, scale);
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| format!("failed to allocate {width}x{height} pixmap"))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    Ok((width, height, unpremultiply(&pixmap.take())))
}

/// Converts EMU to raster pixels at the given scale, floored to a minimum of 1.
fn emu_to_px(emu: f64, scale: f64) -> u32 {
    ((emu / EMU_PER_PX) * scale).round().max(1.0) as u32
}

/// Converts tiny-skia's premultiplied RGBA into straight RGBA expected by the
/// [`image`] crate and by alpha compositing.
fn unpremultiply(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len());
    for pixel in rgba.chunks_exact(4) {
        let (r, g, b, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let af = a as u32;
            let unpre = |c: u8| (((c as u32 * 255 + af / 2) / af).min(255)) as u8;
            out.extend_from_slice(&[unpre(r), unpre(g), unpre(b), a]);
        }
    }
    out
}

/// Composites straight RGBA pixels over an opaque white background, returning
/// opaque RGB bytes suitable for a PDF `DeviceRGB` image.
fn flatten_over_white(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for pixel in rgba.chunks_exact(4) {
        let af = pixel[3] as f32 / 255.0;
        let blend = |c: u8| (c as f32 * af + 255.0 * (1.0 - af)).round() as u8;
        rgb.extend_from_slice(&[blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]);
    }
    rgb
}

/// Rewrites the trailer `/ID` array of a serialized PDF to a fixed value.
///
/// `printpdf` fills `/ID` with a random per-document id and a fresh random id
/// on every save, which would make the output non-deterministic. We reload the
/// finished bytes with `lopdf`, overwrite `/ID`, and re-serialize. `lopdf`
/// stores objects in a `BTreeMap`, so its output is a deterministic function
/// of the object graph.
fn pin_pdf_id(raw: &[u8]) -> Result<Vec<u8>> {
    let mut pdf = LopdfDocument::load_from(Cursor::new(raw))?;
    let fixed = Object::String(FIXED_PDF_ID.to_vec(), StringFormat::Literal);
    pdf.trailer
        .set("ID", Object::Array(vec![fixed.clone(), fixed]));
    let mut out = Vec::with_capacity(raw.len());
    pdf.save_to(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slides_core::{
        Color, Fill, GeometricShape, Geometry, Paragraph, Rect, Run, Shape, Style, TextBox,
        Transform,
    };

    /// Builds a slide containing a text box and a rectangle.
    fn make_slide(text: &str, index: usize) -> Slide {
        let mut slide = Slide::default();
        slide.shapes.push(Shape::TextBox(TextBox {
            id: String::new(),
            frame: Rect::new(100_000.0, 100_000.0, 8_000_000.0, 1_000_000.0),
            paragraphs: vec![Paragraph {
                runs: vec![Run::new(format!("Slide {index}: {text}"))],
                ..Default::default()
            }],
        }));
        slide.shapes.push(Shape::Geometric(GeometricShape {
            id: String::new(),
            transform: Transform {
                frame: Rect::new(1_000_000.0, 2_000_000.0, 2_000_000.0, 2_000_000.0),
                rotation: 0.0,
            },
            geometry: Geometry::Rectangle,
            style: Style {
                fill: Some(Fill::Solid(Color::rgb(50, 100, 200))),
                outline: None,
                shadow: None,
            },
        }));
        slide
    }

    /// Builds a deck with the requested number of slides.
    fn make_deck(n_slides: usize) -> Deck {
        let mut deck = Deck {
            id: "test-deck".to_string(),
            ..Default::default()
        };
        for i in 0..n_slides {
            deck.slides.push(make_slide("content", i));
        }
        deck
    }

    #[test]
    fn export_slide_svg_produces_valid_svg() {
        let slide = make_slide("Hello SVG", 0);
        let svg = export_slide_svg(
            &slide,
            &Theme::default(),
            &MediaStore::new(),
            &RenderOptions::default(),
        );
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("Hello SVG"));
    }

    #[test]
    fn export_slide_png_produces_valid_png() {
        let slide = make_slide("PNG text", 0);
        let png = export_slide_png(
            &slide,
            &Theme::default(),
            &MediaStore::new(),
            &RenderOptions::default(),
            1.0,
        )
        .expect("png export should succeed");
        const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(&png[..8], &PNG_MAGIC);
    }

    #[test]
    fn export_slide_png_deterministic() {
        let slide = make_slide("Same", 0);
        let opts = RenderOptions::default();
        let theme = Theme::default();
        let media = MediaStore::new();
        let first = export_slide_png(&slide, &theme, &media, &opts, 1.5).unwrap();
        let second = export_slide_png(&slide, &theme, &media, &opts, 1.5).unwrap();
        assert_eq!(first, second, "identical input must yield identical PNG");
    }

    #[test]
    fn export_deck_pdf_produces_valid_pdf() {
        let deck = make_deck(3);
        let pdf = export_deck_pdf(&deck, &RenderOptions::default()).expect("pdf export");
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn export_deck_pdf_has_multiple_pages() {
        let opts = RenderOptions::default();
        let single = export_deck_pdf(&make_deck(1), &opts).unwrap();
        let triple = export_deck_pdf(&make_deck(3), &opts).unwrap();
        assert!(
            triple.len() > single.len(),
            "a 3-slide deck should be larger than a 1-slide deck"
        );

        // Precise check: parse the PDF and count page objects.
        let parsed = printpdf::lopdf::Document::load_from(Cursor::new(&triple[..])).unwrap();
        assert_eq!(
            parsed.get_pages().len(),
            3,
            "a 3-slide deck must produce a 3-page PDF"
        );
    }

    #[test]
    fn export_deck_pdf_deterministic() {
        let deck = make_deck(3);
        let opts = RenderOptions::default();
        let first = export_deck_pdf(&deck, &opts).unwrap();
        let second = export_deck_pdf(&deck, &opts).unwrap();
        assert_eq!(first, second, "identical input must yield identical PDF");
    }
}
