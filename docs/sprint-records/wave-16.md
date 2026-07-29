# Wave 16 — v0.4.0 local version history

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.4.0 ("Local version history. Every save
is a content-addressed snapshot. Named versions, restore, 'copy this
revision', and a visual diff between two revisions.")
Last updated: 2026-07-29

Wave 16 adds **local version history**: every save creates a
content-addressed snapshot of the deck model. Users can name versions,
restore them, copy a revision into a new deck, and see a visual diff
between any two revisions. All stored locally — no cloud, no sync.

## Design

Version history is a **desktop-only feature** — it doesn't change the
slides-core model, the PPTX format, or the renderer. Snapshots are stored as
serialized deck JSON (the model's `serde` output) in a per-deck directory
under the app data folder. Each snapshot is content-addressed (SHA-256 of
the serialized JSON) so identical saves are deduplicated.

## What this wave delivers

| # | Component | Location | New vs. extend |
| --- | --- | --- | --- |
| 1 | Version store | `apps/desktop/src-tauri/src/versions.rs` | New: content-addressed snapshot storage |
| 2 | Tauri commands | `apps/desktop/src-tauri/src/commands.rs` | New commands: list/restore/diff/name versions |
| 3 | Desktop UI | `apps/desktop/src/VersionHistory.svelte` | New: version list, name/restore/diff UI |

## Content-addressed snapshot store

Snapshots live at `<app_data>/900Slides/versions/<deck_id>/<hash>.json`:

```json
{
  "hash": "a1b2c3...",
  "timestamp": "2026-07-29T14:30:00Z",
  "name": null,
  "deck_json": "...serialized Deck model..."
}
```

- **Hash**: SHA-256 of the serialized deck JSON (excluding timestamp/name).
  Two saves of an identical deck produce the same hash → deduplicated.
- **Timestamp**: when the save occurred (ISO 8601 UTC).
- **Name**: optional user-assigned label (e.g. "Before client review").
- **deck_json**: the full serialized `slides_core::Deck` via `serde_json`.

A `versions.json` index file in each deck's directory lists all snapshots
(sorted by timestamp) so listing doesn't require reading every file.

## Hook into save

When `save_deck` is called:
1. Serialize the current deck model to JSON.
2. Compute SHA-256 of the JSON.
3. If the hash already exists in the version store for this deck, skip
   (deduplication).
4. Otherwise, write the snapshot to the version directory and update the
   index.
5. Proceed with the normal PPTX save.

## Commands

- `list_versions(deck_id) -> Vec<VersionInfoDto>` — returns the version
  index (hash, timestamp, name) without loading deck content.
- `get_version(deck_id, hash) -> DeckSnapshot` — loads a specific version's
  deck model and returns it as a snapshot DTO.
- `restore_version(deck_id, hash) -> DeckSnapshot` — loads a version's deck
  model, replaces the current session's deck, and returns the snapshot. The
  restore itself is reversible via undo (it goes through the command bus as
  a "restore" command that replaces the entire deck).
- `name_version(deck_id, hash, name)` — assigns a label to a version.
- `diff_versions(deck_id, hash_a, hash_b) -> VersionDiffDto` — returns a
  structural diff between two versions (slides added/removed/modified,
  shapes changed). This is a lightweight diff — not a pixel diff.

## Desktop UI (`VersionHistory.svelte`)

- A panel accessible from File → Version History (or a sidebar tab).
- **Version list**: chronological, showing timestamp, name (if any), and a
  hash prefix. Newest first.
- Click a version → **preview** (loads the deck snapshot and shows a
  read-only thumbnail strip).
- **Name** button → prompts for a label.
- **Restore** button → restores the deck to that version (reversible via
  undo).
- **Copy** button → opens the version as a new untitled deck.
- **Diff** button → select two versions → shows a structural diff
  (slides/sections/shapes added, removed, changed).

## Diff model

```rust
pub struct VersionDiff {
    pub slides_added: Vec<String>,    // slide ids
    pub slides_removed: Vec<String>,
    pub slides_modified: Vec<SlideDiff>,
}

pub struct SlideDiff {
    pub slide_id: String,
    pub shapes_added: usize,
    pub shapes_removed: usize,
    pub text_changed: Vec<String>,    // text excerpts that differ
}
```

## Acceptance criteria

1. Every save creates a content-addressed snapshot (deduplicated by hash).
2. The version list shows timestamps and optional names.
3. Restoring a version replaces the deck (and is undoable).
4. Naming a version persists the label.
5. The diff shows structural changes between two versions.
6. Version data is stored locally only — no network.
7. Quality gate green. Privacy gate passes. No telemetry.
