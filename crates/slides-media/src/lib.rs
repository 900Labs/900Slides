//! Image ingest, EXIF strip, MIME allowlist.
//!
//! This crate is the untrusted-input boundary for images entering a deck. It
//! sniffs the MIME type from leading bytes (never file extensions), enforces a
//! size cap and a maximum dimension, strips EXIF/metadata from raster formats
//! by default, keeps only the first frame of a GIF or animated WebP, and
//! sanitizes SVG against scripts, event handlers, and external references
//! (`PRODUCT_SPEC.md` §7.4).
//!
//! Ingest is deterministic: the same input bytes plus options produce
//! identical output bytes across runs (`PRODUCT_SPEC.md` §6.5). The crate
//! performs no network calls and holds no global state.

use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::Reader;

/// Options controlling image ingest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestOptions {
    /// When `true`, raster bytes are returned verbatim (still subject to the
    /// allowlist and size/dimension caps). When `false`, EXIF and other
    /// metadata are stripped by re-encoding.
    pub preserve_exif: bool,
    /// Maximum accepted input size, in bytes. Oversized input is rejected
    /// rather than silently downscaled.
    pub max_bytes: usize,
    /// Maximum accepted native pixel dimension (applies to both width and
    /// height). Larger images are rejected.
    pub max_dim: u32,
}

impl Default for IngestOptions {
    /// Defaults to stripping EXIF, a 25 MiB size cap, and an 8192px dimension
    /// cap.
    fn default() -> Self {
        Self {
            preserve_exif: false,
            max_bytes: 25 * 1024 * 1024,
            max_dim: 8192,
        }
    }
}

/// The decoded image format, restricted to the allowlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// GIF (first frame only).
    Gif,
    /// WebP (first frame only).
    Webp,
    /// Sanitized SVG.
    Svg,
}

impl ImageFormat {
    /// Returns the canonical MIME type for this format.
    pub fn mime(self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Svg => "image/svg+xml",
        }
    }
}

/// An image that passed ingest, ready to store in a deck's media store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestedImage {
    /// Sanitized or re-encoded output bytes.
    pub bytes: Vec<u8>,
    /// Canonical MIME type.
    pub mime: &'static str,
    /// Decoded format.
    pub format: ImageFormat,
    /// Native pixel width.
    pub width: u32,
    /// Native pixel height.
    pub height: u32,
}

/// Errors returned by [`ingest`].
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    /// The input is not one of the allowlisted image formats.
    #[error("unsupported MIME type")]
    UnsupportedMime,
    /// The input exceeds the size limit.
    #[error("input exceeds the size limit ({max} bytes)")]
    TooLarge {
        /// The configured maximum size, in bytes.
        max: usize,
    },
    /// The decoded image exceeds the maximum dimension.
    #[error("image exceeds the maximum dimension ({max}px)")]
    TooLargeDim {
        /// The configured maximum dimension, in pixels.
        max: u32,
    },
    /// The image bytes are corrupt or unreadable.
    #[error("corrupt or unreadable image data: {0}")]
    Corrupt(String),
    /// The SVG contains an unsafe construct.
    #[error("unsafe SVG: {0}")]
    UnsafeSvg(String),
}

/// Shorthand for a [`Result`] with this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Ingests raw image bytes, validating and sanitizing them.
///
/// Sniffs the MIME type from the leading bytes, enforces the size and
/// dimension caps in `opts`, strips EXIF metadata from raster formats unless
/// `opts.preserve_exif` is set, keeps only the first frame of multi-frame
/// formats, and sanitizes SVG. The same input and options always produce
/// identical output bytes.
pub fn ingest(raw: &[u8], opts: &IngestOptions) -> Result<IngestedImage> {
    if raw.len() > opts.max_bytes {
        return Err(Error::TooLarge {
            max: opts.max_bytes,
        });
    }

    match sniff_format(raw)? {
        ImageFormat::Svg => ingest_svg(raw),
        raster => ingest_raster(raw, opts, raster),
    }
}

/// Determines the image format from the leading magic bytes.
fn sniff_format(raw: &[u8]) -> Result<ImageFormat> {
    if raw.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Ok(ImageFormat::Png)
    } else if raw.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Ok(ImageFormat::Jpeg)
    } else if raw.starts_with(b"GIF8") {
        Ok(ImageFormat::Gif)
    } else if raw.len() >= 12 && &raw[0..4] == b"RIFF" && &raw[8..12] == b"WEBP" {
        Ok(ImageFormat::Webp)
    } else if looks_like_svg(raw) {
        Ok(ImageFormat::Svg)
    } else {
        Err(Error::UnsupportedMime)
    }
}

/// Returns `true` when the buffer (after trimming leading whitespace) begins
/// with an XML declaration or an `<svg` tag.
fn looks_like_svg(raw: &[u8]) -> bool {
    let prefix: Vec<u8> = raw.iter().copied().take(256).collect();
    let text = String::from_utf8_lossy(&prefix);
    let t = text.trim_start();
    t.starts_with("<?xml") || t.starts_with("<svg")
}

/// Ingests a raster format (PNG/JPEG/GIF/WebP), enforcing dimension caps and
/// stripping metadata unless preservation is requested.
///
/// The first frame is decoded; GIF and animated WebP keep only that frame on
/// re-encode.
fn ingest_raster(raw: &[u8], opts: &IngestOptions, format: ImageFormat) -> Result<IngestedImage> {
    let img = image::load_from_memory(raw).map_err(|e| Error::Corrupt(e.to_string()))?;
    let (width, height) = (img.width(), img.height());

    if width > opts.max_dim || height > opts.max_dim {
        return Err(Error::TooLargeDim { max: opts.max_dim });
    }

    let bytes = if opts.preserve_exif {
        raw.to_vec()
    } else {
        reencode_clean(&img, format)?
    };

    Ok(IngestedImage {
        bytes,
        mime: format.mime(),
        format,
        width,
        height,
    })
}

/// Re-encodes a decoded image to a clean byte stream with no metadata.
fn reencode_clean(img: &image::DynamicImage, format: ImageFormat) -> Result<Vec<u8>> {
    let encoder_format = match format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Gif => image::ImageFormat::Gif,
        ImageFormat::Webp => image::ImageFormat::WebP,
        ImageFormat::Svg => image::ImageFormat::Png,
    };

    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), encoder_format)
        .map_err(|e| Error::Corrupt(e.to_string()))?;
    Ok(out)
}

/// Ingests an SVG by validating it against the safety rules. Clean input is
/// returned verbatim (trivially deterministic); unsafe input is rejected.
fn ingest_svg(raw: &[u8]) -> Result<IngestedImage> {
    let text = std::str::from_utf8(raw).map_err(|_| Error::Corrupt("invalid UTF-8".into()))?;
    validate_svg(text)?;

    let trimmed = text.trim_start();
    let (width, height) = parse_svg_dimensions(trimmed).unwrap_or((0, 0));

    Ok(IngestedImage {
        bytes: raw.to_vec(),
        mime: ImageFormat::Svg.mime(),
        format: ImageFormat::Svg,
        width,
        height,
    })
}

/// Validates an SVG document against the safety rules (`PRODUCT_SPEC.md` §7.4).
///
/// Rejects:
/// - any `<script>` element,
/// - any attribute beginning with `on` (event handlers such as `onclick`),
/// - any `href`, `xlink:href`, or `src` attribute pointing to a non-relative
///   URL (absolute `http(s)://`, `file://`, and `data:` URIs are all rejected).
fn validate_svg(text: &str) -> Result<()> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if name.eq_ignore_ascii_case("script") {
                    return Err(Error::UnsafeSvg("contains a <script> element".into()));
                }
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref());
                    let lower = key.to_ascii_lowercase();
                    if lower.starts_with("on") && lower.len() > 2 {
                        return Err(Error::UnsafeSvg(format!("event-handler attribute '{key}'")));
                    }
                    if matches!(lower.as_str(), "href" | "xlink:href" | "src") {
                        let value = attr.unescape_value().unwrap_or_default();
                        if is_unsafe_url(&value) {
                            return Err(Error::UnsafeSvg(format!("external reference '{value}'")));
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(Error::Corrupt(format!("XML parse error: {e}"))),
        }
        buf.clear();
    }
    Ok(())
}

/// Returns `true` when a URL is not a safe relative reference.
fn is_unsafe_url(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.starts_with('#') {
        return false;
    }
    let lowered = trimmed.to_ascii_lowercase();
    lowered.starts_with("http://")
        || lowered.starts_with("https://")
        || lowered.starts_with("file://")
        || lowered.starts_with("data:")
        || lowered.contains("://")
}

/// Best-effort extraction of `width`/`height` attributes from the root `<svg>`.
fn parse_svg_dimensions(text: &str) -> Option<(u32, u32)> {
    let svg_start = text.find("<svg")?;
    let tag_end = text[svg_start..].find('>')?;
    let tag = &text[svg_start..svg_start + tag_end];
    let w = extract_attr(tag, "width")?;
    let h = extract_attr(tag, "height")?;
    Some((w, h))
}

/// Extracts a numeric attribute value from an SVG tag fragment.
fn extract_attr(tag: &str, name: &str) -> Option<u32> {
    let pattern = format!("{name}=\"");
    let start = tag.find(&pattern)? + pattern.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    rest[..end].parse().ok()
}

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal valid 1x1 PNG produced by the image crate.
    fn real_png_1x1() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("encode png");
        out
    }

    /// A minimal valid 1x1 JPEG produced by the image crate.
    fn real_jpeg_1x1() -> Vec<u8> {
        let img = image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 0]));
        let mut out = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut out),
            image::ImageFormat::Jpeg,
        )
        .expect("encode jpeg");
        out
    }

    #[test]
    fn rejects_non_image_bytes() {
        let err = ingest(b"hello world", &IngestOptions::default()).unwrap_err();
        assert_eq!(err, Error::UnsupportedMime);
    }

    #[test]
    fn rejects_oversized_input() {
        let opts = IngestOptions {
            max_bytes: 4,
            ..IngestOptions::default()
        };
        let err = ingest(&real_png_1x1(), &opts).unwrap_err();
        assert_eq!(
            err,
            Error::TooLarge { max: 4 },
            "should reject before decoding"
        );
    }

    #[test]
    fn ingests_png() {
        let img = ingest(&real_png_1x1(), &IngestOptions::default()).expect("ingest png");
        assert_eq!(img.format, ImageFormat::Png);
        assert_eq!(img.mime, "image/png");
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert!(img.bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]));
    }

    #[test]
    fn ingests_jpeg() {
        let img = ingest(&real_jpeg_1x1(), &IngestOptions::default()).expect("ingest jpeg");
        assert_eq!(img.format, ImageFormat::Jpeg);
        assert_eq!(img.mime, "image/jpeg");
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn preserves_exif_returns_input_verbatim() {
        let opts = IngestOptions {
            preserve_exif: true,
            ..IngestOptions::default()
        };
        let input = real_png_1x1();
        let img = ingest(&input, &opts).expect("ingest");
        assert_eq!(img.bytes, input, "preserve_exif returns bytes verbatim");
    }

    #[test]
    fn exif_strip_re_encodes_clean_jpeg() {
        // The image crate's JPEG re-encode carries no EXIF; assert the stripped
        // output is still a valid, decodable JPEG.
        let input = real_jpeg_1x1();
        let img = ingest(&input, &IngestOptions::default()).expect("ingest");
        assert!(img.bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
        image::load_from_memory(&img.bytes).expect("re-decode clean jpeg");
    }

    #[test]
    fn ingests_clean_svg() {
        let svg = b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"50\"><rect/></svg>";
        let img = ingest(svg, &IngestOptions::default()).expect("ingest svg");
        assert_eq!(img.format, ImageFormat::Svg);
        assert_eq!(img.mime, "image/svg+xml");
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 50);
        assert_eq!(img.bytes, svg.to_vec());
    }

    #[test]
    fn rejects_svg_with_script_element() {
        let svg = b"<svg><script>alert(1)</script></svg>";
        let err = ingest(svg, &IngestOptions::default()).unwrap_err();
        assert!(matches!(err, Error::UnsafeSvg(_)));
    }

    #[test]
    fn rejects_svg_with_onload_attribute() {
        let svg = b"<svg onload=\"alert(1)\"></svg>";
        let err = ingest(svg, &IngestOptions::default()).unwrap_err();
        assert!(matches!(err, Error::UnsafeSvg(_)));
    }

    #[test]
    fn rejects_svg_with_external_xlink_href() {
        let svg = b"<svg xmlns:xlink=\"http://www.w3.org/1999/xlink\"><use xlink:href=\"https://evil.example/x#y\"/></svg>";
        let err = ingest(svg, &IngestOptions::default()).unwrap_err();
        assert!(matches!(err, Error::UnsafeSvg(_)));
    }

    #[test]
    fn rejects_svg_with_data_uri() {
        let svg = b"<svg><image href=\"data:image/png;base64,AAAA\"/></svg>";
        let err = ingest(svg, &IngestOptions::default()).unwrap_err();
        assert!(matches!(err, Error::UnsafeSvg(_)));
    }

    #[test]
    fn allows_svg_with_relative_ref_and_fragment() {
        let svg = b"<svg><use href=\"#shape\"/><image href=\"local.png\"/></svg>";
        let img = ingest(svg, &IngestOptions::default()).expect("safe refs allowed");
        assert_eq!(img.format, ImageFormat::Svg);
    }

    #[test]
    fn rejects_svg_with_file_url() {
        let svg = b"<svg><image href=\"file:///etc/passwd\"/></svg>";
        let err = ingest(svg, &IngestOptions::default()).unwrap_err();
        assert!(matches!(err, Error::UnsafeSvg(_)));
    }

    #[test]
    fn determinism_same_input_same_output() {
        let opts = IngestOptions::default();
        let a = ingest(&real_png_1x1(), &opts).expect("ingest a");
        let b = ingest(&real_png_1x1(), &opts).expect("ingest b");
        assert_eq!(a.bytes, b.bytes);
        assert_eq!(a.width, b.width);
        assert_eq!(a.height, b.height);
    }

    #[test]
    fn sniffs_by_magic_not_extension() {
        let fake = b"PNG is great";
        let err = ingest(fake, &IngestOptions::default()).unwrap_err();
        assert_eq!(err, Error::UnsupportedMime);
    }

    #[test]
    fn signature_only_png_is_corrupt() {
        let truncated = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature only
        ];
        let err = ingest(&truncated, &IngestOptions::default()).unwrap_err();
        assert!(
            matches!(err, Error::Corrupt(_)),
            "truncated PNG must be corrupt, got {err:?}"
        );
    }
}
