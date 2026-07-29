# 900Slides

A free, local-first desktop presentation editor. An open-source alternative to
PowerPoint and Google Slides that runs entirely on your own computer — no
account, no subscription, no telemetry, no constant internet connection, and
your `.pptx` files are preserved byte-for-byte.

900Slides is built for the people and communities priced out of subscription
productivity software: classrooms, small businesses, researchers, public
services, and community organizers who need reliable presentation tooling on an
ordinary laptop, including when the Wi-Fi is down.

## Download

Pre-built binaries are produced by GitHub Actions for every tagged release.
Grab the latest from the [Releases page](../../releases) — see
[`docs/RELEASES.md`](docs/RELEASES.md) for full instructions.

| Build | Artifact |
| ----- | -------- |
| **macOS** | `900Slides-macos` — an ad-hoc signed `.app` bundle |
| **Linux** | `900Slides-linux-deb` (`.deb`) and `900Slides-linux-appimage` (`.AppImage`) |

The macOS build is **ad-hoc signed, not notarized** (no paid Apple Developer
account is used). On first launch, Gatekeeper will block it — right-click the
`.app`, choose **Open**, then **Open anyway** (or run
`xattr -cr /path/to/900Slides.app`). Windows and other platforms can build
from source (see below).

## Why it exists

Presentation software is basic working infrastructure — for teaching, running
a small business, organizing a community, or publishing research. Where the
mainstream tools demand subscriptions, accounts, cloud sync, and always-on
connectivity, 900 Labs builds open-source tools that stay useful on a laptop
with intermittent access and an aging OS. 900Slides is the presentation sibling
in that family, and it is built to remain that way.

## Features

**Slide editing**

- **Rich text** in every text box: bold, italic, underline, strikethrough,
  super/subscript, inline code, links, headings (H1–H6), blockquotes, fenced
  code blocks with stepped line-range highlighting (`1-3|4|5,7`), and indent
  levels.
- **Images and shapes**: PNG, JPEG, GIF, WebP, and SVG, plus geometric shapes
  (rectangle, rounded rectangle, ellipse, triangle, line, arrow, right-arrow
  callout, five-point star) with fill, outline, shadow, rotation, and crop.
- **Tables** up to 50 rows × 50 columns with cell text, fills, borders, column
  and row resize, header rows, and cell alignment.
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
- **ODP import and export** (OpenDocument Presentation) — open `.odp` files and
  export decks to `.odp`.
- **Deterministic export** to **SVG** (per slide), **PNG** (2× retina, per
  slide), and **PDF** (entire deck) — fully offline, identical inputs yield
  byte-identical output.
- **Bundled fonts** (Inter, Source Serif 4, JetBrains Mono) are embedded on
  export so files render identically on any platform.

**Private & resilient**

- **No account, no telemetry, no network calls** — the application is fully
  offline.
- **Image sanitization on ingest**: EXIF and other metadata are stripped,
  only allowlisted formats are accepted, and embedded SVG is scrubbed of
  scripts, event handlers, and unsafe URL references.
- **Offline spell-check** (en-US) with red squiggles, right-click suggestions,
  and a personal dictionary that persists across sessions.
- **Crash recovery** via debounced autosave snapshots and a startup recovery
  prompt.

## Platform support

| Platform | v0.3.0 status |
| -------- | ------------- |
| macOS    | Downloadable `.app` (ad-hoc signed) via GitHub Actions |
| Linux    | Downloadable `.deb` and `.AppImage` via GitHub Actions |
| Windows  | Source build only |

Source builds may also work on any Tauri-supported system once its
prerequisites are installed.

## How to use it

1. Choose **New** to start from one of the six built-in templates, or **Open**
   to load an existing `.pptx` or `.odp` file.
2. Click a text box on the canvas and start typing. Edits flow to the Rust
   backend on each change and the canvas re-renders from the returned
   snapshot; undo with the toolbar button.
3. Insert an image from the toolbar (sanitized on the way in) or add a
   geometric shape from the shape menu. Images and shapes render on the canvas
   and can be moved and restyled.
4. Insert a table from the toolbar and click cells to edit, or insert a chart
   and double-click it to edit its data. Content 900Slides cannot yet edit
   (e.g. SmartArt) appears as a labelled placeholder — preserved on save but
   not modifiable.
5. Choose **Save** to write a `.pptx` (or export to `.odp`, SVG, PNG, or PDF).
   Only edited slides are regenerated; every other part of the original file is
   unchanged.
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

900Slides is at v0.3.0. The following are planned for later releases
([`docs/ROADMAP.md`](docs/ROADMAP.md)) and are not yet available:

- **Local version history** (content-addressed snapshots, named versions,
  visual diffs) — v0.4.0.
- **Local comments** anchored to slides, objects, and text ranges — v0.4.0.
- **Accessibility checker** and WCAG 2.2 AA measurement — v0.4.0+.
- **Collaboration and cloud sync** — out of scope until v0.5.0+ and always
  self-hosted / local-first, never a required service.
- **Mobile and web clients.**
- **AI-assisted deck generation** — deferred until on-device models meet the
  project's quality bar; the architecture leaves room for it.
- A **notarized** macOS installer and **Windows/Linux installers** — held for
  v1.0.

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

### Validate the source

After installing the Tauri prerequisites for your platform, run the source
checks:

```bash
./scripts/verify-local.sh
```

This runs formatting, clippy, workspace tests, and the frontend type-check.
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
