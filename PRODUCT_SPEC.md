# 900Slides — Product Specification

Status: Draft v0.2 (desliced)
Owner: 900 Labs
Last updated: 2026-07-24

This document defines what 900Slides is, who it is for, what it ships in its
first release track, what it deliberately does not ship, and how it is built.
It is the source of truth for scope. Code, tests, and release notes must trace
back to the claims here.

The forward-looking feature plan lives in [`docs/ROADMAP.md`](docs/ROADMAP.md),
informed by [`docs/COMPETITIVE_ANALYSIS.md`](docs/COMPETITIVE_ANALYSIS.md),
which surveys twelve presentation products (PowerPoint, Keynote, Google Slides,
LibreOffice Impress, Reveal.js, Slidev, Marp, Pitch, Canva, Beautiful.ai,
Figma Slides). Anything not in this document or in the roadmap is not a
product claim.

## 1. Mission

900Slides is a free, local-first presentation builder for communities and
individuals priced out of subscription productivity software. It lets a person
build, present, and export a compelling deck on an ordinary computer, with no
account, no subscription, no telemetry, and no constant internet connection.

It exists because presentation software is basic working infrastructure for
classrooms, small businesses, community organizers, faith groups, public
services, researchers, journalists, and developers. Where PowerPoint and
Google Slides require subscriptions, accounts, cloud sync, or constant
connectivity, 900Slides must remain useful on a laptop with intermittent Wi-Fi
and an outdated OS.

## 2. Position in the 900 Labs family

900Slides is the presentation sibling of 900Word, 900Sheets, 900Image, and
900API. It shares the same product principles, technology stack, and
distribution posture:

- Rust workspace for the application logic
- Tauri v2 desktop shell
- Svelte 5 frontend
- Local-first storage, offline-first behavior, no telemetry by default
- Apache-2.0 license
- Conservative, evidence-backed public claims
- Public release evidence captured from generated or sanitized fixtures only

It does not share a runtime with the other apps. Each app is a standalone
desktop binary. Cross-app integration (e.g. embed a 900Sheets chart) is out of
scope for v0.1.0.

## 3. Target users and use cases

### 3.1 Primary personas

- **Teacher in a low-bandwidth region.** Builds weekly lesson decks on a
  shared school laptop, presents on a projector, occasionally exports PDF for
  print handouts.
- **Small business owner.** Pitches to clients and partners from a personal
  laptop at a cafe. Needs a deck that looks professional without buying a
  template pack.
- **Community organizer / faith leader.** Builds talks and announcements for
  in-person meetings, with one or two people collaborating on the same machine.
- **Student.** Builds class presentations on an older laptop, exports to PDF
  for submission, presents from a borrowed projector.
- **Developer / researcher.** Builds a technical talk for a meetup or
  conference; cares about keyboard control, code blocks, and presentable
  monospaced text.

### 3.2 Jobs to be done

1. Open a blank deck or a starter template and produce a clean, readable
   presentation in under an hour.
2. Add text, images, shapes, charts, and tables to slides without fighting
   the editor.
3. Apply a consistent look across a deck using a built-in professional
   template and theme.
4. Animate builds and transitions tastefully without becoming a motion-design
   tool.
5. Present the deck locally, with presenter notes, slide numbers, a timer,
   and a next-slide preview.
6. Export to PDF for handouts and save as PPTX for the unavoidable
   collaborator who uses PowerPoint.
7. Recover work after a crash, power loss, or accidental quit.

### 3.3 Anti-personas

900Slides is not for:

- Teams that require real-time multi-user collaboration (use Google Slides or
  Affine).
- Enterprises that require SSO, audit logs, or DLP (use PowerPoint or Keynote).
- Designers who need pixel-perfect control over custom motion graphics (use
  Keynote, After Effects, or a web framework).
- Users who cannot install a desktop app and require a browser-only workflow.

## 4. Product principles

1. **Local-first, offline-first.** Every primary action must work without a
   network. Network is an export convenience, never a prerequisite.
2. **No telemetry by default.** No analytics, no crash reporting, no remote
   calls unless the user opts in to a specific feature that requires it (and
   none ship in v0.1.0).
3. **No account, no subscription.** The app runs the first time it is opened.
4. **Conservative public claims.** Compatibility with PowerPoint, Keynote, and
   Google Slides is described only against evidence captured from generated or
   sanitized fixtures. Unsupported features are documented, not implied.
5. **Bounded complexity.** The editor wins by being predictable and forgiving,
   not by feature count.
6. **Recoverable.** Crash, quit, or power loss must never lose more than the
   last few seconds of editing.
7. **Keyboard-respectful.** Common actions have shortcuts; the editor never
   traps focus.
8. **Accessible by default.** Keyboard navigation, focus rings, sufficient
   contrast, and screen-reader labels for the slide canvas and presenter
   view. Accessibility claims are tested, not asserted.
9. **Lossless PPTX editing.** Unknown OOXML parts and relationships are
   preserved byte-for-byte when untouched. A round-trip through 900Slides
   never destructively rewrites a part it does not understand. Save paths
   modify the smallest possible set of parts.
10. **Transparent fidelity controls.** When an intent cannot be fully
    represented in OOXML, the user is told before save with a per-slide
    warning, not after.
11. **Additive-only features.** No feature is removed across releases
    without a multi-year deprecation cycle and an explicit ADR. Old
    workflows keep working.

## 5. Scope

### 5.1 v0.1.0 — public source release (thin vertical slice)

This is a developer and early-tester release. It is deliberately a **thin
vertical slice**: it proves the architecture end to end — deck model,
command/undo bus, PPTX load/save with opaque passthrough, one template, a
basic presenter, and recovery — without attempting the full feature surface.
Editor completeness is staged in §5.2; secondary formats, polish, and
longer-horizon work are in §5.3 and `docs/ROADMAP.md`. v0.1.0 claims
**lossless passthrough plus a narrow editable subset**, not fidelity.

**Editing surface**

- Slide canvas with a slide thumbnails panel and a notes panel.
- 16:9 aspect ratio only. Other ratios (4:3, 16:10, custom) are deferred.
- A single editable text-box shape per slide. All other PPTX shapes load as
  opaque, non-editable objects that are preserved byte-for-byte on save.

**Slide content**

- Text boxes with paragraphs and basic inline marks (bold, italic,
  underline) plus ordered and unordered lists. No headings-as-styles, no
  code blocks, no links, no tables, no charts, no images in v0.1.0.
- One built-in template ("Default") with a theme (background, heading and
  body font, accent color) and a master slide layout.

**Animations and transitions**

- None in v0.1.0. Slides change instantly in presenter mode. The deck
  model reserves fields for animation and transition so v0.2.0 can add
  them without a schema break.

**Presenter mode**

- A single presenter window: current slide, next-slide preview, slide
  number, an elapsed timer, and plain-text notes.
- Keyboard control: arrow keys, space, Home, End, Esc to exit.
- No laser pointer, highlighter, or black/white slide in v0.1.0.

**PPTX load and save (the core deliverable)**

- PPTX is the native format. Load builds a `slides-core` model for editable
  text boxes; every OOXML part 900Slides does not understand is carried as
  an opaque passthrough object and re-emitted byte-for-byte on save.
- Save regenerates only the slide parts that contain edited text and rewrites
  no other part.
- A loss ledger records unsupported content with per-slide warnings surfaced
  before save (the transparent-fidelity principle, §4.10).
- The app-specific custom part is wired per OOXML: it lives under
  `/customXml/` with an entry in `[Content_Types].xml` and a package
  relationship, so PowerPoint opens the deck without warnings.

**Local file workflow**

- New, open, save, and save as.
- Autosaved recovery snapshots and a final recovery write on close with
  unsaved changes.
- A startup recovery prompt that preserves unselected snapshots and
  quarantines corrupt snapshots.

**Safety**

- Transactional mutations through a command bus; each transaction produces
  an inverse. Bounded undo per §6.3.

**Internationalization and accessibility**

- English UI strings. Keyboard navigation, visible focus rings, and respect
  for the OS reduced-motion preference.
- No spell-check and no accessibility checker in v0.1.0.

**Quality gate**

- `./scripts/verify-local.sh` runs formatting, clippy, type-check, frontend
  tests, Rust tests, and a generated-fixture PPTX round-trip that asserts
  untouched parts are byte-identical before and after a no-op open/save.
- Public-release privacy gate verifies no local paths, hostnames, or secrets
  appear in fixtures or logs.

### 5.2 v0.2.0 — editor completeness

Builds the v0.1.0 architecture into a full slide editor. All features below
stage onto the v0.1.0 model without a schema break.

- The remaining five templates (Educator, Pitch, Conference Talk, Community
  Update, Photo Essay) with per-template themes and masters; slide masters
  and layout variants per template.
- Aspect ratios: 4:3, 16:10, and a custom ratio field.
- Images: PNG, JPEG, GIF (first frame), WebP, SVG. EXIF stripped on import
  unless explicitly preserved. Allowlisted MIME, size-capped. Image crop and
  rotation.
- Full shape set: rectangle, rounded rectangle, ellipse, triangle, line,
  arrow, right-arrow callout, five-point star; fill, outline, shadow
  (offset, blur, color, opacity), corner radius.
- Tables up to 50 rows x 50 columns with cell text, fill, borders, and
  column/row resize.
- Charts: bar, column, line, area, pie, scatter as SVG, with in-place
  data-table editing.
- Rich text: headings, strikethrough, superscript/subscript, links,
  blockquote, inline code, fenced code blocks, tabs.
- Animations: build-ins (none, fade, slide-in left/right/top/bottom, appear,
  disappear) with deterministic timing; one transition per slide (none,
  fade, slide, push, wipe).
- Speaker notes with rich text; slide sections with a collapsible section
  list; find and replace across slides.
- Dual-display presenter mode (separate presenter + audience windows), laser
  pointer and highlighter overlay, and a black/white slide for Q&A.
- Spell-check (en-US) with a user dictionary folder; a keyboard-shortcuts
  dialog; a high-contrast theme.

### 5.3 v0.3.0+ and deliberately deferred

Secondary formats and the installable track are staged in v0.3.0 per
[`docs/ROADMAP.md`](docs/ROADMAP.md), which carries the full horizon plan
(v0.3.0 first installable track, v0.4.0 power-user, v0.5.0 local
collaboration, v1.0 daily-driver). Items staged for v0.3.0 include: ODP
import/export, PDF export with selectable text and embedded fonts, PNG and
SVG per-slide export, PDF image-per-page import, an ad-hoc signed macOS
`.app`, bundled open-licensed fonts, and a CI recovery regression suite.

The following are valuable but explicitly out of scope through v1.0. They
will be revisited once the foundation is solid.

- Real-time multi-user collaboration.
- Cloud sync, accounts, remote document storage.
- A template marketplace or user-submitted templates.
- A plugin or scripting runtime.
- AI-assisted generation of slides, layouts, or speaker notes.
- Recording and narrating slides in-app.
- Streaming or live broadcasting of presenter mode.
- Mobile or web clients.
- Cross-app integration with 900Sheets, 900Word, or 900Image.

## 6. Architecture

900Slides uses one durable source of truth and one rendering layer per
surface, in the same shape as 900Word and 900Sheets.

1. `slides-core` owns the normalized deck model: deck, master, layout,
   slide, placeholder, shape, animation, and notes.
2. The Tauri webview is an editing projection of that model.
3. The presenter window is a separate projection tuned for playback.
4. `.pptx` is the canonical persisted package. 900Slides round-trips OOXML
   and adds a single custom XML part for app-specific data.
5. Rust import and export code sanitizes external content before it reaches
   the editor.

### 6.1 Repository layout

```
900Slides/
├── apps/desktop/              # Tauri v2 + Svelte 5 desktop app
├── crates/slides-core/        # Deck model, commands, undo, theme
├── crates/slides-pptx/        # PPTX load and save (native format)
├── crates/slides-odp/         # ODP import / export conversion boundary
├── crates/slides-pdf/         # PDF export and image-per-page import
├── crates/slides-render/      # Slide rendering to PDF, SVG, and PNG
├── crates/slides-animation/   # Deterministic build and transition playback
├── crates/slides-chart/       # Chart data and SVG previews
├── crates/slides-spell/       # Spell-check dictionary boundary
├── crates/slides-media/       # Image ingest, EXIF strip, MIME allowlist
├── crates/slides-i18n/        # Locale and accessibility helpers
├── crates/slides-fixtures/    # Sanitized generated fixtures only
├── docs/                      # Public documentation, ADRs, sprint records
└── scripts/                   # Validation and release-preflight scripts
```

### 6.2 Deck model (slides-core)

The deck model is a tree:

- `Deck` has a `theme`, a `master`, a list of `Layout`s, an ordered list of
  `Slide`s, and `notes_settings`.
- `Theme` has color palette, font stack, accent color, and background.
- `Master` has placeholder geometry and background layers.
- `Layout` is a named variant of the master with placeholder overrides.
- `Slide` has a `layout_ref`, an ordered list of `Shape`s, a list of
  `Animation`s, a `transition`, and a `notes` blob.
- `Shape` is one of `TextBox`, `Image`, `Shape` (geometric), `Table`,
  `Chart`, `Line`, or `Group`. Each carries a frame, transform, and style.
- `Animation` is a per-shape build-in with a timing offset relative to slide
  start or a previous animation.

The model is versioned. `slides-core` carries a `schema_version` and a
migrator per minor bump.

### 6.3 Transactions and undo

Mirroring 900Sheets and 900Word:

- All mutations go through a command bus that takes a transaction id.
- Each transaction produces an inverse transaction.
- Undo history is bounded: 100 transactions, 64 MiB aggregate, 32 MiB per
  transaction, 200,000 changed shape or text deltas per transaction.
- A transaction that exceeds a per-transaction limit is rejected without
  partially mutating the live deck.

### 6.4 Recovery

- After a successful edit, the editor waits 750 ms for activity to settle,
  flushes pending mutations, and writes a recovery snapshot to the app data
  directory.
- A normal save retires the corresponding recovery.
- At startup the app lists any recovery snapshots newest first. The user can
  restore one, discard one, or cancel and continue to the next.

### 6.5 Determinism

- All animation timings are deterministic. Two runs of the same deck produce
  identical frame sequences.
- Slide rendering is deterministic given the same theme, fonts, and input.
  The render crate exposes a hash of the rendered output for testing.
- Deterministic hashing depends on stable fonts, so the bundled
  open-licensed fonts (roadmap v0.3.0) are a prerequisite for a
  cross-machine render-hash test gate. Until then, the hash test runs on a
  single machine only.

### 6.6 Edit loop and model ownership

`slides-core` is the canonical source of truth; the desktop webview is a
read-and-command projection, never the owner.

- Load reads PPTX into a Rust `Deck`. A serializable snapshot of the deck is
  sent to the Svelte frontend over a Tauri command.
- The frontend never holds local state as truth. Each edit is issued as a
  command object via `invoke`; the Rust command bus applies it
  transactionally, produces an inverse for undo, and returns the new snapshot
  plus an undo token. The frontend re-renders from the returned snapshot.
- Save serializes the in-memory `Deck` plus the carried passthrough parts
  back to PPTX. Nothing in the frontend must be reconciled on save.

## 7. Format strategy

### 7.1 Native format: `.pptx`

PPTX is the canonical editable format. 900Slides opens PPTX, edits it in
memory against `slides-core`, and saves a valid PPTX back to disk. A
PowerPoint user can open, edit, and re-save a 900Slides-authored deck, and
900Slides can open, edit, and re-save a PowerPoint-authored deck, with
warnings when an intent cannot be fully represented.

A PPTX package is a ZIP archive of OOXML parts. 900Slides preserves every
part it does not understand, so unknown PowerPoint features survive a
round-trip untouched. App-specific data that has no direct PPTX equivalent
lives in a custom XML part registered the standard OOXML way:

- The 900Slides manifest lives under `/customXml/` with an entry in
  `[Content_Types].xml` and a package relationship. It carries app version,
  schema version, deck id, animation timing offsets that do not map to the
  OOXML animation model, theme overrides, and recovery breadcrumbs.

PowerPoint and other PPTX readers ignore this part. 900Slides reads and
preserves it on every save. Removing the part is treated as "this deck is
not a 900Slides deck" for recovery purposes only — editing still works.

### 7.2 Import and conversion boundaries

PPTX is opened directly, not imported. The "import" paths below produce a
new PPTX in memory; the user saves as PPTX to persist.

- **ODP.** Round-trip is a priority. ODP opens into the same `slides-core`
  model and saves as PPTX. ODP-specific structures that have no clean PPTX
  equivalent are mapped with a per-slide warning.
- **PDF.** Import is image-per-page: each page is rasterized at a fixed DPI
  and embedded as a full-bleed image on a new slide. Selectable text is not
  preserved in v0.1.0. The user is warned at import time.

### 7.3 Export

- **PDF.** Selectable text where the source has text, embedded fonts from the
  deck theme, deterministic page size from the slide aspect ratio.
- **ODP.** Round-trip is a priority. Animations and shapes are mapped to the
  nearest ODP equivalent with a warning when the mapping is lossy.
- **PNG per slide.** 1x and 2x, transparent or deck background.
- **SVG per slide.** Text as `<text>`, shapes as `<path>` and `<rect>`,
  images referenced by relative path.

"Save as PPTX" is the native save path and is not described as an export.
PowerPoint compatibility is the default, not a special case.

### 7.4 Format security

- All import paths treat input as untrusted. Archives are validated against
  a schema. Embedded resources are size-capped and MIME-allowlisted.
- Image bytes are run through a sanitizer that strips EXIF metadata unless
  the user explicitly preserves it.
- Opaque payloads in PPTX and ODP (OLE objects, embedded media outside the
  allowlist) are dropped with a warning.

## 8. Privacy, security, and telemetry

- No telemetry. No analytics. No crash reporting. No remote calls.
- No account, no login, no token storage.
- The app data directory is the only place where recovery snapshots and
  settings live. The user can open it from the Settings view.
- Network is used only when the user explicitly exports to a service that
  requires it. No such service ships in v0.1.0.

The threat model and privacy model are documented in `docs/THREAT_MODEL.md`
and `docs/PRIVACY_MODEL.md`, mirroring 900Word and 900Sheets.

## 9. Platform support

| Platform | v0.1.0 status                                       |
| -------- | --------------------------------------------------- |
| macOS    | Source builds. No notarized package in v0.1.0.      |
| Windows  | Source builds. No installer published in v0.1.0.    |
| Linux    | Source builds. CI compiles the desktop target.      |

The v0.2.0 track will publish an ad-hoc signed macOS `.app` and a CI-verified
Windows and Linux compile, matching 900Sheets' v0.4.0 posture.

## 10. Release evidence and public claims

- Public claims about PPTX, ODP, and PDF compatibility are limited to
  evidence captured from generated or sanitized placeholder fixtures.
- Evidence is stored under `crates/slides-fixtures/` and surfaced in
  `docs/COMPATIBILITY.md`.
- A release cannot claim a feature as "supported" without a passing test that
  exercises the feature through the public import or export boundary.

## 11. Out of scope (deliberately)

The following are intentionally excluded from v0.1.0 and v0.2.0. Several
appear in competing products and are explicitly not on the roadmap because
they conflict with the principles in section 4.

**Functional exclusions**

- Real-time multi-user collaboration with presence and CRDTs.
- Cloud sync, accounts, remote storage, viewer analytics.
- A template marketplace with payment or DRM.
- A plugin or scripting runtime with sandboxed execution.
- AI-assisted deck generation, layout suggestion, or speaker note writing.
- Slide recording, narration, or in-app voice-over.
- Live broadcasting of presenter mode, audience polls, voting widgets.
- Mobile or web clients.
- Cross-app integration with other 900 Labs apps.
- Automatic binary update channel.

**Distribution exclusions**

- A notarized macOS installer in v0.1.0 or v0.2.0.
- Windows and Linux installers in v0.1.0 or v0.2.0.

**Anti-patterns the spec explicitly rejects**

These patterns appear in competitors and are deliberately not adopted. See
[`docs/COMPETITIVE_ANALYSIS.md`](docs/COMPETITIVE_ANALYSIS.md) for the source
research.

- **Subscription-locked editing features.** Anything behind a paywall.
- **Removing features across major versions.** See the additive-only
  principle in section 4.
- **Closed binary extensions to PPTX.** Every 900Slides-specific datum
  lives in the custom XML part under `/customXml/` with a documented
  schema.
- **Conversion-first storage.** Save paths modify the smallest possible
  set of parts and preserve unknown OOXML byte-for-byte. See the
  lossless PPTX editing principle in section 4.
- **Cloud-only architecture.** Every primary action works offline.
- **Image-only PPTX export.** Export preserves native text, shapes, and
  animations wherever possible.
- **One-way PPTX export.** Every export is round-trippable.
- **Engagement analytics, viewer tracking, or email-capture walls.**
- **Auto-applied animations that cannot be disabled per slide.**

## 12. Open questions

Resolved by the v0.1.0 deslice (recorded here for traceability):

- Custom-part location: a single custom XML part under `/customXml/`,
  registered via `[Content_Types].xml` and a package relationship.
- Model ownership: `slides-core` is canonical; the frontend is a
  read-and-command projection (§6.6).
- Dual-display presenter, in-place chart editing, and spell-check are
  v0.2.0 (en-US only); bundled fonts are v0.3.0.

Still open:

1. Does the loss ledger render unsupported objects as cached PNG previews
   (so the user sees *something*) or as bounding-box placeholders? Default
   assumption: bounding-box placeholders in v0.1.0, cached previews in v0.2.0.
2. Should recovery snapshots be PPTX-on-disk or a separate serialized form?
   Default assumption: a PPTX written to the app data directory with a
   recovery suffix, so the restore path reuses the normal loader.

## 13. Glossary

- **Deck.** A single presentation document. One `.pptx` file.
- **Slide.** One page in the deck.
- **Master.** The base placeholder geometry and background for a deck.
- **Layout.** A named variant of the master.
- **Theme.** The color, font, and accent palette applied to a deck.
- **Shape.** A placeholder on a slide. Text box, image, geometric shape,
  table, chart, line, or group.
- **Build-in.** A per-shape entrance or exit animation.
- **Transition.** A per-slide change effect.
- **Presenter mode.** The local playback window with notes, timer, and
  next-slide preview.
- **Native format.** `.pptx`. The canonical editable package. 900Slides
  treats PPTX as its document format and round-trips OOXML, adding one
  custom XML part for app-specific data.