# Wave 9 — v0.2.0 templates and slide masters

Status: Proposed
Owner: 900 Labs
Scope target: `PRODUCT_SPEC.md` §5.2 ("Five more templates ... with
per-template themes, masters, and layout variants; slide masters and layout
variants per template") and §6.2 (deck model tree).
Last updated: 2026-07-29

Wave 9 adds the **template system**: five new templates (Educator, Pitch,
Conference Talk, Community Update, Photo Essay), each with a distinct theme,
a slide master, and named layouts. This is the largest structural addition
in v0.2.0 — it introduces the `Master`, `Layout`, and `TemplateRegistry`
types that §6.2 calls for, all additively.

## Design decision: minimal master/layout model

The full §6.2 tree (`Master` with placeholder geometry, `Layout` with
overrides, `Slide.layout_ref`) is architecturally correct but heavy. For
v0.2.0 we introduce a **pragmatic subset**:

- `Master` carries background layers (shapes painted behind every slide) and
  placeholder definitions (frames for title/content).
- `Layout` is a named variant with placeholder overrides + background
  overrides.
- `Slide` gains `layout_ref: Option<String>` (the layout name).
- `Deck` gains `template: Option<String>` (the template name) and
  `layouts: Vec<Layout>` (the deck's available layouts, derived from its
  template).
- A `TemplateRegistry` in `slides-core` provides the 6 built-in templates
  (Default + 5 new), each defining a `Theme`, a `Master`, and a `Vec<Layout>`.

This is additive (`#[serde(default)]` everywhere). Old decks have
`template: None` and `layouts: []` — they render identically to before.
`SCHEMA_VERSION` stays `1`.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | `Master`, `Layout`, `TemplateRegistry`, template field, command |
| 2 | Renderer | `crates/slides-render/src/lib.rs` | Paint master background layers + placeholder frames |
| 3 | Desktop | `apps/desktop/` | Template picker on new-deck, layout picker per slide |

## Explicitly out of this wave

- User-created custom templates (the 6 built-ins only).
- Template import/export (a brand-kit or `.pptx` template file).
- Master-slide editing (masters are built-in, not user-editable in v0.2.0).
- Placeholder auto-fitting (content overflow is the editor's problem).
- Per-template speaker-note layouts.

## The shared contract — model changes (component 1)

### New types

```rust
/// A slide master: background layers painted behind every slide in the deck,
/// plus placeholder definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Master {
    /// Background shapes (painted first, behind all slide content).
    #[serde(default)]
    pub background_shapes: Vec<BackgroundShape>,
    /// Named placeholders that layouts reference.
    #[serde(default)]
    pub placeholders: Vec<PlaceholderDef>,
}

/// A simple background shape on a master (rectangles, accents).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackgroundShape {
    /// The geometric shape data.
    pub geometry: Geometry,
    pub style: Style,
    pub transform: Transform,
}

/// A named placeholder (e.g. "title", "content", "footer").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceholderDef {
    pub name: String,
    pub frame: Rect,
}

/// A named layout variant of the master.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Layout name (e.g. "Title Slide", "Title and Content", "Section Header").
    pub name: String,
    /// Placeholder overrides keyed by placeholder name.
    #[serde(default)]
    pub placeholders: Vec<PlaceholderDef>,
}

impl Master {
    pub fn default_16_9() -> Self;  // blank master
}

impl Default for Master { ... }  // empty
impl Default for Layout { ... }  // from name
```

### New Deck fields (additive)

```rust
// On Deck:
#[serde(default)]
pub template: Option<String>,       // "default", "educator", etc.
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub layouts: Vec<Layout>,           // the deck's available layouts
#[serde(default)]
pub master: Master,                 // the deck's master
```

### New Slide field (additive)

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub layout_ref: Option<String>,     // layout name this slide uses
```

### New command

- `SetTemplate { template_name: String }` — applies a built-in template's
  theme, master, and layouts to the deck. Inverse snapshots prior (theme,
  master, layouts, template). Validate: template_name must be one of the 6
  built-ins.

### TemplateRegistry

```rust
/// The six built-in templates, each with a theme, master, and layouts.
pub struct TemplateRegistry;

impl TemplateRegistry {
    pub fn names() -> Vec<&'static str>;  // ["default", "educator", "pitch", ...]
    pub fn get(name: &str) -> Option<TemplateDefinition>;
}

pub struct TemplateDefinition {
    pub name: &'static str,
    pub display_name: &'static str,
    pub theme: Theme,
    pub master: Master,
    pub layouts: Vec<Layout>,
}
```

### The six templates

Each template has a distinct theme (colors, fonts) and 3-4 layouts:

1. **Default** (existing): Calibri, blue accent, white background. Layouts:
   Title Slide, Title and Content, Blank.
2. **Educator**: serif headings (Georgia), warm palette (cream background,
   navy text, orange accent). Layouts: Lesson Title, Bulleted Content,
   Definition (two-column), Exercise.
3. **Pitch**: sans-serif (Inter/Helvetica), dark background (#1a1a2e),
   white text, teal accent. Layouts: Cover, Problem/Solution, Metrics,
   Team, Closing.
4. **Conference Talk**: monospace headings (JetBrains Mono/Consolas), dark
   background (#0d1117), green accent. Layouts: Title, Code Block, Big Idea,
   Q&A.
5. **Community Update**: friendly sans (Verdana), light green background,
   dark green accent. Layouts: Welcome, Announcements, Events, Call to Action.
6. **Photo Essay**: clean serif (Georgia) + sans body, black background,
   white text, no accent (photos are the focus). Layouts: Cover, Full Photo,
   Captioned Photo, Gallery.

## Component 2 — Renderer (`slides-render`)

When rendering a slide:
1. Paint the master's `background_shapes` first (behind all slide content).
2. If the slide has a `layout_ref`, paint that layout's placeholder frames
   as light dashed guides (editor-only, NOT in presenter or export).
3. Existing slide content renders on top as before.

The master background is painted before the theme background rect — or
after, depending on design. Simplest: paint theme background, then master
background shapes, then slide content. Add a test asserting master
background shapes appear in the output SVG.

## Component 3 — Desktop (`apps/desktop/`)

- **Template picker**: when creating a new deck, show a grid of 6 templates
  with name + theme preview. Selecting one calls `set_template`, which
  applies the theme/master/layouts and creates a starter slide using the
  first layout.
- **Layout picker**: in the slide thumbnail context menu or a side panel,
  a dropdown of the deck's layouts. Selecting one calls a `set_slide_layout`
  command (or just sets `layout_ref` via a small command) and re-renders
  with the placeholder guides.
- **New deck flow**: `new_deck` gains an optional `template_name` parameter.
  Default falls back to "default".

## New commands

- `SetTemplate { template_name: String }` — applies a built-in template.
  Inverse snapshots prior theme, master, layouts, and template name.
- `SetSlideLayout { slide_id, layout_name: Option<String> }` — sets or
  clears a slide's layout_ref. Inverse snapshots prior.

## Dependency ordering

1. **Model** (component 1) — single worktree, merged first.
2. **Parallel:** Renderer (component 2) + Desktop (component 3).

## Acceptance criteria

1. Creating a deck with the "Pitch" template produces a dark-themed deck
   with the Pitch theme, master, and layouts.
2. The renderer paints master background shapes behind slide content.
3. The layout picker shows the deck's layouts; selecting one updates the
   slide's layout_ref.
4. Old decks (template: None, layouts: [], master: empty) render identically
   to before — no visual change.
5. Every new command round-trips through undo correctly.
6. Quality gate green. Privacy gate passes. No telemetry.
