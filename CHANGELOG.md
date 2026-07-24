# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-07-24

### Added

- Deck model in `slides-core`: Deck, Slide, Shape (TextBox | Passthrough),
  Theme, Color, Rect (EMU geometry), Paragraph, Run, ListStyle.
- Transactional command bus with bounded undo (100 transactions, 64 MiB
  aggregate, 32 MiB per transaction). Each command produces a verified
  inverse.
- `EditText` command with correct inverse and dirty-slide tracking.
- PPTX load boundary in `slides-pptx`: parses `[Content_Types].xml`,
  relationships, `presentation.xml`, slide XML, and theme. Maps text boxes to
  editable shapes; everything else to opaque passthrough objects.
- PPTX save boundary: regenerates only edited slides; copies all other parts
  byte-for-byte from the original package. Round-trip test asserts
  byte-identity of untouched parts.
- Loss ledger: per-slide warnings for content that is preserved but not
  editable.
- 900Slides custom XML manifest under `/customXml/` with content-type and
  relationship wiring so PowerPoint opens authored decks without repair
  warnings.
- Tauri v2 desktop shell with 14 commands: new, open, save, snapshot, edit
  text, undo, loss ledger, presenter start/state/next/previous, and recovery
  list/restore/discard.
- Svelte 5 editor: slide thumbnails, slide canvas with editable text boxes
  and passthrough placeholders, notes panel, toolbar (New/Open/Save/Undo).
- Fullscreen presenter mode: current slide, next-slide preview, slide
  counter, elapsed timer, speaker notes, keyboard navigation.
- Recovery: debounced (750 ms) autosave snapshots after each edit, startup
  recovery prompt with restore/discard/skip.
- Quality gate: `scripts/verify-local.sh` (fmt, clippy, test, svelte-check).
- Workspace of 11 library crates, Apache-2.0 license, rust-toolchain pin.

### Known limitations

- No image, shape, table, or chart editing (preserved on save, shown as
  placeholders in the editor).
- No animations or transitions.
- No PDF, PNG, SVG, or ODP export.
- No spell-check.
- 16:9 aspect ratio only.
- No signed or notarized installer for any platform.
