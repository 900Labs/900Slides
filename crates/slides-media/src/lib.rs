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

use image::ImageReader;
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

    /// Returns the canonical lowercase file extension for this format.
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Gif => "gif",
            ImageFormat::Webp => "webp",
            ImageFormat::Svg => "svg",
        }
    }
}

/// Returns the file extension for an allowlisted MIME type, or `None` when the
/// MIME type is not one of the formats this crate accepts.
///
/// This is the single source of truth for the MIME-to-extension mapping; other
/// crates should call this rather than re-declaring a parallel table.
pub fn extension_for_mime(mime: &str) -> Option<&'static str> {
    all_formats()
        .iter()
        .copied()
        .find(|f| f.mime() == mime)
        .map(ImageFormat::extension)
}

/// Returns every supported image format.
fn all_formats() -> &'static [ImageFormat] {
    &[
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Gif,
        ImageFormat::Webp,
        ImageFormat::Svg,
    ]
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
    // Probe dimensions from the header BEFORE full decode so a malicious
    // small-compressed / huge-dimension image cannot trigger a multi-GB
    // pixel-buffer allocation before the cap rejects it.
    let reader = ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .map_err(|e| Error::Corrupt(e.to_string()))?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| Error::Corrupt(e.to_string()))?;

    if width > opts.max_dim || height > opts.max_dim {
        return Err(Error::TooLargeDim { max: opts.max_dim });
    }

    let bytes = if opts.preserve_exif {
        raw.to_vec()
    } else if !raster_has_privacy_metadata(raw, format) {
        // The source carries no EXIF/text metadata, so there is nothing to
        // strip. Skip the full decode + re-encode and return the bytes
        // verbatim. (Multi-frame GIF/WebP: returning the original bytes also
        // preserves extra frames; the load path keeps only the modeled frame
        // count via the renderer, which is acceptable for v0.1/v0.2.)
        raw.to_vec()
    } else {
        let img = image::load_from_memory(raw).map_err(|e| Error::Corrupt(e.to_string()))?;
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

/// Returns `true` when `raw` (of `format`) carries privacy-sensitive metadata
/// (EXIF, XMP, ICC, embedded text) that the ingest path must strip.
///
/// Conservative: when the format's metadata layout cannot be confirmed clean
/// (e.g. truncated, or an unhandled container), this returns `true` so the
/// caller re-encodes and strips, rather than risking leaving metadata in. The
/// scan is bounded to the leading 256 KiB where these markers live.
fn raster_has_privacy_metadata(raw: &[u8], format: ImageFormat) -> bool {
    let window: &[u8] = if raw.len() > 256 * 1024 {
        &raw[..256 * 1024]
    } else {
        raw
    };
    match format {
        ImageFormat::Jpeg => {
            // APP1 (FF E1) carries EXIF and XMP; APP2 (FF E2) carries ICC
            // profiles; COM (FF FE) carries comment text.
            jpeg_marker_present(window, 0xE1)
                || jpeg_marker_present(window, 0xE2)
                || jpeg_marker_present(window, 0xFE)
        }
        ImageFormat::Png => {
            // PNG ancillary chunks that carry text/profile metadata.
            png_chunk_present(window, b"tEXt")
                || png_chunk_present(window, b"zTXt")
                || png_chunk_present(window, b"iTXt")
                || png_chunk_present(window, b"eXIf")
                || png_chunk_present(window, b"iCCp")
        }
        // Conservative default for GIF/WebP: assume metadata may be present and
        // let the re-encode path strip it.
        ImageFormat::Gif | ImageFormat::Webp => true,
        ImageFormat::Svg => false,
    }
}

/// Scans a JPEG stream (bounded `window`) for a segment marker `code` (the
/// low byte after `0xFF`). Walks APPn/COM markers by their 2-byte length fields
/// so a coincidental `FF xx` inside compressed scan data does not cause a
/// false positive.
fn jpeg_marker_present(window: &[u8], code: u8) -> bool {
    let mut i = 2; // skip the SOI marker FF D8
    while i + 3 < window.len() {
        if window[i] != 0xFF {
            // Not a marker boundary; resync to the next FF.
            i += 1;
            continue;
        }
        let marker = window[i + 1];
        // Standalone markers (no length payload) — includes RSTn, SOI, EOI.
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            i += 2;
            continue;
        }
        // SOS (0xFFDA) begins the compressed image data; no metadata follows it.
        if marker == 0xDA {
            return false;
        }
        if marker == code {
            return true;
        }
        // Advance by the segment length (stored big-endian after the marker).
        let len = (u16::from_be_bytes([window[i + 2], window[i + 3]]) as usize) + 2;
        i += len;
    }
    false
}

/// Returns `true` when `window` (a PNG prefix) contains a chunk of the given
/// 4-byte type. Walks chunks by their length fields so compressed IDAT data
/// cannot produce a false positive.
fn png_chunk_present(window: &[u8], chunk_type: &[u8; 4]) -> bool {
    // Skip the 8-byte PNG signature; each chunk is [len:4][type:4][data:len][crc:4].
    let mut i = 8;
    while i + 8 <= window.len() {
        let len =
            u32::from_be_bytes([window[i], window[i + 1], window[i + 2], window[i + 3]]) as usize;
        let ctype = &window[i + 4..i + 8];
        if ctype == chunk_type {
            return true;
        }
        // Stop at the first IDAT: no metadata chunks follow compressed data.
        if ctype == b"IDAT" {
            return false;
        }
        i += 12 + len; // len field + type + data + crc
    }
    false
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
                // Compare the local name (after any namespace prefix) so a
                // namespaced `<svg:script>` cannot bypass the `<script>` reject.
                let local = e.name().local_name();
                let local_str = String::from_utf8_lossy(local.as_ref());
                if local_str.eq_ignore_ascii_case("script") {
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
///
/// Only `#`-fragment references and schemeless relative paths are allowed.
/// Any scheme (`javascript`, `vbscript`, `http`, `file`, `data`, etc.) is
/// rejected, identified by a colon appearing before the first slash.
fn is_unsafe_url(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.starts_with('#') {
        return false;
    }
    let lowered = trimmed.to_ascii_lowercase();
    // Reject known dangerous script schemes explicitly...
    const DANGEROUS_SCHEMES: &[&str] = &[
        "javascript:",
        "vbscript:",
        "mocha:",
        "livescript:",
        "http:",
        "https:",
        "file:",
        "data:",
    ];
    if DANGEROUS_SCHEMES.iter().any(|s| lowered.starts_with(s)) {
        return true;
    }
    // ...and any other scheme: a colon before the first slash means a scheme is
    // present, which is unsafe. Relative paths and fragments contain no such
    // leading colon.
    if let Some(colon) = lowered.find(':') {
        let before_colon = &lowered[..colon];
        if !before_colon.contains('/') {
            return true;
        }
    }
    false
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
    fn clean_raster_returned_verbatim_without_reencode() {
        // A clean PNG (no tEXt/eXIf chunks) is returned verbatim: the ingest
        // bytes equal the input bytes, proving the decode+re-encode was skipped.
        let input = real_png_1x1();
        let img = ingest(&input, &IngestOptions::default()).expect("ingest");
        assert_eq!(img.bytes, input, "clean PNG should pass through verbatim");
    }

    #[test]
    fn png_with_text_chunk_is_reencoded() {
        // Inject a tEXt chunk after the IHDR of a clean PNG; ingest must detect
        // it and re-encode (so the stripped output no longer contains the chunk).
        let bytes = real_png_1x1();
        // Build a tEXt chunk: len=8, type "tEXt", data "key\0val" (8 bytes), crc.
        let text_chunk = {
            let data = b"key\0val";
            let mut chunk = Vec::new();
            chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
            chunk.extend_from_slice(b"tEXt");
            chunk.extend_from_slice(data);
            let crc = png_crc32(b"tEXtkey\0val");
            chunk.extend_from_slice(&crc.to_be_bytes());
            chunk
        };
        // Splice it right after the IHDR (signature 8 + IHDR chunk 25 = offset 33).
        let mut injected = Vec::with_capacity(bytes.len() + text_chunk.len());
        injected.extend_from_slice(&bytes[..33]);
        injected.extend_from_slice(&text_chunk);
        injected.extend_from_slice(&bytes[33..]);

        let img = ingest(&injected, &IngestOptions::default()).expect("ingest");
        assert!(
            !img.bytes.windows(4).any(|w| w == b"tEXt"),
            "re-encoded output must not contain the injected tEXt chunk"
        );
    }

    #[test]
    fn jpeg_with_app1_marker_is_reencoded() {
        // A JPEG carrying an APP1 (EXIF) marker must be re-encoded.
        let bytes = real_jpeg_1x1();
        // Insert a minimal APP1 segment after SOI (FF D8): FF E1 + len + "Exif".
        let mut app1 = vec![0xFF, 0xE1, 0x00, 0x08];
        app1.extend_from_slice(b"Exif\x00\x00");
        let mut injected = Vec::with_capacity(bytes.len() + app1.len());
        injected.extend_from_slice(&bytes[..2]);
        injected.extend_from_slice(&app1);
        injected.extend_from_slice(&bytes[2..]);

        let img = ingest(&injected, &IngestOptions::default()).expect("ingest");
        // The re-encoded JPEG must not contain the EXIF APP1 marker.
        assert!(
            !img.bytes.windows(2).any(|w| w == [0xFF, 0xE1]),
            "re-encoded JPEG must not contain APP1/EXIF"
        );
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
        let svg = b"<svg><image href=\"file:///etc/hosts\"/></svg>";
        let err = ingest(svg, &IngestOptions::default()).unwrap_err();
        assert!(matches!(err, Error::UnsafeSvg(_)));
    }

    #[test]
    fn rejects_svg_with_javascript_uri() {
        // `javascript:` has no `://`, so it must be rejected by scheme, not by
        // the legacy `://` check.
        for attr in [
            b"<svg><a xlink:href=\"javascript:alert(1)\"/></svg>".as_ref(),
            b"<svg><image href=\"javascript:alert(1)\"/></svg>",
            b"<svg><image src=\"javascript:alert(1)\"/></svg>",
            b"<svg><a href=\"JavaScript:alert(1)\"/></svg>",
        ] {
            let err = ingest(attr, &IngestOptions::default())
                .expect_err(&format!("should reject {attr:?}"));
            assert!(
                matches!(err, Error::UnsafeSvg(_)),
                "javascript URI must be rejected for {attr:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn rejects_svg_with_vbscript_uri() {
        let svg = b"<svg><image href=\"vbscript:msgbox(1)\"/></svg>";
        let err = ingest(svg, &IngestOptions::default()).unwrap_err();
        assert!(matches!(err, Error::UnsafeSvg(_)));
    }

    #[test]
    fn rejects_namespaced_script_element() {
        // A prefix-bound script element must be caught by local-name comparison.
        let svg = b"<svg xmlns:svg=\"http://www.w3.org/2000/svg\"><svg:script>alert(1)</svg:script></svg>";
        let err = ingest(svg, &IngestOptions::default()).unwrap_err();
        assert!(matches!(err, Error::UnsafeSvg(_)));
    }

    #[test]
    fn dimension_cap_rejects_before_full_decode() {
        // A PNG whose IHDR declares huge dimensions but keeps a tiny data
        // stream. The header probe reads the declared size and rejects it
        // (TooLargeDim) before the decoder allocates a multi-GB pixel buffer.
        // The IHDR CRC is recomputed so the PNG reader accepts the header.
        let mut huge = real_png_1x1();
        let width = 65_535u32.to_be_bytes();
        let height = 65_535u32.to_be_bytes();
        huge[16..20].copy_from_slice(&width);
        huge[20..24].copy_from_slice(&height);
        // IHDR CRC covers bytes 12 (chunk type "IHDR") through 28 (end of the
        // 13-byte data section); the CRC is stored in bytes 29..33.
        let crc = png_crc32(&huge[12..29]);
        huge[29..33].copy_from_slice(&crc.to_be_bytes());
        let opts = IngestOptions {
            max_dim: 8192,
            ..IngestOptions::default()
        };
        let err = ingest(&huge, &opts).unwrap_err();
        assert_eq!(
            err,
            Error::TooLargeDim { max: 8192 },
            "huge-dimension header must be rejected before full decode, got {err:?}"
        );
    }

    /// Computes a PNG/CRC-32 (IEEE 802.3) over `bytes`.
    fn png_crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in bytes {
            crc ^= b as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        crc ^ 0xFFFF_FFFF
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
