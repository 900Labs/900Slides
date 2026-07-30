# Wave 21 — UI overhaul: professional editor experience

Status: Proposed
Owner: 900 Labs
Scope target: Complete frontend redesign to match PowerPoint/Keynote/Google
Slides conventions. Backend (crates) unchanged — this is a desktop-only
(`apps/desktop/`) rewrite of the Svelte layer.
Last updated: 2026-07-30

## Problem

The current UI is pre-alpha: big text boxes for New/Open/Save instead of
toolbar buttons; no native app menu; a stray default text box on blank
slides; clicking shapes does nothing; shape buttons are individual instead
of a flyout picker; text box creation is missing; formatting is lost on
blur because there's no proper selection/edit-mode model.

## Design principles (from research)

1. **Classic menu bar** (File, Edit, View, Insert, Format, Arrange, Slide
   Show, Help) — matches macOS/Google Slides expectations.
2. **Single-row toolbar** with compact icon buttons grouped by function.
3. **Formatting scope resolution**: Shape (selected, not editing) → all
   text; Run (editing + selection) → selected text only; Paragraph (editing)
   → current paragraph.
4. **Double-click to edit text** in a shape; single click to select.
5. **Shape flyout palette** (categories grid) with drag-to-create.
6. **Right-side inspector** with contextual tabs.
7. **Clean blank canvas** — no default shapes.

## What this wave delivers

All changes are in `apps/desktop/` — no crate modifications.

| Area | Changes |
| --- | --- |
| Native menu | Tauri menu configuration (File/Edit/View/Insert/Format/Arrange/Slide Show/Help) |
| Toolbar | Compact icon buttons replacing text boxes; grouped by function |
| Canvas | Remove default text box; fix selection (click to select, double-click to edit); drag-to-create shapes/text |
| Shape picker | Flyout palette replacing individual shape buttons |
| Text box tool | Dedicated text box creation tool (Insert > Text Box or toolbar) |
| Formatting | Proper scope resolution (shape/run/paragraph); applies on commit, not lost on blur |
| Inspector | Right sidebar with Style/Text/Arrange tabs, disclosure sections |
| Slide navigator | Drag-to-reorder, right-click context menu (New/Duplicate/Delete/Layout) |

## Detailed specs

### 1. Native menu bar

Configure via Tauri's `Menu` API in `main.rs`. Map menu items to existing
Tauri commands or frontend events:

- **File**: New (⌘N), Open… (⌘O), Close (⌘W), Save (⌘S), Save As… (⇧⌘S),
  Export ▸ (SVG/PNG/PDF), Print… (⌘P), Quit (⌘Q)
- **Edit**: Undo (⌘Z), Redo (⇧⌘Z), Cut (⌘X), Copy (⌘C), Paste (⌘V),
  Duplicate (⌘D), Delete, Select All (⌘A), Find… (⌘F), Find and Replace… (⌓⌘H)
- **View**: Zoom In (⌘+), Zoom Out (⌘−), Fit to Window (⌘0),
  Show/Hide Notes, Show/Hide Ruler
- **Insert**: New Slide (⇧⌘N), Text Box, Image…, Shape ▸, Table…, Chart ▸,
  Hyperlink… (⌘K), Comment
- **Format**: Font…, Bold (⌘B), Italic (⌘I), Underline (⌘U),
  Align Left/Center/Right/Justify, Layout ▸
- **Arrange**: Bring to Front, Bring Forward, Send to Back, Send Backward,
  Align ▸, Distribute ▸, Rotate ▸, Lock/Unlock
- **Slide Show**: Start from Beginning, From Current Slide, Rehearse Timings
- **Help**: Keyboard Shortcuts, About

### 2. Toolbar redesign

Replace the current big text boxes with a compact single-row toolbar:
- Left: Undo, Redo, | New Slide (split → layouts), | Save
- Center: Font family dropdown, Size, B/I/U, Text color, | Alignment buttons
- Right: Text Box, Shape (flyout), Image, Table, Chart, | Present
- Each group separated by a thin divider.
- Icons (not text) for most buttons, with tooltips.

### 3. Canvas interaction fixes

- **Blank canvas**: new decks/slides start with zero shapes (remove the
  default text box from `new_deck`).
- **Selection model**: 
  - Click shape → selects it (shows resize handles + rotation handle).
  - Click empty canvas → deselects.
  - Double-click shape → enters text edit mode (caret appears).
  - Esc or click outside → exits edit mode (shape stays selected).
- **Drag-to-create**: when a shape/text tool is active, click-drag on the
  canvas creates the object at the drag bounds.
- **Formatting scope**:
  - Selected + not editing → formatting applies to all text in the shape.
  - Editing + text selected → applies to the selected run only.
  - Editing + caret only → applies to current paragraph/run.

### 4. Shape picker flyout

Replace individual shape buttons with a single "Shape" toolbar button that
opens a flyout grid:
- Categories: Rectangles, Basic Shapes, Arrows, Stars.
- Each shape shown as an icon in a grid (~6 per row).
- Clicking a shape activates drag-to-create mode.
- Recently Used row at top.

### 5. Text box tool

- Toolbar "Text Box" button (or Insert > Text Box).
- Activates drag-to-create: click-drag on canvas to draw the text box.
- After creating, enters text edit mode immediately.
- Plain click drops a default-size text box centered on click point.

### 6. Right-side inspector

A context-sensitive right sidebar with tabs:
- **Style** (when a shape is selected): Fill color, Border (color/width/dash),
  Shadow (toggle/color/blur/offset), Opacity slider.
- **Text** (when text box selected or editing): Font family, size,
  B/I/U/S, color, alignment, bullets, line spacing.
- **Arrange** (always available for selected shapes): Position X/Y,
  Size W/H, Rotation, layering (Front/Back/Forward/Backward buttons),
  Align, Distribute, Lock.

### 7. Slide navigator improvements

- Drag-to-reorder slides (visual drop indicator).
- Right-click context menu: New Slide, Duplicate Slide, Delete Slide,
  Layout ▸, Hide Slide.
- Slide numbers shown on thumbnails.
- Active slide highlighted.

## Execution strategy

This is a large frontend-only change. Split into focused subagents:

1. **Menu + toolbar** — native menu config + compact toolbar.
2. **Canvas + selection model** — blank canvas, click/double-click/drag-to-
   create, formatting scope resolution.
3. **Shape picker + text tool** — flyout palette, drag-to-create.
4. **Inspector** — right sidebar with contextual tabs.
5. **Slide navigator** — drag-reorder, context menu.

## Acceptance criteria

1. The app has a native menu bar with File/Edit/Insert/etc.
2. New/Open/Save are toolbar buttons, not text boxes.
3. A blank slide is truly empty (no default text box).
4. Clicking a shape selects it and shows handles.
5. Double-clicking a shape enters text edit mode.
6. The Shape button opens a flyout grid; selecting one enables drag-to-create.
7. Text box tool creates text boxes via drag-to-create.
8. Formatting applies correctly based on selection scope.
9. A right-side inspector shows shape properties.
10. Slides can be reordered by drag.
