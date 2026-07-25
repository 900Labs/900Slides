# Wave 1 — v0.2.0 editor foundation

Status: Proposed
Owner: 900 Labs
Scope target: `PRODUCT_SPEC.md` §5.2 (images + geometric shapes subset)
Last updated: 2026-07-25

This wave makes **images and geometric shapes first-class** across the whole
stack: model, media ingest, renderer, PPTX load/save, and edit commands. It is
the foundation that later waves (tables, charts, animations, templates,
dual-display presenter, spell-check) build on. It stages onto the v0.1.0 model
**without a schema break** (`PRODUCT_SPEC.md` §6.2, §5.2 opening line).

## What this wave delivers

Six components, mapped one-to-one to the cut-off plan ("the model, loader,
saver, renderer, commands, and the previously-stub `slides-media` crate"):

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | Extend `Shape` enum + add `Style`, `Geometry`, `Image` |
| 2 | Media | `crates/slides-media/` | New (was stub) |
| 3 | Renderer | `crates/slides-render/` | New (was stub) |
| 4 | Loader | `crates/slides-pptx/src/load.rs` | Extend |
| 5 | Saver | `crates/slides-pptx/src/save.rs` | Extend |
| 6 | Commands | `crates/slides-core` (commands) + `apps/desktop/src-tauri/src/commands.rs` | Extend |

## Explicitly out of this wave (later waves)

- Tables, charts (`slides-chart`, `slides-animation` stay stubs).
- Animations and transitions beyond the existing reserved fields.
- The five extra templates; 4:3 / 16:10 / custom ratios.
- Rich-text additions (strikethrough, super/subscript, links, code, tabs).
- Dual-display presenter, laser pointer, highlighter, black/white slide.
- Spell-check, shortcuts dialog, high-contrast theme.
- PNG/PDF/ODP export (renderer ships **SVG only** this wave; PNG export is v0.3.0).

## The shared contract — model changes (component 1, lands first)

Everything else depends on the new `slides-core` types. They must land first
and be reviewed before components 2–6 branch. Additive only; `SCHEMA_VERSION`
stays `1` (old decks must still deserialize).

Extend `crates/slides-core/src/lib.rs`:

- Add a transform type so shapes/images can rotate:

  ```rust
  /// Placement of a shape: bounding frame (EMU) plus rotation in degrees.
  #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
  pub struct Transform { pub frame: Rect, pub rotation: f64 }
  ```

- Add a geometry enum covering the §5.2 shape set:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum Geometry {
      Rectangle, RoundedRectangle { radius: f64 },
      Ellipse, Triangle, Line, Arrow, RightArrowCallout, Star5,
  }
  ```

- Add a style:

  ```rust
  #[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
  pub struct Style {
      pub fill: Option<Fill>,
      pub outline: Option<Outline>,
      pub shadow: Option<Shadow>,
  }
  pub enum Fill { Solid(Color) }                       // gradients later
  pub struct Outline { pub color: Color, pub width_emu: f64, pub dash: DashStyle }
  pub enum DashStyle { Solid, Dash, Dot, DashDot }
  pub struct Shadow { pub offset_x: f64, pub offset_y: f64, pub blur: f64,
                      pub color: Color, pub opacity: f64 } // opacity 0..=1
  ```

  (`Crop` is `{ left, top, right, bottom }` in fractions 0..=1.)

- Extend the `Shape` enum additively (keep `TextBox`, `Passthrough`):

  ```rust
  pub enum Shape {
      TextBox(TextBox),
      Image(ImageShape),
      Geometric(GeometricShape),
      Passthrough(PassthroughObject),   // unchanged
  }
  pub struct ImageShape {
      pub transform: Transform,
      pub media_ref: String,            // key into the media store
      pub crop: Option<Crop>,
  }
  pub struct GeometricShape {
      pub transform: Transform,
      pub geometry: Geometry,
      pub style: Style,
  }
  ```

- Add editing commands (each implements `Command` with a verified `inverse`,
  `affected_slide_ids`, `validate`, `serialized_size`):

  `AddShape`, `DeleteShape`, `MoveShape` (set transform), `SetShapeStyle`,
  `InsertImage`. `AddShape`/`DeleteShape` are inverses of each other; the others
  snapshot-and-restore like the existing `EditText`.

- Add a media store type on the deck or session side so images are referenced
  by `media_ref` rather than inlined into the model (keeps the model diffable
  and undo bounded). Recommended: `pub struct MediaStore(BTreeMap<String, MediaEntry>)`
  with `MediaEntry { mime, bytes, width, height }` carried on `Deck` as
  `pub media: MediaStore`.

- Keep the existing tests green and add model round-trip tests for the new
  variants. `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` must pass.

## Component specifications

### 2. Media — `crates/slides-media/`

Implement the stub. Per `PRODUCT_SPEC.md` §5.2 and §7.4 (format security).

- Public API:

  ```rust
  pub struct IngestOptions { pub preserve_exif: bool,
      pub max_bytes: usize, pub max_dim: u32 }
  pub struct IngestedImage { pub bytes: Vec<u8>, pub mime: &'static str,
      pub format: ImageFormat, pub width: u32, pub height: u32 }
  pub enum ImageFormat { Png, Jpeg, Gif, Webp, Svg }

  pub fn ingest(raw: &[u8], opts: &IngestOptions) -> Result<IngestedImage, Error>;
  ```

- MIME allowlist: `image/png`, `image/jpeg`, `image/gif` (first frame only),
  `image/webp`, `image/svg+xml`. Reject anything else.
- Size cap and max dimension enforced; oversized input is rejected (not silently
  downscaled) so the user controls quality.
- EXIF stripped from raster formats unless `preserve_exif` is true; re-encode to
  a clean byte stream.
- SVG is **sanitized**, not rendered: reject `<script>`, external references
  (`href`/`xlink:href` to `http(s)://`, `file://`), and event-handler attributes.
  This honours the no-remote-calls principle (`PRODUCT_SPEC.md` §4, §8).
- Add the `image` crate to `[workspace.dependencies]` for raster decode/encode.
  Keep the dependency surface minimal.
- Deterministic: the same input + options produce identical output bytes.

### 3. Renderer — `crates/slides-render/`

Implement the stub. SVG only this wave (`§7.3`; PNG export is v0.3.0). Used for
slide thumbnails and presenter previews (`PRODUCT_SPEC.md` §6.5, the "cached
previews in v0.2.0" note at line 510).

- Public API:

  ```rust
  pub struct RenderOptions { pub width_emu: f64, pub height_emu: f64 }
  pub struct RenderedSlide { pub svg: String, pub hash: u64 }

  pub fn render_slide(
      slide: &slides_core::Slide,
      theme: &slides_core::Theme,
      media: &slides_core::MediaStore,
      opts: &RenderOptions,
  ) -> RenderedSlide;
  ```

- Render rules (per §7.3 "SVG per slide"):
  - `TextBox` → `<text>` runs, run-level bold/italic/underline, paragraph list
    markers, theme fonts.
  - `Geometric` → `<rect>` / `<ellipse>` / `<path>` matching `Geometry`, with
    fill, stroke, `transform="rotate(...)"`, and `filter` shadow.
  - `Image` → `<image href="data:{mime};base64,...">` using `media_ref`, with
    crop and rotation.
  - `Passthrough` → a labelled bounding-box `<rect>` with the object's label
    (the renderer cannot interpret arbitrary OOXML).
- **Determinism (§6.5):** no `HashMap` iteration in output; deterministic
  attribute order; base64 over normalized bytes. Expose a stable `hash` of the
  SVG so CI can assert two runs are byte-identical.
- Theme background painted first; slide aspect ratio derived from options.

### 4. Loader — `crates/slides-pptx/src/load.rs`

Extend the existing text-box path. Currently everything non-text becomes
`Passthrough`; now map images and geometric shapes to editable variants.

- `p:pic` (picture): resolve `a:blip/@r:embed` through the slide rels to the
  media part; ingest via `slides_media` (EXIF strip, allowlist); read geometry
  from `p:spPr/xfrm` (`a:off`, `a:ext`, rotation) and crop from `a:srcRect`;
  emit an `ImageShape`. Add the normalized bytes to the `MediaStore` and set
  `media_ref`.
- `p:sp` with `p:spPr/a:prstGeom`: map the preset (`rect`, `roundRect`,
  `ellipse`, `triangle`, `line`, `arrow`, `wedgeRoundRectCallout`/`rightArrow`,
  `star5`) to `Geometry`. Read fill from `a:solidFill/srgbClr`, outline from
  `a:ln`, shadow from `a:effectLst/a:outerShdw`. Emit a `GeometricShape`.
- Unrecognized presets or fills stay `Passthrough` and add a per-slide
  `LossWarning` (do not drop silently).
- Text boxes continue to map to `TextBox` exactly as today.

### 5. Saver — `crates/slides-pptx/src/save.rs`

Extend the dirty-slide regeneration path. Unedited slides + all non-slide parts
stay byte-for-byte identical (the lossless-passthrough principle, §4.9).

- For each dirty slide, emit the full `<p:spTree>` from the model:
  - `ImageShape` → write a new media part if `media_ref` is new or changed, add
    a slide relationship + an entry in `[Content_Types].xml` if needed, and emit
    `p:pic`/`a:blip` referencing it. Preserve crop via `a:srcRect`.
  - `GeometricShape` → emit `p:sp` with `a:prstGeom`, `a:solidFill`, `a:ln`,
    `a:effectLst/a:outerShdw`, and `p:spPr/xfrm` (rotation, frame).
  - `TextBox` / `Passthrough` → unchanged from v0.1.0.
- New media parts must not collide with existing part names; allocate the next
  free `ppt/media/imageN.<ext>`.

### 6. Commands — desktop + slides-core

- The `Command` impls from component 1 are the reversible units. The Tauri
  command surface (`apps/desktop/src-tauri/src/commands.rs`) gains:
  - `insert_image(slide_id, bytes)` → ingests via `slides_media`, builds an
    `InsertImage` command, applies it, returns the updated `DeckSnapshot`.
  - `add_shape(slide_id, geometry, style)` → `AddShape`.
  - `update_shape_transform(slide_id, shape_index, transform)` → `MoveShape`.
  - `update_shape_style(slide_id, shape_index, style)` → `SetShapeStyle`.
  - `delete_shape(slide_id, shape_index)` → `DeleteShape`.
  - `render_slide_svg(slide_id)` → calls `slides_render::render_slide` and
    returns the SVG string + hash for the thumbnail/presenter preview.
- Reuse the existing `CommandBus`; every command returns an inverse and keeps
  undo bounded (§6.3). The `media_ref` of a deleted image is orphaned from the
  store on undo by storing the entry in the inverse — design `DeleteShape` so
  its inverse re-inserts both the shape and its media entry if it was the last
  reference.
- Media is part of the deck snapshot sent to the frontend so the canvas can
  display images; the editable text-box path in the canvas stays as-is. New
  shapes/images render in the canvas; their SVG preview is the renderer output.

## Frontend (Svelte) — companion to component 6

Minimal, scoped to show the new content:

- `SlideCanvas.svelte` renders `ImageShape` and `GeometricShape` (via the
  `render_slide_svg` preview or direct DOM) on top of the existing editable
  text boxes. Selection + move/resize is welcome but can be a follow-up; the
  wave must at least *display* new shapes and let the toolbar add an image and
  a basic shape.
- Thumbnail panel switches from placeholder boxes to the renderer's SVG.
- `lib/types.ts` gains the new shape variants mirroring the model.

## Dependency ordering (how to execute)

Components 2–6 all consume the new `slides-core` types, so the model must land
on the base branch first. Recommended sequencing:

1. **Model (component 1)** — single worktree, reviewed and merged first.
2. **Parallel fan-out on top of the merged model:**
   - `slides-media` (independent)
   - `slides-render` (needs model only)
   - `slides-pptx` loader + saver together (needs model + media)
   - desktop commands + frontend (needs model, media, render, pptx)

The desktop worktree depends on all the others being available, so it is the
final merge. Media and render can run concurrently once the model is in.

## Acceptance criteria

The wave is done when all of these hold:

1. A PPTX containing images and shapes opens into editable `ImageShape` /
   `GeometricShape` model objects (not passthrough), and saving produces a
   byte-for-byte-identical package for every *unmodified* slide and part, plus
   correctly regenerated parts for edited slides.
2. `slides-media::ingest` rejects disallowed MIME and oversized input, strips
   EXIF by default, and keeps GIF to its first frame; the SVG sanitizer rejects
   scripts and external references.
3. `slides_render::render_slide` produces deterministic SVG (stable hash) for
   text, images, shapes, and passthrough placeholders.
4. Every new command round-trips through undo correctly, including image media
   being restored on undo of a delete.
5. The editor canvas displays images and shapes; the thumbnail panel uses
   renderer SVG.
6. **Quality gate is green** in this order (`AGENTS.md`):
   `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
   `cargo test --workspace`; `npm run check --prefix apps/desktop`;
   `npm run tauri:dev --prefix apps/desktop` (smoke).
7. No telemetry, analytics, or remote calls introduced (`PRODUCT_SPEC.md` §4, §8).

## Test fixtures

- Add sanitized generated fixtures under `crates/slides-fixtures` (currently a
  stub) — small hand-built PPTX packages with one image and one of each
  geometry preset. No real-world decks, no EXIF, no local paths (`§5.1` privacy
  gate). A round-trip test asserts untouched parts are byte-identical.
- Renderer hash-gate test asserts determinism on a fixed slide.
