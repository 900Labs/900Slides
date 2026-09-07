# 900Slides roadmap

Last updated: 2026-09-07. Priorities are gated by evidence, not dates.

The current source is on the 0.4 development track. It includes text, images,
shapes, tables, charts, local history/comments, templates, presenter tools,
animations, and common exports. Feature presence does not establish everyday
usability, platform compatibility, or performance on older computers.

The next milestone is **a reliable everyday presentation editor for modest
computers**. The [competitive research](COMPETITIVE_ANALYSIS.md),
[product specification](../PRODUCT_SPEC.md), and
[low-resource acceptance matrix](LOW_RESOURCE_REQUIREMENTS.md) define the bar.

## 1. Protect work and make core editing predictable

Current audit remediation is recorded in [Wave 23](sprint-records/wave-23.md):
Save/Save As, unsaved-work prompts, atomic file replacement, draft recovery,
chart save/undo fidelity, bounded caches, presenter backgrounds, template
selection and small-screen navigation. Preserve these with regression tests.

Before calling this milestone complete:

- Qualify failed writes, forced exits, restart recovery and dirty-document
  transitions on packaged applications (LR-08).
- Exercise complete create/edit/save/reopen/present/export workflows, including
  chart/table/image edits and unsupported PPTX content (LR-09).
- Validate controls, dialogs and keyboard navigation at 1024×768, 1366×768,
  and increased UI scaling (LR-06, LR-11).
- Have target users produce and hand over a five-slide lesson or community
  update. Use observed blockers to order the next work (LR-14).

## 2. Qualify real installations and constrained devices

Installable Windows and Linux packages belong in the next usable milestone.
The workflows now configure Windows NSIS with offline WebView2, Linux `.deb`
and `.AppImage` on an older build baseline, and macOS `.app`. All require
successful artifact builds plus clean-machine installation evidence.

- Test the complete offline installation path, including dependencies (LR-01).
- Measure cold start, process-tree memory, editing latency and idle CPU on the
  4 GB primary and 2 GB constrained profiles (LR-02–05).
- Publish executable, download, installed and runtime sizes separately (LR-07).
- Stress large or malformed documents and make slow operations understandable
  and recoverable (LR-13).

The current byte-bounded caches reduce one source of growth; they are not
whole-process memory limits. Do not claim support for an obsolete OS because
its hardware is old, or infer compatibility from the Tauri framework minimum.

## 3. Make good slides easier to produce

Prioritize coherent authoring over more animation controls:

- Useful starter content/layouts, predictable title/body editing, alignment,
  and spacing; validate that previews match editor, presenter and exports.
- Better master/layout fidelity and font substitution notices.
- Complete image-description editing, screen-reader workflows, and accessible
  error/recovery dialogs. The existing weighted document score is not WCAG
  conformance measurement.
- Externalize UI strings, then validate translations, text expansion, RTL,
  Devanagari and CJK with actual font/script fixtures (LR-10–12).

## 4. Later capabilities

These remain design candidates, after the usability and installation gates:

- Portable brand kits, local outline-to-slides, CSV/JSON template variables,
  batch generation and spreadsheet-linked charts.
- More complete local version comparison and review workflows.
- Optional layout reflow and a simpler presentation-focused editor view,
  subject to usability evidence.
- A curated, open-licensed template gallery and documented extension format.
- Signed Windows and notarized macOS distributions.
- On-device captions, rehearsal feedback, recording or AI assistance only if
  they meet quality and hardware budgets without remote calls.

Real-time collaboration, cloud synchronization, mobile/web clients and automatic
updates are deferred. Accounts, subscriptions, viewer tracking, analytics,
remote inference and remotely fetched assets are excluded from application code.

## Release decisions

A release report links the exact revision, artifact checksums, automated checks,
manual smoke results, target-device measurements and remaining limitations.
Historical feature work is in the [changelog](../CHANGELOG.md) and sprint records.
A check marked "not verified" is an open release gate; it is not an implicit pass.
