# 900Slides

A free, local-first desktop presentation editor. An open-source alternative to
PowerPoint and Google Slides that runs entirely on your own computer — no
account, no subscription, no telemetry, no constant internet connection, and
untouched parts of your `.pptx` files are preserved byte-for-byte.

900Slides is built for the people and communities priced out of subscription
productivity software: classrooms, small businesses, researchers, public
services, and community organizers who need reliable presentation tooling on an
ordinary laptop, including when the Wi-Fi is down.

## Download

The source is on the **0.4 development track**. The release workflow builds
platform artifacts on tagged releases and manual runs; check a successful run in
[GitHub Actions](https://github.com/900Labs/900Slides/actions/workflows/release.yml)
for downloads. A configured job is not evidence that an installer has been tested.
See [`docs/RELEASES.md`](docs/RELEASES.md) for installation and validation status.

| Platform | Configured artifact | Validation still required |
| --- | --- | --- |
| macOS | Ad-hoc signed `.app` | Target OS install and Gatekeeper flow; not notarized |
| Linux | `.deb` and `.AppImage`, built on Ubuntu 22.04 | Clean supported systems and offline dependencies |
| Windows | NSIS installer including the offline WebView2 installer | Clean offline Windows installation; not code signed |

The Windows offline bundle includes a browser runtime and is substantially larger
than the app executable. The 16 MiB executable regression budget is **not** an
installer-size or RAM claim. Supported older-device targets and outstanding tests
are tracked in [`docs/LOW_RESOURCE_REQUIREMENTS.md`](docs/LOW_RESOURCE_REQUIREMENTS.md).

## Why it exists

Presentation software is basic working infrastructure — for teaching, running
a small business, organizing a community, or publishing research. Many
established tools work offline, but licensing, account setup, runtime requirements,
and offline installation differ. 900 Labs builds open-source tools that stay
useful with intermittent access and modest hardware. 900Slides is the presentation
sibling in that family, and it is built to remain that way.

## Features

**Slide editing**

- **Slide organization**: add, move up/down and delete slides, with undo. The
  final slide is retained; reordering and deletion persist when saving PPTX.

- **Rich text** in every text box: bold, italic, underline, strikethrough,
  super/subscript, inline code, links, headings (H1–H6), blockquotes, fenced
  code blocks with stepped line-range highlighting (`1-3|4|5,7`), and indent
  levels.
- **Images and shapes**: PNG, JPEG, GIF, WebP, and SVG, plus geometric shapes
  (rectangle, rounded rectangle, ellipse, triangle, line, arrow, right-arrow
  callout, five-point star). Objects can be moved and resized; geometric shapes
  have fill, outline, opacity, and shadow controls. Imported image crops and
  rotations render, but dedicated crop/rotation controls remain open.
- **Tables** up to 50 rows × 50 columns with text editing and row/column
  insertion and deletion. Imported fills, borders, header rows, cell alignment,
  and dimensions render; dedicated cell-formatting and row/column resize
  controls remain open.
- **Charts** (bar, column, line, area, pie, scatter) with an in-place
  data-table editor for categories, series, and values. Switch the chart type
  at any time.

**Presenting**

- **Dual-display mode**: a presenter window (controls, notes, timer, next-slide
  preview) drives a separate fullscreen audience window, synchronized live.
- **Laser pointer** (`L`), **highlighter** (`H`), and **black/white Q&A slide**
  (`B` / `W`) — keyboard navigation throughout.
- **Projector filter panel** to compensate for difficult projectors: invert,
  brightness, contrast, saturation, sepia, and hue-rotate, persisted per deck.

**Animations & transitions**

- **Build-ins** (fade, slide-in from any edge, appear, disappear) with an
  ordered build-sequence editor, revealed step-by-step on each click.
- **Transitions** between slides: none, fade, slide, push, wipe.
- **Magic Move (morph)** automatically interpolates position, size, rotation,
  and opacity for shapes that share a stable id across adjacent slides.
- **Stepped code highlighting** advances through code-block ranges during a talk.

**Templates & structure**

- **Six built-in templates** (Default, Educator, Pitch, Conference Talk,
  Community Update, Photo Essay), each with its own theme, slide master, and
  named layouts.
- **Aspect ratios** 16:9, 4:3, and 16:10, with a toolbar picker.
- **Named, collapsible sections**, **rich-text speaker notes**, **find and
  replace** (Cmd/Ctrl+F), a **keyboard-shortcuts dialog** (`?`), and a
  **high-contrast theme** toggle.

**Formats & fidelity**

- **Lossless PPTX**: unedited parts of the package are carried as opaque
  objects and re-emitted unchanged; save regenerates only the slides you
  touched. A per-slide **loss ledger** warns you about content that is preserved
  but not yet editable (e.g. SmartArt).
- **ODP import and export library** (OpenDocument Presentation). The current
  desktop file chooser exposes PPTX; ODP UI integration remains open.
- **Deterministic export** to **SVG** (per slide), **PNG** (2× retina, per
  slide), and **PDF** (entire deck) — fully offline, identical inputs yield
  byte-identical output.
- **Bundled render/export fonts** (Inter, Source Serif 4, JetBrains Mono) reduce
  dependence on installed fonts. Editor substitution and additional script
  coverage still require qualification.

**Private & resilient**

- **No account, no telemetry, no network calls** — the application is fully
  offline.
- **Image sanitization on ingest**: EXIF and other metadata are stripped,
  only allowlisted formats are accepted, and embedded SVG is scrubbed of
  scripts, event handlers, and unsafe URL references.
- **Offline spell-check** (en-US) with red squiggles, right-click suggestions,
  and a personal dictionary that persists across sessions.
- **Safer file workflow**: Save reuses the current PPTX path; Save As creates
  a copy. New, Open, and Quit protect unsaved work with Save / Discard / Cancel.
- **Crash recovery**: text, table cells, and notes commit while you type;
  debounced recovery snapshots and a startup recovery prompt help recover work.
- **Bounded preview caches**: viewport-aware thumbnails release offscreen SVGs
  and use a byte budget. The active deck and its source media still require RAM.
- **Local history, comments, and document accessibility checks** are available.
  The accessibility score is a heuristic, not a WCAG conformance assessment.

## How to use it

1. Choose **New** to start from one of the six built-in templates, or **Open**
   to load an existing `.pptx` file.
2. Choose **Text Box** from the toolbar or Insert menu, then click the canvas
   to place one. To edit an existing text box, select it and press Enter or
   double-click it. Edits commit automatically while typing, with an explicit flush before
   saving or switching slides. Undo with the toolbar button.
3. Insert an image from the toolbar (sanitized on the way in), or choose
   **Shape** and a geometry from the picker. Click the canvas for a default
   shape, or click-drag to place it at explicit bounds. Images and shapes render
   on the canvas and can be moved and restyled.
4. Insert a table from the toolbar and click cells to edit, or insert a chart
   and double-click it to edit its data. Content 900Slides cannot yet edit
   (e.g. SmartArt) appears as a labelled placeholder — preserved on save but
   not modifiable.
5. Choose **Save** to write a `.pptx`, or use the Export menu for SVG, PNG,
   or PDF. For imported PPTX, save patches supported edited parts and preserves
   untouched package content; ODP conversion has a separate compatibility scope.
6. Choose **Present** to open dual-display mode: a presenter window and a
   fullscreen audience window. Navigate with arrow keys, space, Home, End, and
   Escape. Press `B` / `W` for a black/white Q&A slide, `L` for the laser
   pointer, and `H` for the highlighter.

### Recovery

After an edit, 900Slides waits for activity to settle, then writes a recovery
snapshot to the app data directory. Recovery files are separate from the file
you opened, and a normal save retires the corresponding snapshot. If recovery
snapshots are found at startup, the app lists them newest first — restore one,
discard one, or skip to a new deck.

## What it does not do (yet)

The next milestone is a dependable everyday editor on supported modest hardware.
These remain open acceptance gaps ([roadmap](docs/ROADMAP.md)):

- Verified 2 GB / 4 GB device performance and clean offline installation on Windows/Linux.
- Desktop ODP controls, slide duplication, full mixed-style text editing,
  and broader PowerPoint/LibreOffice interoperability coverage, master-layout fidelity,
  and clear font substitution reporting.
- Complete image alt-text editing, keyboard/screen-reader audits, localized UI,
  and RTL/CJK validation. The current UI and spell checker are English-first.
- A notarized macOS distribution and a signed Windows installer.
- Collaboration, mobile/web clients, and on-device AI features remain deferred.

## Build from source

Prerequisites:

- Rust 1.92.0, pinned by `rust-toolchain.toml`
- Node.js 20.19 or newer, 22.12 or newer, or 24 or newer
- The [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/)

```bash
git clone https://github.com/900Labs/900Slides.git
cd 900Slides
npm ci --prefix apps/desktop
npm run tauri:dev --prefix apps/desktop
```

To produce a release bundle:

```bash
npm run tauri:build --prefix apps/desktop
```

The standalone Tauri workspace owns the release profile used for distributable
builds: size optimization, link-time optimization, one codegen unit, stripped
symbols, and abort-on-panic. The size gate runs the full Tauri build so the
frontend is rebuilt from source, the distributable `custom-protocol` feature is
enabled, and the resulting target executable is checked against the maintained
16 MiB regression budget:

```bash
./scripts/verify-wave22.sh
```

Passing a binary path checks an already packaged executable without rebuilding.
No-argument mode requires installed npm dependencies plus the platform's Tauri
system prerequisites. Plain `cargo build --release` is a development-protocol
build and is not the representative release-size measurement.

### Validate the source

After installing the Tauri prerequisites for your platform, run the source
checks:

```bash
./scripts/verify-local.sh
```

This runs workspace and standalone desktop formatting, clippy and tests,
frontend type-check, unit tests, production frontend build, and the privacy gate.
Complete the native smoke test with `npm run tauri:dev --prefix apps/desktop`.
Run the public-release privacy gate before publishing or changing repository
visibility:

```bash
./scripts/verify-public-release.sh
```

## Repository layout

A Rust workspace of 11 library crates plus a Svelte 5 / Tauri v2 desktop app:

```
apps/desktop/             Desktop UI and Tauri command boundary
crates/slides-core/       Deck model, commands, undo, theme
crates/slides-pptx/       PPTX load and save (native format)
crates/slides-odp/        ODP (OpenDocument Presentation) import and export
crates/slides-pdf/        SVG, PNG, and PDF export
crates/slides-render/     Deterministic slide rendering to SVG
crates/slides-animation/  Deterministic build-in timeline and CSS playback
crates/slides-chart/      Chart data model and deterministic SVG previews
crates/slides-spell/      Offline en-US spell-check and suggestions
crates/slides-media/      Image ingest, EXIF strip, MIME allowlist, SVG sanitize
crates/slides-i18n/       Locale and accessibility helpers (stub)
crates/slides-fixtures/   Sanitized generated fixtures only (stub)
```

See the [Roadmap](docs/ROADMAP.md) for the full feature trajectory and the
[Competitive analysis](docs/COMPETITIVE_ANALYSIS.md) for the research that
informed it.

## Quality gate

Run the complete local gate before opening a pull request:

```bash
./scripts/verify-local.sh
```

The workspace test suite is **300+ Rust tests**, zero clippy warnings, and a
clean `svelte-check`. The Rust suite includes generated PPTX round-trip tests
that assert untouched parts are byte-identical after edits, determinism tests
for the renderer, sanitizer and allowlist tests for image ingest and link URLs,
and table/chart invariant and command round-trip tests.

## Contributing and support

- [Contributing guide](CONTRIBUTING.md)
- [Support guide](SUPPORT.md)
- [Security policy](SECURITY.md)

Bug reports, sanitized compatibility fixtures made with invented data,
translations, documentation improvements, and focused performance work are all
welcome.

## License

900Slides is licensed under the Apache License 2.0. See [LICENSE](LICENSE).
