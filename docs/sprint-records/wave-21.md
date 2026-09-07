# Wave 21 — UI overhaul: professional editor experience

Status: In progress
Owner: 900 Labs
Scope target: a durable editor interaction upgrade that follows familiar
presentation-app conventions. This wave includes the desktop UI plus the
small core and PPTX persistence changes required for new slides to survive
save, recovery, and reopen.
Last updated: 2026-08-03

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

## Delivered scope

This is not a complete PowerPoint/Keynote replacement. The delivered scope is
the working creation and direct-manipulation loop listed below; unsupported
native menu commands are intentionally omitted instead of being enabled
no-ops.

| Area | Changes |
| --- | --- |
| Native menu | Tauri menu configuration for implemented File, Edit, Insert, Format, Slide Show, and Help actions |
| Toolbar | Compact icon buttons replacing text boxes; grouped by function |
| Canvas | Click to select, keyboard selection, double-click or Enter to edit text, Escape to exit, drag to move/resize, pixel-sized arrow nudges |
| Shape picker | Flyout palette that arms a geometric creation tool; click uses a default frame and drag supplies explicit bounds, including horizontal and vertical lines |
| Text box tool | Dedicated click-to-place text box tool (Insert > Text Box or toolbar) |
| Formatting | Existing runs are retained across textarea edits; untouched bold, italic, and multi-run spans no longer collapse to defaults |
| Inspector | Right sidebar with Style/Text/Arrange tabs, disclosure sections |
| Slide persistence | Undoable slide insertion plus PPTX slide parts, relationships, content types, ordered presentation IDs, and save/reopen coverage |

## Original target and deferred scope

### 1. Native menu bar

The native menu exposes only actions wired to existing frontend commands:

- **File**: New (⌘N), Open… (⌘O), Save (⌘S), Save As… (⇧⌘S), Export ▸
  (SVG/PNG/PDF)
- **Edit**: Undo (⌘Z), Redo (⇧⌘Z), Find… (⌘F), Find and Replace… (⌘H)
- **Insert**: New Slide (⇧⌘N), Text Box, Image…, Shape ▸, Table…, Chart ▸,
  Comment
- **Format**: Bold (⌘B), Italic (⌘I), Underline (⌘U)
- **Slide Show**: Start Presentation
- **Help**: Keyboard Shortcuts

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
- **Shape creation**: when a geometric tool is active, click creates a
  default frame and click-drag creates the shape at the drag bounds. Horizontal
  and vertical line drags retain their axis with a thin valid frame. Text boxes
  currently support click-to-place; text-box drag creation remains planned.
- **Rotated line manipulation**: a line can be selected and moved along its
  visible direction. Its resize handles remain intentionally hidden until the
  editor has rotation-aware resize deltas, rather than exposing a resize action
  that would alter the wrong axis.
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

## Acceptance criteria

1. Done: the app has a native menu containing only wired actions.
2. Done: New/Open/Save and New Slide are visible editor controls.
3. Done: new slides are blank and persist through save, recovery, and reopen.
4. Done: pointer and keyboard shape selection expose move controls; supported
   axis-aligned shapes expose resize handles. Rotated-line resize remains the
   explicit limitation described above.
5. Done: double-click or Enter edits a text box; Escape exits editing.
6. Done: the Shape flyout arms click-or-drag geometric creation.
7. Partial: text boxes support click-to-place; drag-to-create text is planned.
8. Partial: plain textarea edits preserve existing run formatting; true
   selection-range editing needs a rich-text editing surface.
9. Done: the right-side inspector exposes style, text, arrange, notes, and
   animation controls already backed by commands.
10. Planned: slide drag reordering, context actions, duplication, deletion,
    and layout selection require their own structural PPTX commands.

## Implementation checkpoint (2026-08-03)

The desktop editor now has the interaction foundation required for a usable
local-first editing loop:

- The native menu is installed once and emits only actions handled by the
  Svelte editor. Unsupported close, clipboard, duplicate, zoom, arrange,
  rehearse, and about entries were removed rather than left as no-ops.
- The compact toolbar provides visible New Slide, Save, Text Box, Shape,
  Image, Table, Chart, Present, and formatting controls. New Slide is also
  available at the bottom of the navigator and from Insert.
- Text-box, geometric-shape, transform, animation, presenter, and rendering
  invokes use Tauri v2's camelCase JavaScript argument names. This removes the
  command-boundary failures that made inserts and edits appear non-functional.
- Canvas selection supports click-to-select, keyboard selection, Enter or
  double-click-to-edit text, drag-to-move, eight resize handles,
  Escape-to-leave text editing, and pixel-sized arrow-key nudges.
- The Shape picker now arms an explicit creation mode. A click uses the
  backend default frame; a drag sends validated EMU bounds to the undoable
  shape command. Horizontal and vertical line drags preserve their intended
  axis while using a thin serializable frame.
- New Slide is an undoable core command. Saving adds the new slide XML,
  content type, presentation relationship and ordered ID entry, and a copied
  slide-layout relationship; regression coverage reopens and edits it again.
- The right panel includes Style, Text, and Arrange inspector tabs. The Style
  tab edits a selected geometric shape's fill, outline, opacity, and shadow;
  Arrange edits position and size through the same transform command used by
  direct manipulation.

The remaining navigator work is drag reordering and the slide context menu
(duplicate/delete/layout/hide). It needs separate persistence support for
slide order and copied/deleted slide parts, and is intentionally not presented
as a completed feature.

Verification is recorded with the implementation run. No network, telemetry,
analytics, external assets, or new dependencies were added.
