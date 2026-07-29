# Wave 17 — v0.4.0 local comments

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.4.0 ("Local comments. Anchored to
slides, objects, and text ranges. Reply, resolve, assignment. Stored in
the custom XML part under `/customXml/` and preserved across round-trip.")
Last updated: 2026-07-29

Wave 17 adds **local comments**: threaded annotations anchored to slides,
specific shapes, or text ranges within a text box. Comments support reply,
resolve, and assignment. They are stored in the slides-core model and
persisted to the PPTX custom XML part (alongside the existing 900Slides
manifest), surviving byte-for-byte round-trip.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | `Comment`, `CommentThread`, commands |
| 2 | PPTX | `crates/slides-pptx/` | Comment persistence in custom XML |
| 3 | Desktop | `apps/desktop/` | Comment UI: anchors, panel, reply/resolve |

## The shared contract — model changes (component 1)

### Comment and thread types

```rust
/// A single comment in a thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    /// Unique id (UUID without hyphens).
    pub id: String,
    /// Author name (free text; no account system).
    pub author: String,
    /// Comment body (plain text).
    pub body: String,
    /// ISO 8601 timestamp (UTC).
    pub timestamp: String,
    /// Whether this specific comment is marked resolved (only applies to
    /// the thread root; replies inherit the thread's resolved state).
    #[serde(default)]
    pub resolved: bool,
}

/// A comment thread anchored to a target within the deck.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentThread {
    /// Unique id (UUID without hyphens).
    pub id: String,
    /// Where this thread is anchored.
    pub anchor: CommentAnchor,
    /// The root comment + replies in chronological order.
    pub comments: Vec<Comment>,
    /// Optional assignee (free text name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    /// Whether the entire thread is resolved.
    #[serde(default)]
    pub resolved: bool,
}

/// Where a comment thread is anchored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommentAnchor {
    /// Anchored to a whole slide.
    Slide {
        slide_id: String,
    },
    /// Anchored to a specific shape (by id).
    Shape {
        slide_id: String,
        shape_id: String,
    },
    /// Anchored to a text range within a specific text box.
    TextRange {
        slide_id: String,
        shape_id: String,
        /// Byte offsets into the text box's concatenated text.
        start: usize,
        end: usize,
    },
}
```

### New Deck field (additive)

```rust
// On Deck:
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub comments: Vec<CommentThread>,
```

Old decks deserialize with empty comments (no change in behavior).

### New commands

- `AddComment { anchor: CommentAnchor, author: String, body: String }` —
  creates a new thread with the root comment. Inverse: remove the thread.
- `ReplyToComment { thread_id: String, author: String, body: String }` —
  appends a reply. Inverse: remove the last reply.
- `SetCommentResolved { thread_id: String, resolved: bool }` — toggles
  resolved state. Inverse snapshots prior bool.
- `AssignComment { thread_id: String, assignee: Option<String> }` — sets
  or clears the assignee. Inverse snapshots prior.
- `DeleteCommentThread { thread_id: String }` — removes a thread.
  Inverse: re-insert (snapshots the removed thread).

### Validation

- `slide_id` must exist on the deck.
- For `Shape`/`TextRange` anchors, `shape_id` must reference a shape on
  that slide (by id).
- `TextRange.start <= end` must hold.
- Thread/comment ids must be unique.

## Component 2 — PPTX persistence (`slides-pptx`)

Comments are stored in the existing 900Slides custom XML part
(`customXml/item1.xml`). The saver serializes `deck.comments` into the
manifest XML alongside existing 900Slides metadata. The loader reads them
back on open.

This means comments survive PPTX round-trip with other tools: PowerPoint
ignores the custom XML part, and 900Slides reads them back unchanged.

- Extend `write_manifest` (in save.rs) to include a `<comments>` section
  containing serialized threads.
- Extend the manifest loader (in session.rs/load.rs) to parse the
  `<comments>` section back into `deck.comments`.
- The byte-for-byte guarantee holds for untouched decks (comments are part
  of the custom XML part, which is regenerated only when dirty).

## Component 3 — Desktop (`apps/desktop/`)

- A **comments sidebar** (toggle with a button or `C` key) listing all
  threads, grouped by slide. Each thread shows the anchor context (slide
  number, shape label, or text excerpt), the author, body, and replies.
- **Click a shape** → "Add comment" in context menu → creates a Shape-
  anchored thread.
- **Right-click selected text** → "Comment on selection" → creates a
  TextRange-anchored thread.
- **Reply** input at the bottom of each thread.
- **Resolve** toggle (checkbox or button).
- **Assign** input (optional).
- **Delete** button per thread.
- Resolved threads are collapsed/dimmed by default.

## Dependency ordering

1. **Model** (component 1) — types + commands. Single worktree.
2. **PPTX** (component 2) — manifest persistence. After model merges.
3. **Desktop** (component 3) — UI. After model + pptx merge.

## Acceptance criteria

1. Adding a comment creates a thread anchored to a slide/shape/text range.
2. Reply, resolve, assign, and delete all work and are reversible via undo.
3. Comments persist in the custom XML part and survive PPTX round-trip.
4. Old decks (no comments) deserialize unchanged.
5. Quality gate green. Privacy gate passes. No telemetry.
