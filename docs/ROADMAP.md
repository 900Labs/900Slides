# 900Slides — Roadmap

Status: Draft v0.1
Owner: 900 Labs
Last updated: 2026-07-22

This roadmap lays out the feature trajectory for 900Slides from v0.1.0
through v1.0, scoped against twelve competing presentation products. It is
informed by [`COMPETITIVE_ANALYSIS.md`](./COMPETITIVE_ANALYSIS.md) and the
[`PRODUCT_SPEC.md`](../PRODUCT_SPEC.md).

## How to read this

The roadmap is sequenced in **horizons**, not quarters. Each horizon is
gated by the previous one being correct, recoverable, and respectably
tested. Adding features is additive only — features are not removed across
releases without a multi-year deprecation period.

| Horizon | Posture | Track claim |
| --- | --- | --- |
| v0.1.0 | Thin vertical slice | Proves the architecture: deck model, command bus, PPTX load/save with opaque passthrough, one template, basic presenter, recovery. |
| v0.2.0 | Editor completeness | Full slide editor (images, shapes, tables, charts, animations, more templates, dual-display presenter) with no schema break. |
| v0.3.0 | First installable track | macOS ad-hoc signed `.app`, secondary formats (ODP/PDF/PNG/SVG), magic move, bundled fonts. |
| v0.4.0 | Power-user track | Smart animate, version history, local comments, projector filters. |
| v0.5.0 | Local collaboration track | Comments, version history, brand kits, variables, batch generate. |
| v1.0 | Daily-driver contender | Notarized macOS installer, Windows and Linux installers, a11y at WCAG 2.2 AA. |

## Principles derived from the competitive analysis

These reinforce and extend [`PRODUCT_SPEC.md`](../PRODUCT_SPEC.md) section 4.

1. **Lossless PPTX editing.** Open a deck, edit it, save it. Unknown OOXML
   parts and relationships are preserved byte-for-byte when untouched. A
   round-trip through 900Slides never destructively rewrites a part it
   does not understand.
2. **Patch, do not regenerate.** Save paths modify the smallest possible
   set of parts. Where a part must be regenerated, the diff is surfaced
   for review before save.
3. **Transparent fidelity controls.** When an intent cannot be fully
   represented, the user is told before save with a per-slide warning,
   not after.
4. **Additive-only features.** No feature is removed across releases
   without a multi-year deprecation. Old workflows keep working.
5. **Reduce-motion is respected.** Animations and transitions are skipped
   or simplified when the OS reports the user prefers reduced motion,
   without disabling the authoring tools.
6. **A11y is a first-class product claim.** WCAG 2.2 AA is the public
   accessibility target from v1.0 and is measured from v0.4.0 onward.

## v0.1.0 — thin vertical slice (architecture proof)

Track claim: developer and early-tester build from source. Proves the deck
model, the command/undo bus, PPTX load/save with **opaque passthrough** plus
a narrow editable subset, one template, a basic presenter, and recovery.
v0.1.0 claims lossless passthrough, not fidelity.

Defined in [`PRODUCT_SPEC.md`](../PRODUCT_SPEC.md) section 5.1.

- Editing surface: slide canvas, thumbnails, notes panel; 16:9 only; a
  single editable text-box shape per slide (paragraphs, bold/italic/
  underline, ordered and unordered lists).
- One built-in template ("Default") with a theme and a master layout.
- No animations or transitions; the model reserves fields for them.
- Presenter: single window, current + next preview, slide number, elapsed
  timer, plain-text notes, keyboard nav (arrows / space / Home / End / Esc).
- PPTX native: load editable text into `slides-core`; carry everything else
  as opaque passthrough objects re-emitted byte-for-byte on save; loss ledger
  with per-slide warnings before save; custom part under `/customXml/`.
- File workflow: new / open / save / save as, privacy-preserving recent
  tokens, autosave recovery, startup recovery prompt.
- Safety: transactional command bus with bounded undo/redo.
- A11y/i18n: English UI, keyboard nav, focus rings, OS reduced-motion
  respected. No spell-check, no a11y checker.
- Quality gate: `verify-local.sh` including a no-op open/save round-trip
  that asserts untouched parts are byte-identical.

## v0.2.0 — editor completeness

Builds the v0.1.0 architecture into a full slide editor with no schema break.
Defined in [`PRODUCT_SPEC.md`](../PRODUCT_SPEC.md) section 5.2.

- Five more templates (Educator, Pitch, Conference Talk, Community Update,
  Photo Essay) with per-template themes, masters, and layout variants.
- Aspect ratios: 4:3, 16:10, custom.
- Images (PNG/JPEG/GIF/WebP/SVG, EXIF strip, allowlisted, size-capped) with
  crop and rotation.
- Full shape set (rect, rounded rect, ellipse, triangle, line, arrow,
  callout, star) with fill, outline, shadow, corner radius.
- Tables up to 50x50 with cell text, fill, borders, resize.
- Charts (bar, column, line, area, pie, scatter as SVG) with in-place
  data-table editing.
- Rich text: headings, strikethrough, super/subscript, links, blockquote,
  inline and fenced code, tabs.
- Animations (build-ins with deterministic timing) and transitions.
- Speaker notes rich text, slide sections, find and replace.
- Dual-display presenter mode, laser pointer and highlighter, black/white
  slide.
- Spell-check (en-US) with a user dictionary folder; shortcuts dialog;
  high-contrast theme.

## v0.3.0 — first installable track

Goal: the first track an everyday user installs and uses end to end
without compiling from source. Ships an ad-hoc signed macOS `.app` and
documents a Windows and Linux compile path.

Additions over v0.2.0:

- **Ad-hoc signed macOS `.app`** released through a GitHub Actions
  artifact (matching 900Sheets' v0.4.0 release posture).
- **Documented Windows and Linux source-build path** with no published
  installers.
- **Secondary formats**: ODP import/export, PDF export with selectable text
  and embedded fonts, PNG and SVG per-slide export, PDF image-per-page
  import.
- **Projector CSS filter panel** in presenter mode: invert, brightness,
  contrast, saturation, sepia, hue-rotate, persisted per device. Borrowed
  from Slidev. Cheap, high value for community speakers.
- **Magic Move / Morph equivalent** for identity-matched object morphing
  between adjacent slides, expressed in the editor and emitted as a
  `p:transition` morph on save. Object identity is tracked by stable IDs.
  Borrowed from Keynote, PowerPoint, Reveal.js, Figma Slides.
- **Stepped code highlighting** for code blocks with `1-3|4|5,7` style
  ranges. Borrowed from Slidev. Targeted at the developer persona.
- **Bundled open-licensed font set** (one sans, one serif, one mono)
  embedded in the binary so decks render consistently without depending
  on system fonts.

## v0.4.0 — power-user track

Goal: power users prefer 900Slides to LibreOffice Impress for daily PPTX
work, while staying local-first.

Additions over v0.3.0:

- **Local version history.** Every save is a content-addressed snapshot.
  Named versions, restore, "copy this revision", and a visual diff
  between two revisions. Borrowed from Google Slides and Keynote's
  "Restore an earlier version".
- **Local comments.** Anchored to slides, objects, and text ranges. Reply,
  resolve, assignment. Stored in the custom XML part under `/customXml/`
  and preserved
  across round-trip. Borrowed from Google Slides and Keynote.
- **Accessibility checker.** Surfaces missing alt text, low contrast,
  reading-order issues, missing slide titles. Modeled on PowerPoint's
  Accessibility Checker and Canva's a11y page.
- **Custom layouts per template.** Each template ships with multiple
  named layouts users can pick from. Aligns with PowerPoint and Keynote
  layout models.
- **Rehearse timings.** Per-slide duration recording.
- **Animations: motion paths.** Drawn and preset paths with editable
  points. Matches PowerPoint's motion path model.
- **Animations: trigger model.** Click / hover / after-previous / with-
  previous / timeline-driven. Matches PowerPoint's trigger vocabulary.
- **Per-slide reduce-motion override.** Override the system preference
  per slide for accessibility audits.
- **Animation Pane.** Explicit ordered list of build-ins with thumbnail
  previews, durations, and delays. Modeled on PowerPoint.
- **WCAG 2.2 AA measurement.** Accessibility checker now reports a
  numeric score per deck.

## v0.5.0 — local collaboration track

Goal: lightweight collaboration and content workflows that today require
a SaaS product. No server, no account.

Additions over v0.4.0:

- **Brand kit as a portable file.** `brand.toml` with palette, fonts,
  logos, locked-template list. Git-friendly, diffable, no server.
  Borrowed from Pitch and Canva.
- **Slide variables + batch deck generation.** A template with named text
  and image variables can be filled from a CSV or JSON to produce N
  decks. Borrowed from Pitch. Local, deterministic, scriptable.
- **Spreadsheet-linked charts.** A chart reads its data table from an
  adjacent CSV that can be edited outside 900Slides and refreshed on
  open. Borrowed from Beautiful.ai.
- **Outline-first content workflow.** A side panel where the user drafts
  an outline (one line per slide) and then "fills" the deck. Borrowed
  from Beautiful.ai's Create-with-AI workflow, minus the AI.
- **On-device live captioning during presentation** using whisper.cpp
  with a small on-device model. Borrowed from PowerPoint's live captions
  and Google Slides' live captions, with no remote audio calls.
- **Optional self-hosted collaboration** via a simple git-compatible
  sync (deferred but designed for). The PPTX package is git-friendly; the
  deck model is diff-friendly. We document a Git workflow before we ship a
  sync protocol.

## v1.0 — daily-driver contender

Goal: an everyday user on macOS, Windows, or Linux chooses 900Slides
over a subscription product for most of their slide work.

Additions over v0.5.0:

- **Notarized macOS installer** through a paid Apple Developer account.
- **Windows and Linux installers** (MSI and AppImage at minimum).
- **Smart Slide auto-reflow as opt-in mode.** A "snap to layout" mode
  that constrains freeform edits to a small set of layout blocks per
  template. Borrowed from Beautiful.ai. Toggleable per deck; the default
  remains freeform.
- **Slide ↔ Design view toggle.** Slide view hides power tools for
  presenters; Design view exposes them for authors. Modeled on Figma
  Slides. Behind a usability study.
- **Speaker Coach.** During rehearsal, whisper.cpp transcribes and the
  editor shows pacing (words per minute), filler word count, and
  verbatim-reading detection. No audio leaves the device.
- **WCAG 2.2 AA public claim.** With automated measurement, manual audit,
  and documented limitations.
- **A public template gallery** as a curated static site of
  community-submitted `.pptx` templates reviewed for a11y and quality.
  Submission is via pull request; no marketplace, no payment, no DRM.
- **CJK vertical text, RTL, and bidirectional support** in the editor and
  the renderer. Aligns with Keynote's typography advantage.
- **Pinned third-party extensions surface.** A documented stable custom
  XML part and a public extension manifest for power users. Borrowed
  from Marp's engine architecture.

## Features deliberately deferred past v1.0

- Real-time multi-user co-editing with presence and CRDTs. (Possible
  post-v1.0 with a self-hosted relay; design notes kept in
  `docs/THREAT_MODEL.md`.)
- A template marketplace with payment.
- A plugin runtime with sandboxed execution.
- AI-assisted deck generation. (Deferred until on-device model quality
  matches a community-priced expectation. The architecture leaves room
  for it.)
- Slide recording and narration in-app.
- Streaming or live broadcasting of presenter mode.
- Mobile or web clients.
- Cross-app integration with 900Sheets, 900Word, 900Image.
- An automatic binary update channel.

## Features explicitly out of scope

These appear in competitors and are deliberately not on the roadmap:

- **Subscription-locked editing features.** Anything behind a paywall.
- **Closed binary extensions to PPTX.** Every 900Slides-specific datum is
  in the custom XML part under `/customXml/` with a documented
  schema.
- **Cloud-required AI features.** On-device or nothing.
- **Engagement analytics, viewer tracking, or email-capture walls.**
- **Image-only PPTX export.** Export preserves native text, shapes, and
  animations wherever possible.
- **One-way PPTX export.** Every export is round-trippable.
- **Template marketplace as value proposition.** Curated static gallery
  only.
- **Auto-applied animations that cannot be disabled per slide.**

## Risk register

- **PPTX round-trip is the single hardest unknown.** The competitive
  evidence (LibreOffice Impress) suggests that full round-trip without
  degradation is unrealistic. The mitigation is the "lossless PPTX
  editing" principle: preserve unknown parts, patch only changed
  structures, surface a loss ledger.
- **Animation determinism.** The competitive landscape shows that morph
  effects are the most valued and the most fragile. Determinism must be
  proven with a hash test in CI.
- **Accessibility is often a claim, not a measurement.** WCAG 2.2 AA
  requires automated and manual audits. The accessibility checker ships
  in v0.4.0 and is the audit substrate from that point on.
- **No network is a feature and a ceiling.** Live captioning, voice-over,
  and collaboration all have an online ceiling. The on-device story
  (whisper.cpp, local git sync) is the answer.
- **Apple notarization.** Requires a paid Apple Developer account and a
  release engineering process. Held for v1.0 deliberately.

## How features are admitted to the roadmap

A feature is added to a horizon if it meets all of the following:

1. It traces to a competitive observation in
   [`COMPETITIVE_ANALYSIS.md`](./COMPETITIVE_ANALYSIS.md) **or** a
   documented user need from the personas in `PRODUCT_SPEC.md` section 3.
2. It does not conflict with the principles in `PRODUCT_SPEC.md`
   section 4 or the principles in this document.
3. It does not depend on a cloud service or a paid tier.
4. It has a public test that proves the behavior through the public
   import or export boundary.

A feature is removed from a horizon only after a multi-year deprecation
cycle and an explicit ADR in `docs/`.