# 900Slides

900Slides is a free, local-first desktop presentation editor. It is built for
people and communities who need to build, present, and save slides without an
account, subscription, telemetry, or constant internet connection.

## Why it exists

900 Labs builds open-source tools for people and communities priced out of
modern software. Presentation tools are basic working infrastructure for
schools, small businesses, researchers, public services, and community
organizations. 900Slides is designed to remain useful on an ordinary computer
and when an internet connection is unavailable.

## Current release

Version 0.1.0 is a **public source release** intended for developers and early
testers who are comfortable building from source. It is not a signed binary
installer release. It is a thin vertical slice that proves the architecture —
the deck model, lossless PPTX editing, a basic editor, and recovery.

What v0.1.0 does:

- Opens `.pptx` files and edits **text boxes**: paragraphs, bold, italic,
  underline, strikethrough, super/subscript, inline code, links, headings
  (H1-H6), blockquotes, fenced code blocks, and indent levels.
- Edits **images and geometric shapes** (rectangle, rounded rectangle,
  ellipse, triangle, line, arrow, right-arrow callout, five-point star) with
  fill, outline, shadow, rotation, and crop. Insert an image through the
  toolbar or a shape through the shape menu.
- Edits **tables** up to 50 rows × 50 columns with cell text, fills, borders,
  column/row resize, header rows, and cell alignment. Insert a table through
  the toolbar, click cells to edit, and add or remove rows and columns.
- Sanitizes every image on the way in: EXIF and other metadata are stripped by
  default, only allowlisted image formats are accepted, and embedded SVG is
  scrubbed of scripts, event handlers, and unsafe URL references.
- Preserves everything else in the PPTX package **byte-for-byte**. Charts,
  tables, SmartArt, and any other content 900Slides does not yet edit are
  carried as opaque objects and re-emitted unchanged on save. Save regenerates
  only the slides you actually edited.
- Shows per-slide warnings when a file contains content that is preserved but
  not editable (the loss ledger).
- Renders a slide to deterministic SVG for thumbnails and presenter previews.
- Presents a deck locally: a fullscreen presenter window with the current
  slide, a next-slide preview, a slide counter, an elapsed timer, and speaker
  notes. Keyboard navigation only.
- Recovers work after a crash or accidental quit via debounced autosave
  snapshots and a startup recovery prompt.
- Undoes every edit through a transactional command bus with bounded history.

What v0.1.0 does **not** do yet:

- Edit charts (they are preserved on save but shown as non-editable
  placeholders in the editor).
- Animate builds or transitions.
- Export to PDF, PNG, SVG, or ODP.
- Spell-check.
- Support aspect ratios other than 16:9.
- Ship a signed or notarized installer on any platform.

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

### Validate the source

After installing the Tauri prerequisites for the host platform, run the source
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

## How to use it

1. Choose **New** for a blank deck with one editable text box, or **Open** to
   load an existing `.pptx` file.
2. Click a text box on the slide canvas and type.
3. Edits are sent to the Rust backend on each change; the canvas re-renders
   from the returned snapshot. Undo with the toolbar button.
4. Insert an image with the toolbar button (it is sanitized on the way in), or
   add a geometric shape from the shape menu. Images and shapes render on the
   canvas and can be moved and restyled.
5. Content 900Slides cannot edit (charts, tables, SmartArt) appears as a
   labelled placeholder. It is preserved on save but cannot be modified.
6. Choose **Save** to write a `.pptx` file. Only edited slides are
   regenerated; every other part of the original file is unchanged.
7. Choose **Present** to open a fullscreen presenter window. Use arrow keys,
   space, Home, End, and Escape to navigate.

### Recovery

After a successful edit, 900Slides waits 750 milliseconds for activity to
settle and writes a recovery snapshot to the app data directory. Recovery
files are separate from the file you opened. A normal save retires the
corresponding recovery snapshot.

If recovery snapshots are found at startup, the app lists them newest first.
Restore one, discard one, or skip to a new deck. Restoring does not delete
the others.

## Platform support

| Platform | v0.1.0 status                                                  |
| -------- | -------------------------------------------------------------- |
| macOS    | Source builds. No notarized or signed package in v0.1.0.       |
| Windows  | Source builds. CI compiles the workspace and desktop target.   |
| Linux    | Source builds. CI compiles the workspace and desktop target.   |

Source builds may work on Tauri-supported systems once their platform
prerequisites are installed. No installer is published for any platform in
this release.

## Development

The repository is a Rust workspace with a Svelte 5 and Tauri v2 desktop
application:

```
apps/desktop/             Desktop UI and Tauri command boundary
crates/slides-core/       Deck model, commands, undo, theme
crates/slides-pptx/       PPTX load and save (native format)
crates/slides-odp/        ODP import / export conversion boundary (stub)
crates/slides-pdf/        PDF export and image-per-page import (stub)
crates/slides-render/     Deterministic slide rendering to SVG
crates/slides-animation/  Deterministic build and transition playback (stub)
crates/slides-chart/      Chart data and SVG previews (stub)
crates/slides-spell/      Spell-check dictionary boundary (stub)
crates/slides-media/      Image ingest, EXIF strip, MIME allowlist, SVG sanitize
crates/slides-i18n/       Locale and accessibility helpers (stub)
crates/slides-fixtures/   Sanitized generated fixtures only (stub)
```

See [Architecture](docs/ROADMAP.md) for the full feature trajectory and
[Competitive analysis](docs/COMPETITIVE_ANALYSIS.md) for the research that
informed the roadmap.

## Quality gate

Run the complete local gate before opening a pull request:

```bash
./scripts/verify-local.sh
```

The workspace test suite is 152 Rust tests, zero clippy warnings, and a clean
svelte-check. The Rust suite includes generated PPTX round-trip tests that
assert untouched parts are byte-identical after edits, determinism tests
for the renderer, sanitizer/allowlist tests for image ingest and link
URLs, and table invariant + command round-trip tests.

## Contributing and support

- [Contributing guide](CONTRIBUTING.md)
- [Support guide](SUPPORT.md)
- [Security policy](SECURITY.md)

Bug reports, sanitized compatibility fixtures made with invented data,
translations, documentation improvements, and focused performance work are
welcome.

## License

900Slides is licensed under the Apache License 2.0. See [LICENSE](LICENSE).
