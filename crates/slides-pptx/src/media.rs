//! Media helpers shared by the loader and saver: content-addressed media keys,
//! MIME/extension mapping, and OOXML relative-target computation.

use std::hash::Hasher;

use crate::package::normalize_package_path;

/// Computes a deterministic, content-addressed key for media bytes.
///
/// The key is `img-<xxh64-hex>` over the ingested bytes, so identical images
/// dedup to a single [`slides_core::MediaStore`] entry regardless of how many
/// shapes reference them or which package part they came from. Exposed
/// publicly so callers that insert images through other paths (e.g. the
/// desktop command surface) use the same keying scheme as the loader.
pub fn media_key(bytes: &[u8]) -> String {
    use twox_hash::XxHash64;
    let mut hasher = XxHash64::default();
    hasher.write(bytes);
    format!("img-{:016x}", hasher.finish())
}

/// Returns the canonical file extension for a MIME type, delegating to the
/// single source of truth in [`slides_media`] so the allowlist cannot drift
/// between the two crates. Returns `None` when the MIME type is not one of the
/// formats [`slides_media`] accepts.
pub(crate) fn extension_for_mime(mime: &str) -> Option<&'static str> {
    slides_media::extension_for_mime(mime)
}

/// Computes a relationship target path for `target_path` relative to the
/// directory of `base_part`.
///
/// Both inputs are absolute-in-package paths (e.g. `ppt/slides/slide1.xml` and
/// `ppt/media/image1.png`). The returned string is suitable for a `Target=`
/// attribute on a relationship stored alongside `base_part` (for example,
/// `../media/image1.png`).
pub(crate) fn relative_target(base_part: &str, target_path: &str) -> String {
    let base_dir = package_base_dir(base_part);
    let base_segments: Vec<&str> = base_dir.split('/').filter(|s| !s.is_empty()).collect();
    let target_segments: Vec<&str> = target_path.split('/').filter(|s| !s.is_empty()).collect();

    // Find the common leading prefix.
    let common = base_segments
        .iter()
        .zip(target_segments.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let up = base_segments.len().saturating_sub(common);
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), up).collect();
    parts.extend(target_segments[common..].iter().map(|s| (*s).to_string()));
    parts.join("/")
}

/// Returns the directory portion of an absolute-in-package part path.
fn package_base_dir(part: &str) -> String {
    let normalized = normalize_package_path(part);
    match normalized.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_key_is_stable_and_distinguishes_bytes() {
        let a = media_key(b"abc");
        let b = media_key(b"abc");
        let c = media_key(b"abd");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("img-"));
    }

    #[test]
    fn relative_target_walks_up_to_common_root() {
        assert_eq!(
            relative_target("ppt/slides/slide1.xml", "ppt/media/image1.png"),
            "../media/image1.png"
        );
        assert_eq!(
            relative_target("ppt/slides/slide1.xml", "ppt/slides/slide2.xml"),
            "slide2.xml"
        );
        assert_eq!(
            relative_target("ppt/slides/slide1.xml", "ppt/theme/theme1.xml"),
            "../theme/theme1.xml"
        );
    }
}
