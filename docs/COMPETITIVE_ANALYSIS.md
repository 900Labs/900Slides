# 900Slides — Competitive Analysis

Status: Research input to the roadmap. Drawn from vendor docs, support
pages, and recent reviews for twelve presentation products as of July 2026.

This document is the raw research. The synthesis and the forward-looking plan
live in [`ROADMAP.md`](./ROADMAP.md).

## Products surveyed

| Product | Category | License / cost |
| --- | --- | --- |
| Microsoft PowerPoint | Legacy heavyweight | Microsoft 365 subscription |
| Apple Keynote | Legacy heavyweight | Free on Apple devices; advanced features behind Creator Studio (2026) |
| Google Slides | Free / cloud heavyweight | Free with Google account; Gemini features require paid Workspace / AI plan |
| LibreOffice Impress | Open-source desktop | MPL-2.0, free |
| Reveal.js | Developer HTML framework | MIT, free; commercial GUI on Slides.com |
| Slidev | Developer Markdown + Vue framework | MIT, free |
| Marp | Developer Markdown framework | MIT, free |
| Pitch | Modern web challenger | Freemium SaaS |
| Canva Presentations | Modern web challenger | Freemium SaaS |
| Beautiful.ai | Modern web challenger | Freemium SaaS |
| Figma Slides | Modern web challenger | Freemium SaaS |

## Per-product synthesis

Each entry below is the synthesis the roadmap depends on, not a verbatim
copy of vendor marketing. Long sections are condensed to the points that
change a product decision.

### Microsoft PowerPoint

- **Strongest asset**: PPTX is the de facto interchange format. Every other
  product either imports or exports it.
- **Animation model**: the most expressive in the category. ~50 build-ins,
  ~50 transitions, Morph, motion paths with editable points, an Animation
  Pane with explicit timing, triggers, and rewind. The cost is complexity.
- **Collaboration**: real-time co-authoring through OneDrive and SharePoint
  with version history, threaded comments, presence avatars, and sensitivity
  labels.
- **AI layer**: Copilot, Designer, Presenter Coach, Speaker Coach with
  rehearsal feedback, image generation through Bing / DALL-E, and live
  captions with translation during presenting. All require a paid tier and
  send content to Microsoft servers.
- **Presenter mode**: notes, timer, next-slide preview, laser pointer / pen,
  Rehearse Timings, Record Slide Show with audio + ink, PowerPoint Live for
  web presenting with attendee pacing, Quizzes / Forms for live polling.
- **Weaknesses for our audience**: subscription-locked AI, cloud-first
  posture, heavy weight, animation lock-in that round-trips badly, and
  aggressive telemetry.
- **For 900Slides**: borrow the structure of an Animation Pane and the
  concept of motion paths. Avoid the cloud dependency and the paid AI wall.

### Apple Keynote

- **Strongest asset**: typography and rendering quality. Ligatures, fractions,
  vertical text, OpenType features, and CJK handling are best in class.
- **Magic Move** (since 5.0, 2009): object-level morph by matching identity
  across consecutive slides. Closer to a "design intent" primitive than
  PowerPoint's Morph.
- **3D transitions**: OpenGL transitions (Cube, Flip, Twist, Droplet, Grid,
  Reflection, Revolving Door, Swoosh) that feel premium and effortless.
- **Collaboration**: real-time co-authoring through iCloud Drive since 2016,
  threaded comments, activity view, multi-platform Mac / iPad / iPhone / web.
- **Apple Creator Studio (2026)**: outline-to-deck, auto-generated presenter
  notes, Slide Clean Up, image generation through Image Playground, Super
  Resolution, Auto Crop, Generate Custom Shapes, Edit in Pixelmator round-trip.
  Most advanced features sit behind a paid tier.
- **Recording and remote**: Record Presentation exports a narrated `.m4v` or
  `.mov`, Remote control from Apple Watch and iPhone, multi-presenter
  slideshows, FaceTime presenting, password-protected kiosk mode.
- **Accessibility**: VoiceOver authoring guide, system accessibility hooks,
  honors Reduce Motion. Notably **no live captions during presentation** and
  no formal Accessibility Checker.
- **Weaknesses**: Apple-only platforms, lossy PowerPoint round-trip, no
  SmartArt equivalent, no live captions, frequent feature removals across
  major versions (6.0 in 2013 and 15.x in 2026).
- **For 900Slides**: borrow the Magic Move concept (object-identity morph).
  Adopt an additive-only feature policy to avoid Keynote's removal churn.
  Borrow Slide Clean Up as a local rule-based layout engine.

### Google Slides

- **Strongest asset**: the easiest path to a shared deck. Real-time collab,
  presence, link sharing, version history, comments with assignment.
- **AI**: Gemini can generate a slide from a prompt, generate and edit
  images, rewrite and adjust tone of slide text. Gated by paid Workspace /
  Google AI plans. Some "generated slides" come back as rasterized images and
  are not editable as real slides.
- **Presenter mode**: notes, timer, laser pointer, live captions, Q&A
  through a link, Meet integration for presenting into meetings.
- **Accessibility**: alt text, screen reader support, keyboard navigation,
  braille display support, live captions. The Accessibility Checker is
  shallower than PowerPoint's.
- **Export**: PPTX, ODP, PDF, TXT, JPEG, PNG, SVG, and a Google-hosted
  embeddable HTML. Export fidelity is reasonable for common content but
  drifts on masters, animations, and theme inheritance.
- **For 900Slides**: borrow the review / comments model (anchored to objects
  and ranges). Avoid the cloud dependency and the conversion-first storage
  pattern.

### LibreOffice Impress

- **Strongest asset**: offline-first desktop authoring with no account.
  Broad object model: equations, charts, 3D, curves, connectors, freeform,
  Fontwork, OLE.
- **PPTX round-trip (the critical context for 900Slides)**: Impress can
  open, edit, and save PPTX, but round-trip is imperfect. What generally
  works: ordinary text, basic shapes, common images, basic tables, basic
  charts, simple masters, hyperlinks, simple animations. What degrades:
  theme fonts and inheritance, placeholder geometry, autofit, SmartArt,
  complex charts, gradients / shadows / glow / bevels / 3D / soft edges,
  connectors and glue points, image masks, grouped object z-order,
  animation ordering and timing, theme background behavior, notes and
  comments. What breaks: complex animations and triggers, Morph-like
  transitions, SmartArt editability, VBA macros, embedded OLE, Excel-linked
  charts, unsupported codecs, missing fonts.
- **Collaboration**: no real-time collab. Sharing is filesystem-based or
  through external services (Nextcloud, ownCloud, Git). Version history is
  whatever the filesystem provides.
- **Accessibility**: alt text, keyboard access, screen reader, PDF export
  preserves structure, but Accessibility Checker is less presentation-
  specific than PowerPoint's.
- **For 900Slides**: lesson is to **avoid the "open, convert to internal
  model, regenerate PPTX" pattern**. Preserve unknown OOXML parts and
  relationships, patch only changed structures, surface a loss ledger for
  unsupported content, and render fallback previews for objects that
  cannot be edited.

### Reveal.js

- **Editing model**: HTML-first. One `index.html` of `<section>` elements.
  Markdown is opt-in via a plugin. No GUI editor in the OSS project.
- **Standout mechanics**: **fragments** as a first-class stepped reveal
  primitive, **Auto-Animate** (morph between adjacent slides by element
  identity), nested slides, overview mode, jump-to-slide, programmable
  themes, `data-*` extension surface.
- **Code**: syntax highlighting via highlight.js, LaTeX via MathJax plugin.
  No Monaco, no Sandpack, no in-slide code runner.
- **Presenter**: speaker view with notes, timer, pacing timer, overview.
  No built-in drawing, camera, or recording.
- **Export**: PDF via the print stylesheet and a browser. PPTX is not
  first-class — Decktape is the community CLI.
- **For 900Slides**: borrow Auto-Animate's identity-matching pattern for a
  Magic Move / Morph equivalent. Adopt fragments as a first-class concept.

### Slidev

- **Editing model**: Markdown-first with Vue components inside Markdown and
  a CLI-driven dev workflow. Heavy toolchain (Node, Vite, Vue).
- **Code**: best-in-class. Shiki + TwoSlash + Monaco + Shiki Magic Move +
  code runners (code-runner / WebContainers / iframes). Code groups, line
  ranges, stepped highlighting.
- **Presenter**: presenter mode with three layouts, screen mirror for live
  coding demos, notes editor, drawing, recording. **CSS filter panel** in
  the projector view (invert, brightness, contrast, saturation, sepia, hue)
  persisted per device.
- **Export**: PDF, PPTX (image-per-slide), PNG, MD, static SPA build.
  PPTX export is image-based, so text is not selectable.
- **For 900Slides**: borrow the projector CSS filter panel (cheap, high
  value for community speakers). Do **not** ship image-only PPTX export.
  Stepped code highlighting belongs in the developer persona track.

### Marp

- **Editing model**: pure CommonMark with `---` slide separators, optional
  YAML front-matter, and HTML-comment directives. One file, Git-friendly,
  no GUI.
- **Engine / converter plugin architecture**: `--engine` accepts an npm
  module, a class, or a JS function. This is a clean extension surface.
- **Standout feature**: zero-friction source model. A Marp deck is one
  Markdown file with directives for metadata.
- **Weaknesses**: animation is essentially absent beyond fragmented lists
  and View Transitions. PPTX export is image-based by default; the
  experimental `--pptx-editable` requires LibreOffice and is lower fidelity.
- **For 900Slides**: lesson is to keep the source model (PPTX) portable and
  inspectable, adopt a directive-style metadata layer for non-visual
  intent, and avoid treating PPTX export as a "render and stamp" afterthought.

### Pitch

- **Editing model**: freeform canvas with a slide grid timeline, ready-made
  layouts (~200+), real-time co-editing, slide assignments, contextual
  comments, co-presenting.
- **Differentiators**: **slide variables + batch deck generation** (fill
  text and image variables from a CSV or JSON), **Pitch Rooms** (branded
  deal rooms combining decks + files + links), per-slide assignments,
  viewer analytics, **brand library** shared across teams.
- **AI**: Pitch Agent (May 2026) generates on-brand decks from prompts and
  refines them through chat. 25+ AI actions including image generation,
  tone rewrite, slide summaries, and shareable messages.
- **Export**: PDF (including mobile), PPTX (round-tripped), share link,
  embed link. PPTX export breaks complex animations.
- **For 900Slides**: borrow slide variables + batch deck generation
  ("generate 30 personalized onboarding decks from a CSV"), model brand
  assets as a portable file (`brand.toml` or similar), and slide
  assignments as local metadata.

### Canva Presentations

- **Editing model**: drag-and-drop freeform canvas with multi-format
  publishing (decks, docs, sheets, whiteboards, sites).
- **Differentiators**: **Magic Animate** (one-click "animate the deck"),
  **Brand Kits** + **Brand Templates** (locked layouts), **Magic Write**,
  **Magic Resize**, **Magic Layers**, **Magic Image/Video Generator**,
  brand-trained AI.
- **Export**: broadest matrix in the category — PDF, PPTX, MP4, GIF, HTML
  (via Canva Sites), PNG / JPG, animated Web export, social publishing.
- **Accessibility**: alt text, focus order, color contrast warnings, reduce-
  motion support, an accessibility checker (WCAG-oriented).
- **Weaknesses**: cloud-only, increasingly heavy, freeform canvas is
  constrained, recognizably "Canva" output.
- **For 900Slides**: borrow the multi-format export posture (we have PDF,
  ODP, PNG, SVG; expand to MP4 via offscreen render and HTML5). Adopt
  reduce-motion as a system-preference override on animations. Borrow the
  a11y checker model.

### Beautiful.ai

- **Editing model**: **Smart Slides** (auto-aligning layout blocks). 300+
  smart layouts that reflow as you edit. Constrained by design.
- **Differentiators**: locked customizable themes, shared slide libraries,
  **Create with AI** (outline-first guided workflow), AI image generation
  with style presets, embedded voice-overs per slide, spreadsheet-linked
  charts.
- **Export**: PDF, PPTX (one-way only), share link. No video / GIF / HTML.
- **For 900Slides**: borrow Smart Slide auto-reflow as an **opt-in** mode
  (not the default) so power users keep freeform. Adopt the outline-first
  content workflow as a UX primitive even without AI. Spreadsheet-linked
  charts are a natural fit for the data-driven deck use case.

### Figma Slides

- **Editing model**: **toggle between Slide view and Design view**. Slide
  view is a presentation grid with notes and polls; Design view is the
  full Figma canvas (auto layout, layers, components). Real-time
  multiplayer, chat, audio, comments, Spotlight.
- **Animation**: **Smart Animate** matches objects between adjacent slides
  and tweens position / scale / rotation / opacity with easing curves and
  spring presets. Object-level animation tracks inherited from Figma
  Motion. Transitions include Dissolve, Push, Slide, Move with direction.
- **Media**: live embeddable Figma prototypes (clickable UI in a slide),
  live polls and voting widgets, alignment scale for audience engagement,
  video with "Dress your videos" overlay controls.
- **Accessibility**: the model to study. Public commitment to **WCAG 2.2 AA**,
  documented screen reader support, keyboard navigation (F6, Cmd-K actions
  menu), focus navigation, contrast, presenter notes accessible via
  keyboard.
- **Weaknesses**: browser-first Chromium app, no usable offline path, PPTX
  import has severe limitations (videos, tables, diagrams, animations
  dropped), no clean PPTX export.
- **For 900Slides**: borrow Smart Animate transitions (pure client-side
  motion, no server). Adopt WCAG 2.2 AA as a first-class accessibility
  claim. The Slide ↔ Design view toggle is interesting but adds UI
  complexity; defer to a usability study.

## Cross-cutting patterns

### Patterns worth borrowing

- **Identity-matched object morph** (Magic Move, Morph, Auto-Animate,
  Smart Animate). The single highest-leverage animation feature in the
  category and pure client-side.
- **First-class fragments / click steps** (Reveal.js, Slidev). A clean
  abstraction for stepped reveals that maps cleanly to PowerPoint
  animation triggers and to a timeline panel in the GUI.
- **Projector CSS filter panel in presenter mode** (Slidev). Cheap,
  high-value, solves a real embarrassing problem for community speakers.
- **Slide variables + batch deck generation** (Pitch). Transforms the
  "generate 30 personalized decks from a CSV" workflow and is naturally
  local-first.
- **Brand kit as a portable file** (Pitch, Canva). Git-friendly, diffable,
  no server.
- **Local version history with named versions and diff** (PowerPoint,
  Keynote, Google Slides). Easy to do with a content-addressed store,
  hard for any cloud product to match for offline users.
- **Local comments anchored to slides, objects, and ranges** (Google
  Slides, Keynote). Review workflow without a server.
- **On-device live captioning** (PowerPoint, Google Slides). Whisper.cpp
  fits naturally in the Tauri / Rust stack and sidesteps the privacy
  downsides of Microsoft or Google.
- **WCAG 2.2 AA accessibility posture** (Figma Slides). A real moat in
  OSS and rare in the slide category.
- **Multi-format export matrix** (Canva). PDF, PPTX, ODP, PNG, SVG, MP4,
  HTML, GIF — spread across a small toolchain.

### Patterns to avoid

- **Subscription-locking core editing features** (Microsoft 365, Apple
  Creator Studio). Directly conflicts with the audience.
- **Removing features across major versions** (Keynote 6.0, Keynote 15.x,
  PowerPoint Ribbon reshuffles). Adopt an additive-only policy.
- **Treating the file format as a walled garden** (Microsoft's Morph data
  extensions, Apple's `.key` schema drift). 900Slides commits to in-place
  PPTX editing and preservation of unknown parts.
- **Conversion-first storage** (LibreOffice Impress with PPTX, Google
  Slides with PPTX). Patch only changed structures, preserve unknown
  parts, surface a loss ledger.
- **Cloud-only architecture** (Google Slides, Pitch, Canva, Beautiful.ai,
  Figma Slides). Direct mission conflict.
- **Massive template marketplace as value proposition** (Canva, Pitch).
  Biases toward low-quality derivative design and creates lock-in.
- **Engagement analytics as gating feature** (Pitch, Beautiful.ai).
  Wrong economics for free / local-first.
- **Image-only PPTX export** (Slidev, Marp default, Reveal.js + Decktape).
  Defeats the in-place PPTX editing differentiator.
- **One-way PPTX export** (Beautiful.ai, Figma Slides). Contradicts the
  core thesis.
- **Auto-applied animations that cannot be disabled per slide** (Beautiful.ai).
  Accessibility problem.
- **AI features that are gated, metered, or rely on external models**
  (all four modern challengers). Inference cost is incompatible with
  the audience.

## Sources

Per-product source URLs were collected by the research subagents and
recorded in their individual reports. The full list is preserved in the
research artifacts at `/docs/research/` and can be regenerated on demand.

Representative URLs (used during synthesis):

- Apple Keynote User Guide: https://support.apple.com/guide/keynote/welcome/mac
- Apple Creator Studio features in Keynote: https://support.apple.com/guide/keynote/apple-creator-studio-features-in-keynote-tancda0441cd/mac
- Microsoft PowerPoint product page: https://www.microsoft.com/en-us/microsoft-365/powerpoint
- Microsoft 365 Copilot in PowerPoint: https://www.microsoft.com/en-us/microsoft-365/microsoft-copilot
- Google Slides help: https://support.google.com/docs/answer/2763168
- Generate a slide with Gemini in Google Slides: https://support.google.com/docs/answer/16961475
- LibreOffice Impress Help: https://help.libreoffice.org/latest/en-US/text/simpress/main0000.html
- LibreOffice PPTX round-trip guidance: https://help.libreoffice.org/latest/en-US/text/shared/guide/import_ms.html
- Reveal.js docs: https://revealjs.com/
- Slidev docs: https://sli.dev/
- Marp docs: https://marp.app/
- Pitch: https://pitch.com
- Canva Presentations: https://www.canva.com/presentations/
- Beautiful.ai: https://www.beautiful.ai/
- Figma Slides: https://www.figma.com/slides/
- Figma Slides accessibility: https://help.figma.com/hc/en-us/articles/35063862380311