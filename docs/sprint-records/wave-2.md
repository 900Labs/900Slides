# Wave 2 — v0.2.0 rich text

Status: Proposed
Owner: 900 Labs
Scope target: `PRODUCT_SPEC.md` §5.2 (rich-text additions)
Last updated: 2026-07-26

Wave 1 made images and geometric shapes first-class. **Wave 2 completes the
inline rich-text set** on top of the existing text-box editing, without touching
the format boundary's media or shape tree. It is the lowest-risk next step: it
extends `Run` and `Paragraph` (which already work end to end through
load/save/render/command) and unblocks Wave 6's templates (which need heading
styles).

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | Extend `Run`, `Paragraph`; add `Link`, `VerticalAlign`, `HeadingLevel`, `ParagraphStyle` |
| 2 | Renderer | `crates/slides-render/src/lib.rs` | Extend `push_run` + paragraph rendering |
| 3 | Loader | `crates/slides-pptx/src/load.rs` | Extend `parse_run` / `parse_text_box` |
| 4 | Saver | `crates/slides-pptx/src/save.rs` | Extend `write_paragraph` + run-property emitting |
| 5 | Commands | `crates/slides-core` (commands) + `apps/desktop/src-tauri/src/commands.rs` | Reuse existing `EditText`/`EditTextBox`; add a `SetRunStyle` convenience command |

## Explicitly out of this wave (later waves)

- Tables, charts, animations, transitions.
- New templates and masters; aspect ratios other than 16:9.
- Dual-display presenter, laser pointer, highlighter, black/white slide.
- Spell-check, shortcuts dialog, high-contrast theme.
- Smart typography (ligatures, kerning, OpenType features), text-on-path,
  vertical text, RTL/bidi (those are v1.0+).
- Heading *numbering schemes* beyond plain list markers (headings are styled,
  not auto-numbered, this wave).

## The shared contract — model changes (component 1, lands first)

Everything else consumes the new `slides-core` types, so they land first. All
additive; `SCHEMA_VERSION` stays `1` (old decks and recovery snapshots must
still deserialize). New fields are `#[serde(default)]` so v0.1.0/Wave 1 decks
load unchanged.

### Run — extend, add fields behind `#[serde(default)]`

```rust
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    // NEW (all default):
    #[serde(default)] pub strikethrough: bool,
    #[serde(default)] pub vertical_align: VerticalAlign,   // None|Superscript|Subscript
    #[serde(default)] pub link: Option<Link>,
    #[serde(default)] pub code: bool,        // inline code: monospace, boxed
    #[serde(default)] pub font_family: Option<String>, // override theme body font (used by code)
}

pub enum VerticalAlign { #[default] Baseline, Superscript, Subscript }

pub struct Link {
    pub url: String,            // absolute or relative; no scheme injection
    pub display: Option<String>, // optional override of run.text for the link label
}
```

Keep the existing `Run::new(...).bold()` builder; add `.strikethrough()`,
`.superscript()`, `.subscript()`, `.code()`, `.link(url)`, `.font(family)`.

### Paragraph — extend with a style block

```rust
pub struct Paragraph {
    pub runs: Vec<Run>,
    pub list_style: ListStyle,            // unchanged
    #[serde(default)] pub style: ParagraphStyle,
}

#[derive(Default)]
pub struct ParagraphStyle {
    #[serde(default)] pub heading: Option<HeadingLevel>,   // H1..H6
    #[serde(default)] pub blockquote: bool,
    #[serde(default)] pub code_block: bool,                // fenced code; monospace
    #[serde(default)] pub indent_level: u32,               // 0 = no indent; tabs map here
}

pub enum HeadingLevel { H1, H2, H3, H4, H5, H6 }
```

`indent_level` absorbs "tabs" from the spec: a tabbed paragraph is an indented
one. Code blocks are fenced (whole paragraph), distinguished from inline `code`
on a run.

### Link safety (do not defer this — it is part of the model)

`Link::url` validation belongs in the model constructor / a
`Link::new(url) -> Result<Link, LinkError>` so the boundary is at the type, not
the caller. Reject:
- `javascript:`, `vbscript:`, `data:`, `file:`, and any scheme not in an
  allowlist (`http`, `https`, `mailto`, `tel`, `#fragment`, relative path).
- Control characters.

Mirror the `slides_media::is_unsafe_url` posture (reject any colon-before-slash
and the dangerous scheme list). This keeps link following (added later in the
presenter/frontend) from becoming an XSS surface.

### New command

- `SetRunStyle { slide_id, shape_index, paragraph_index, run_index, bold: Option<bool>, italic: Option<bool>, underline: Option<bool>, strikethrough: Option<bool>, vertical_align: Option<VerticalAlign>, code: Option<bool> }` —
  a merge-patch command over a single run's style flags (only the `Some(_)`
  fields change). Inverse snapshots the prior run. This avoids round-tripping
  the whole paragraph for a single toolbar toggle (the existing `EditText`
  replaces all runs of a paragraph, which is fine for typing but coarse for a
  bold-toggle hotkey).

`EditText` and `EditTextBox` stay as-is; they already carry `Run`s and
`Paragraph`s, so they pick up the new fields automatically.

## Component specifications

### 2. Renderer (`slides-render`)

Extend `push_run` and paragraph layout (`render_text_box`):
- **Strikethrough** → `text-decoration="line-through"` (combine with underline
  as a space-joined `text-decoration`).
- **Superscript / subscript** → wrap the run in `<tspan baseline-shift="super|sub" font-size="...">` (super = ~65% size, baseline-shift ~0.5em; sub = negative). Document the chosen ratios.
- **Link** → render as a `<tspan>` with a distinct color (theme accent) and
  underline; wrap the visible text in an `<a href="...">` element so it is a
  real SVG link. The href is escaped.
- **Inline code** → `<tspan font-family="monospace">` with a subtle background
  (`<rect>` behind the run is acceptable; or a fill on the tspan). Use the
  theme's body font as fallback for the rect width estimate.
- **Headings** → larger `font-size` per level (H1 ~2x body, H2 ~1.5x, ... down
  to H6 ~1.1x) and bold. Use `theme.heading_font` for headings.
- **Blockquote** → left indent + a vertical bar (a thin `<rect>`) + italic body.
- **Code block** → monospace font, a background `<rect>` covering the
  paragraph's line region, and preserve the runs' text verbatim (no rich marks
  inside a code block).
- **Indent level** → `x = frame.x + padding + indent_level * indent_step`.
- **Determinism:** all of the above is deterministic; the existing hash must
  stay stable for unchanged input. Keep attribute order fixed.

### 3. Loader (`slides-pptx/src/load.rs`)

Map OOXML run/paragraph properties onto the new model fields:
- `a:rPr` attributes: `strike="sngStrike"` → `strikethrough`; `baseline` (`>0`
  superscript, `<0` subscript, in thousandths of a percent) → `VerticalAlign`;
  `i="1"`/`b="1"` already handled.
- `a:hlinkClick r:id="..."` → resolve the relationship to a target URL → `Link`
  (validated at construction; an unresolvable or disallowed link is preserved
  as plain text + a loss warning, not dropped silently).
- `a:rPr` child `a:latin typeface="..."` → `font_family`.
- Paragraph: `a:pPr pStyle="..."` → map known heading style names
  (`Title`/`Heading1..6`) to `HeadingLevel`; `lvl` → `indent_level`. Code-block
  detection is heuristic (monospace font + no marks) — out of scope for load
  fidelity this wave; a monospace-styled paragraph loads as `code` runs at
  best, otherwise stays as a styled paragraph. Blockquote is not a native
  OOXML concept; it is not read on load.

### 4. Saver (`slides-pptx/src/save.rs`)

Extend `write_paragraph` and the run-property emitter:
- Run: emit `<a:rPr>` with `b`, `i`, `u`, `strike`, `baseline` (super/sub in
  OOXML percentage units), `lang`. For a link, emit `<a:hlinkClick r:id="...">`
  (add a hyperlink relationship to the slide rels, allocating the next rId).
  For code, set `a:latin typeface="...monospace..."`.
- Paragraph: `a:pPr` with `pStyle` for headings (`Heading1..6`), `lvl` for
  indent. Code block → set the paragraph font to monospace. Blockquote → emit
  as an indented paragraph (no OOXML equivalent; preserved via indent + the
  custom part if needed later).
- The patch path (`patch_slide_xml`) already rewrites the whole `<p:txBody>`,
  so adding new `a:rPr` attributes does not require structural changes — only
  the run-XML builder changes.
- **Lossless guarantee (§4.9) unchanged:** unedited slides are still copied
  byte-for-byte. New run properties appear only on edited (dirty) slides.

### 5. Commands (desktop)

- The desktop `edit_text` command already sends replacement runs; it picks up
  the new fields for free.
- Add `set_run_style` Tauri command wrapping the new `SetRunStyle` model
  command (toolbar toggles for bold/italic/underline/strikethrough/code, super/
  sub).
- Frontend toolbar: extend the existing text-formatting row with the new
  toggles. Headings are a paragraph-level dropdown on the format row. Blockquote
  and code-block are paragraph toggles. These are additive to the existing
  editable text box interaction.

## Dependency ordering (how to execute)

Components 2–5 consume the new `slides-core` types, so the model lands first
on the base branch:

1. **Model (component 1)** — single worktree, reviewed and merged first.
2. **Parallel fan-out on the merged model:**
   - `slides-render` (needs model only)
   - `slides-pptx` loader + saver together (needs model; relationship target
     resolution for links reuses existing rels infra)
   - desktop commands + frontend (needs model + render + pptx) — final merge

## Acceptance criteria

The wave is done when all of these hold:

1. A PPTX containing a rich-text run (strikethrough, superscript, link, inline
   code) loads into the new model fields, and saving produces a
   byte-for-byte-identical package for every *unmodified* slide plus correctly
   regenerated parts for edited slides. A round-trip of an edited rich run
   preserves every property.
2. `Link::new` rejects `javascript:`/`vbscript:`/`data:`/`file:` and any
   non-allowlisted scheme; the link is validated at construction.
3. `slides_render::render_slide` produces deterministic SVG (stable hash) for
   every new property; headings/indent/links render correctly.
4. `SetRunStyle` round-trips through undo; merging only the toggled property.
5. The editor toolbar exposes the new toggles; headings are a dropdown.
6. **Quality gate green** (`AGENTS.md` order): `cargo fmt`; `cargo clippy
   --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `npm
   run check --prefix apps/desktop`; `npm run tauri:dev` smoke.
7. No telemetry, analytics, or remote calls introduced (§4, §8).
8. `scripts/verify-public-release.sh` passes (no local paths / secrets / new
   network deps).

## Test fixtures

- Add sanitized generated PPTX fixtures under `crates/slides-fixtures` with one
  run of each new property (hand-built OOXML, no real-world decks, no EXIF, no
  local paths). A round-trip test asserts untouched parts are byte-identical.
- Renderer hash-gate test asserts determinism for a paragraph with each new
  property.
- `Link::new` unit tests for the scheme allowlist.

## Notes

- **Link following** (clicking a link to open the browser) is a frontend/presenter
  feature, not a model feature. The model only stores a validated URL. Wave 2
  stores + renders + round-trips links; actually opening them in the OS browser
  is a follow-up (Tauri opener, behind the security model).
- **Headings as styles vs. semantic levels:** this wave models heading as a
  semantic `HeadingLevel` on the paragraph, not as a run-style. This keeps it
  diffable and lets Wave 6's templates define per-level appearance.
