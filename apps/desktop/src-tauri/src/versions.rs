//! Content-addressed local version-history snapshot store.
//!
//! Snapshots live at `<app_data>/900Slides/versions/<deck_id>/<hash>.json`,
//! where `<app_data>` is the 900Slides app-data root passed in as
//! `app_data_dir`. Each snapshot records the full serialized
//! [`slides_core::Deck`] model plus metadata (a timestamp and an optional
//! user-assigned name). The hash is the SHA-256 of the serialized deck JSON,
//! so two saves of an identical deck produce the same hash and are
//! deduplicated: the snapshot file and index are left untouched.
//!
//! A `versions.json` index file in each deck's directory lists every snapshot
//! (hash, timestamp, name) sorted by timestamp, so listing never needs to read
//! individual snapshot files. This is a desktop-only, fully local feature:
//! nothing here performs any network access.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// SHA-256 digest length, in hexadecimal characters.
const HASH_HEX_LEN: usize = 64;
/// Cap on the number of text excerpts recorded per modified slide in a diff.
const TEXT_DIFF_CAP: usize = 6;

/// Lightweight index entry for a snapshot: no deck payload.
///
/// Doubles as the DTO returned to the frontend by `list_snapshots`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfo {
    /// Content hash of the snapshot's deck JSON.
    pub hash: String,
    /// ISO 8601 UTC timestamp of the save that created this snapshot.
    pub timestamp: String,
    /// Optional user-assigned label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Full on-disk snapshot record: index metadata plus the serialized deck.
///
/// Field names are snake_case (notably `deck_json`) to match the documented
/// on-disk format.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotRecord {
    hash: String,
    timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    deck_json: String,
}

/// On-disk index file listing every snapshot for a deck, sorted by timestamp.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct VersionIndex {
    #[serde(default)]
    snapshots: Vec<SnapshotInfo>,
}

/// Structural difference between two decks (see [`diff_snapshots`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDiff {
    /// Slide ids present in `deck_b` but not `deck_a`.
    pub slides_added: Vec<String>,
    /// Slide ids present in `deck_a` but not `deck_b`.
    pub slides_removed: Vec<String>,
    /// Slides present in both decks whose content differs.
    pub slides_modified: Vec<SlideDiff>,
}

/// Structural difference for a single slide present in both decks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideDiff {
    /// Id of the modified slide.
    pub slide_id: String,
    /// Number of shapes present only in the second deck.
    pub shapes_added: usize,
    /// Number of shapes present only in the first deck.
    pub shapes_removed: usize,
    /// Text excerpts that differ between the two versions of the slide.
    pub text_changed: Vec<String>,
}

/// Serializes the deck, computes its content hash, deduplicates, writes the
/// snapshot, and updates the index. Returns the hash.
///
/// The hash is the SHA-256 of `serde_json::to_string(deck)` — the deck model
/// excludes timestamp/name metadata, so identical decks always hash alike and
/// are deduplicated.
pub fn save_snapshot(
    app_data_dir: &Path,
    deck_id: &str,
    deck: &slides_core::Deck,
) -> Result<String, String> {
    let deck_json = serde_json::to_string(deck).map_err(|e| e.to_string())?;
    let hash = sha256_hex(deck_json.as_bytes());
    let dir = deck_version_dir(app_data_dir, deck_id)?;
    let snapshot_path = dir.join(format!("{hash}.json"));

    // Write the snapshot file only when it is absent (content-addressed
    // deduplication). When it already exists we reuse its original timestamp.
    let timestamp = if snapshot_path.exists() {
        match read_index(&dir) {
            Ok(index) => index
                .snapshots
                .into_iter()
                .find(|s| s.hash == hash)
                .map(|s| s.timestamp),
            Err(_) => None,
        }
        .map_or_else(|| read_record(&snapshot_path).map(|r| r.timestamp), Ok)?
    } else {
        let ts = iso8601_now();
        let record = SnapshotRecord {
            hash: hash.clone(),
            timestamp: ts.clone(),
            name: None,
            deck_json,
        };
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
        atomic_write(&snapshot_path, &bytes)?;
        ts
    };

    // Ensure the index tracks this hash (idempotent across deduplicated saves).
    let mut index = read_index(&dir)?;
    if !index.snapshots.iter().any(|s| s.hash == hash) {
        index.snapshots.push(SnapshotInfo {
            hash: hash.clone(),
            timestamp,
            name: None,
        });
        sort_index(&mut index);
        write_index(&dir, &index)?;
    }

    Ok(hash)
}

/// Lists every snapshot for a deck, newest first, reading only the index file.
pub fn list_snapshots(app_data_dir: &Path, deck_id: &str) -> Result<Vec<SnapshotInfo>, String> {
    let dir = deck_version_dir(app_data_dir, deck_id)?;
    let mut index = read_index(&dir)?;
    sort_index(&mut index);
    // Newest first.
    index.snapshots.reverse();
    Ok(index.snapshots)
}

/// Loads a specific snapshot's deck model.
pub fn load_snapshot(
    app_data_dir: &Path,
    deck_id: &str,
    hash: &str,
) -> Result<slides_core::Deck, String> {
    validate_hash(hash)?;
    let dir = deck_version_dir(app_data_dir, deck_id)?;
    let record = read_record(&dir.join(format!("{hash}.json")))?;
    serde_json::from_str(&record.deck_json).map_err(|e| e.to_string())
}

/// Assigns (or replaces) a user-assigned label on a snapshot. Updates both the
/// snapshot record and its index entry.
pub fn name_snapshot(
    app_data_dir: &Path,
    deck_id: &str,
    hash: &str,
    name: &str,
) -> Result<(), String> {
    validate_hash(hash)?;
    let dir = deck_version_dir(app_data_dir, deck_id)?;
    let snapshot_path = dir.join(format!("{hash}.json"));

    if snapshot_path.exists() {
        let mut record = read_record(&snapshot_path)?;
        record.name = Some(name.to_string());
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
        atomic_write(&snapshot_path, &bytes)?;
    }

    let mut index = read_index(&dir)?;
    let mut updated = false;
    for entry in index.snapshots.iter_mut() {
        if entry.hash == hash {
            entry.name = Some(name.to_string());
            updated = true;
        }
    }
    if updated {
        write_index(&dir, &index)?;
    }
    Ok(())
}

/// Computes a lightweight structural diff between two decks (slides
/// added/removed/modified, and per-slide shape/text changes). This is a
/// structural diff, not a pixel diff.
pub fn diff_snapshots(deck_a: &slides_core::Deck, deck_b: &slides_core::Deck) -> VersionDiff {
    use std::collections::HashSet;

    let ids_a: HashSet<&str> = deck_a.slides.iter().map(|s| s.id.as_str()).collect();
    let ids_b: HashSet<&str> = deck_b.slides.iter().map(|s| s.id.as_str()).collect();

    let mut slides_added: Vec<String> = ids_b
        .iter()
        .filter(|id| !ids_a.contains(*id))
        .map(|s| s.to_string())
        .collect();
    let mut slides_removed: Vec<String> = ids_a
        .iter()
        .filter(|id| !ids_b.contains(*id))
        .map(|s| s.to_string())
        .collect();
    slides_added.sort();
    slides_removed.sort();

    let mut slides_modified = Vec::new();
    for a in &deck_a.slides {
        if let Some(b) = deck_b.slides.iter().find(|s| s.id == a.id) {
            let diff = diff_slide(a, b);
            if diff.shapes_added > 0 || diff.shapes_removed > 0 || !diff.text_changed.is_empty() {
                slides_modified.push(diff);
            }
        }
    }
    slides_modified.sort_by(|a, b| a.slide_id.cmp(&b.slide_id));

    VersionDiff {
        slides_added,
        slides_removed,
        slides_modified,
    }
}

/// Returns the per-deck version directory, creating it (and the `versions`
/// root) if needed.
fn deck_version_dir(app_data_dir: &Path, deck_id: &str) -> Result<PathBuf, String> {
    validate_deck_id(deck_id)?;
    let dir = app_data_dir.join("versions").join(deck_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Rejects deck ids that could escape the versions directory.
fn validate_deck_id(deck_id: &str) -> Result<(), String> {
    if deck_id.is_empty() {
        return Err("deck id is empty".to_string());
    }
    if deck_id.contains('/')
        || deck_id.contains('\\')
        || deck_id.starts_with('.')
        || deck_id.contains('\0')
    {
        return Err("deck id contains invalid characters".to_string());
    }
    Ok(())
}

/// Rejects anything that is not a 64-char lowercase-or-uppercase hex string.
fn validate_hash(hash: &str) -> Result<(), String> {
    if hash.len() != HASH_HEX_LEN || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid version hash".to_string());
    }
    Ok(())
}

/// Reads and parses a deck's index file, or returns an empty index when absent.
fn read_index(dir: &Path) -> Result<VersionIndex, String> {
    let path = dir.join("versions.json");
    if !path.exists() {
        return Ok(VersionIndex::default());
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Ok(VersionIndex::default());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// Writes a deck's index file atomically.
fn write_index(dir: &Path, index: &VersionIndex) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?;
    atomic_write(&dir.join("versions.json"), &bytes)
}

/// Reads and parses a single snapshot record file.
fn read_record(path: &Path) -> Result<SnapshotRecord, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// Sorts the index's snapshots by timestamp, ascending (oldest first).
fn sort_index(index: &mut VersionIndex) {
    index
        .snapshots
        .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
}

/// Writes bytes to a path via a temp file + atomic rename.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| "snapshot path has no parent".to_string())?;
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("snapshot")
    ));
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Computes the lowercase hex SHA-256 digest of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Returns the current time as an ISO 8601 UTC string (`YYYY-MM-DDTHH:MM:SSZ`).
fn iso8601_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    iso8601_from_secs(secs)
}

/// Formats a Unix epoch second count as an ISO 8601 UTC string.
fn iso8601_from_secs(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Converts days since 1970-01-01 to a Gregorian `(year, month, day)` date.
///
/// Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// Computes the structural diff for a single slide present in both decks.
fn diff_slide(a: &slides_core::Slide, b: &slides_core::Slide) -> SlideDiff {
    use std::collections::HashSet;

    let sigs_a: Vec<String> = a.shapes.iter().map(shape_signature).collect();
    let sigs_b: Vec<String> = b.shapes.iter().map(shape_signature).collect();
    let set_a: HashSet<&str> = sigs_a.iter().map(String::as_str).collect();
    let set_b: HashSet<&str> = sigs_b.iter().map(String::as_str).collect();
    let shapes_removed = sigs_a
        .iter()
        .filter(|s| !set_b.contains(s.as_str()))
        .count();
    let shapes_added = sigs_b
        .iter()
        .filter(|s| !set_a.contains(s.as_str()))
        .count();

    let texts_a = slide_text_excerpts(a);
    let texts_b = slide_text_excerpts(b);
    let text_a_set: HashSet<&str> = texts_a.iter().map(String::as_str).collect();
    let text_b_set: HashSet<&str> = texts_b.iter().map(String::as_str).collect();
    let mut text_changed: Vec<String> = texts_a
        .iter()
        .filter(|t| !text_b_set.contains(t.as_str()))
        .chain(texts_b.iter().filter(|t| !text_a_set.contains(t.as_str())))
        .cloned()
        .collect();
    text_changed.sort();
    text_changed.truncate(TEXT_DIFF_CAP);

    SlideDiff {
        slide_id: a.id.clone(),
        shapes_added,
        shapes_removed,
        text_changed,
    }
}

/// Returns a stable content signature for a shape, used to detect added or
/// removed shapes across two versions of a slide.
fn shape_signature(shape: &slides_core::Shape) -> String {
    match shape {
        slides_core::Shape::TextBox(text_box) => {
            format!("text|{}", text_box_text(text_box))
        }
        slides_core::Shape::Image(image) => format!("image|{}", image.media_ref),
        slides_core::Shape::Geometric(geometric) => {
            format!("geo|{:?}", geometric.geometry)
        }
        slides_core::Shape::Table(table) => {
            let cells: String = table
                .rows
                .iter()
                .flat_map(|r| r.cells.iter())
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join("|");
            format!(
                "table|{}x{}|{}",
                table.rows.len(),
                table.column_widths.len(),
                cells
            )
        }
        slides_core::Shape::Chart(chart) => {
            format!(
                "chart|{:?}|{}",
                chart.chart_type,
                chart.title.clone().unwrap_or_default()
            )
        }
        slides_core::Shape::Passthrough(passthrough) => {
            format!("pass|{}", passthrough.source_part)
        }
    }
}

/// Joins a text box's paragraph/run text into a single string.
fn text_box_text(text_box: &slides_core::TextBox) -> String {
    text_box
        .paragraphs
        .iter()
        .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collects human-readable text excerpts for a slide's shapes.
fn slide_text_excerpts(slide: &slides_core::Slide) -> Vec<String> {
    let mut excerpts = Vec::new();
    for shape in &slide.shapes {
        match shape {
            slides_core::Shape::TextBox(text_box) => {
                let text = text_box_text(text_box);
                if !text.is_empty() {
                    excerpts.push(text);
                }
            }
            slides_core::Shape::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        if !cell.text.is_empty() {
                            excerpts.push(cell.text.clone());
                        }
                    }
                }
            }
            slides_core::Shape::Chart(chart) => {
                if let Some(title) = &chart.title {
                    if !title.is_empty() {
                        excerpts.push(title.clone());
                    }
                }
            }
            slides_core::Shape::Passthrough(passthrough) => {
                if !passthrough.label.is_empty() {
                    excerpts.push(passthrough.label.clone());
                }
            }
            slides_core::Shape::Image(_) | slides_core::Shape::Geometric(_) => {}
        }
    }
    excerpts
}

#[cfg(test)]
mod tests {
    use super::*;
    use slides_core::{Deck, Rect, Slide, TextBox};

    fn text_box(id: &str, text: &str) -> TextBox {
        TextBox {
            id: id.to_string(),
            frame: Rect::new(0.0, 0.0, 100.0, 50.0),
            paragraphs: vec![slides_core::Paragraph {
                runs: vec![slides_core::Run {
                    text: text.to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        }
    }

    fn deck_with(id: &str, slides: Vec<Slide>) -> Deck {
        Deck {
            id: id.to_string(),
            slides,
            ..Default::default()
        }
    }

    fn slide(id: &str, text: &str) -> Slide {
        Slide {
            id: id.to_string(),
            shapes: vec![slides_core::Shape::TextBox(text_box("tb", text))],
            ..Default::default()
        }
    }

    #[test]
    fn sha256_is_deterministic_and_hex() {
        let h = sha256_hex(b"hello");
        assert_eq!(h.len(), HASH_HEX_LEN);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h, sha256_hex(b"hello"));
        assert_ne!(h, sha256_hex(b"world"));
    }

    #[test]
    fn save_dedupes_identical_decks() {
        let tmp = std::env::temp_dir().join(format!(
            "900slides-versions-dedup-{}",
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let deck = deck_with("deck-a", vec![slide("s1", "hello")]);
        let h1 = save_snapshot(&tmp, "deck-a", &deck).unwrap();
        let h2 = save_snapshot(&tmp, "deck-a", &deck).unwrap();
        assert_eq!(h1, h2, "identical decks share a hash");
        let list = list_snapshots(&tmp, "deck-a").unwrap();
        assert_eq!(list.len(), 1, "deduplicated to one index entry");
        assert_eq!(list[0].hash, h1);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn save_and_load_round_trips_deck() {
        let tmp = std::env::temp_dir().join(format!(
            "900slides-versions-rt-{}",
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let deck = deck_with("deck-b", vec![slide("s1", "first"), slide("s2", "second")]);
        let h = save_snapshot(&tmp, "deck-b", &deck).unwrap();
        let loaded = load_snapshot(&tmp, "deck-b", &h).unwrap();
        assert_eq!(loaded, deck);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn name_persists_across_list() {
        let tmp = std::env::temp_dir().join(format!(
            "900slides-versions-name-{}",
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let deck = deck_with("deck-c", vec![slide("s1", "x")]);
        let h = save_snapshot(&tmp, "deck-c", &deck).unwrap();
        name_snapshot(&tmp, "deck-c", &h, "Before review").unwrap();
        let list = list_snapshots(&tmp, "deck-c").unwrap();
        assert_eq!(list[0].name.as_deref(), Some("Before review"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn reject_bad_hash_and_deck_id() {
        let tmp = std::env::temp_dir().join("900slides-versions-validate-unused");
        let deck = deck_with("deck-d", vec![]);
        let h = save_snapshot(&tmp, "deck-d", &deck).unwrap();
        // Bad hash rejected.
        assert!(load_snapshot(&tmp, "deck-d", "nothex").is_err());
        // Good hash works.
        assert!(load_snapshot(&tmp, "deck-d", &h).is_ok());
        // Path-traversal deck id rejected.
        assert!(save_snapshot(&tmp, "../escape", &deck).is_err());
    }

    #[test]
    fn diff_reports_added_removed_modified() {
        let a = deck_with("d", vec![slide("s1", "alpha"), slide("s2", "beta")]);
        let mut b = deck_with("d", vec![slide("s1", "alpha"), slide("s3", "gamma")]);
        // Modify s1 in b by changing its text.
        b.slides[0] = slide("s1", "ALPHA");

        let diff = diff_snapshots(&a, &b);
        assert_eq!(diff.slides_added, vec!["s3".to_string()]);
        assert_eq!(diff.slides_removed, vec!["s2".to_string()]);
        assert_eq!(diff.slides_modified.len(), 1);
        assert_eq!(diff.slides_modified[0].slide_id, "s1");
        assert!(diff.slides_modified[0]
            .text_changed
            .contains(&"alpha".to_string()));
        assert!(diff.slides_modified[0]
            .text_changed
            .contains(&"ALPHA".to_string()));
    }

    #[test]
    fn civil_from_days_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2000-01-01 is 10957 days after 1970-01-01.
        assert_eq!(civil_from_days(10957), (2000, 1, 1));
        // 2026-07-29 is 20663 days after 1970-01-01.
        assert_eq!(civil_from_days(20663), (2026, 7, 29));
        assert_eq!(iso8601_from_secs(0), "1970-01-01T00:00:00Z");
    }
}
