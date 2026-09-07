# Wave 23 — reliable editing and modest-hardware readiness

Status: audit remediation validated locally; release qualification remains open.
Date: 2026-09-07.

## Product decision

900Slides needs dependable everyday editing before more specialized features.
The audit focused on keeping work safe, preserving editable PPTX content,
controlling avoidable memory growth, and making ordinary tasks easier to find.
The updated [competitive analysis](../COMPETITIVE_ANALYSIS.md) corrects claims
that competing tools cannot work offline. The distinction to validate is
account-free creation, dependable offline save/export/recovery, and usability
on supported modest computers.

## Audit and changes

| Finding | Resulting behavior |
| --- | --- |
| Text boxes, notes and table cells only persisted on blur. | Drafts use a serialized queue, commit after idle/max-wait bounds, remain available after command failure, and flush before document actions. |
| New/Open/Quit could bypass unsaved work; Save always opened a path dialog. | Authoritative document status, current filename, separate Save/Save As, and Save/Discard/Cancel transitions. |
| Saving truncated the destination directly. | Same-directory temporary write, synchronization and replacement; failed writes preserve the prior destination. |
| Chart invalidation missed save → undo → save, and positional source associations drifted after shape deletion. | Stable source associations and comparisons against saved chart content; round-trip regression coverage. |
| Newly created shapes wrote UUID text into PPTX fields requiring unsigned numeric IDs. | Per-slide numeric allocation reserves existing and opaque nested IDs; persisted mappings preserve editor identity, comments and animation targets. |
| Preview caches were count-only and offscreen components kept SVG strings. | 8 MiB estimated thumbnail strings/overhead with a 200-entry ceiling; offscreen release and cancelled-result guards. |
| Multiple historical media maps were retained without a byte budget. | Current media revision retained for IPC reuse; history capped at 16 MiB / three revisions and released on deck changes. |
| Shape shortcuts swallowed Space/Enter inside text fields; editing roles hid fields from accessibility APIs. | Editable descendants retain native keyboard behavior; text/table fields are exposed to accessibility tools. |
| Font size/color were dropped by the command boundary and ignored by renderers. | Explicit run attributes survive edits and render/export; defaults match logical slide typography and contrast with dark backgrounds. |
| Adding a paragraph dropped title styling; a missing font used different editing/display fallbacks. | New paragraphs inherit boundary formatting, including across draft commits; editor and readonly canvas use consistent fallback stacks. |
| Reaching 100 undo entries blocked every new edit. | Oldest history entries are evicted within count/byte limits; oversized individual transactions are rejected without partial mutation. |
| Slides could be added but not organized. | Move up/down and Delete have undo, boundary guards and structural PPTX round-trip coverage. |
| The macOS File menu was used as the application menu; predefined Quit bypassed the save prompt through native termination. | A native application menu precedes File; its custom Quit action and window close request the same unsaved-work guard. |
| Presenter/audience backgrounds were always white. | Presenter state carries the editor's theme background. |
| Production CSP omitted embedded image/font sources. | Allow local data image/font resources while retaining local script policy and blocking embedded objects. |
| Template picker offered little orientation; low-resolution dialogs could overflow. | Theme previews, category labels, filters, keyboard navigation, compact editor controls and clearer recovery choices. |
| Accessibility heuristic claimed WCAG conformance. | Labelled as a limited document check score; manual accessibility qualification remains open. |
| CI omitted frontend unit tests/build and standalone desktop tests; Linux uploads could pass with no installer. | Expanded checks, production desktop builds, explicit platform bundles and fail-on-missing artifacts. |
| Older-computer requirements were not measurable. | Fourteen explicit acceptance gates covering installation, resources, small screens, recovery, fidelity, fonts, access and user tasks. |

Cache budgets are estimates for retained JavaScript data, not process-memory
ceilings. Active source media, decoded images, rendering, Rust state and undo
history consume additional memory. Saved-deletion source XML/relationships are
retained separately while referenced by live slides or undo/redo; they are not
included in the command history's serialized-byte budget. The existing 16 MiB size gate measures the
executable only; Windows offline runtime installation is substantially larger.

## Validation

Final automated checks passed:

- Workspace formatting, clippy with warnings denied, and **405 Rust tests**.
- Separate desktop formatting, clippy with warnings denied, and **25 Rust tests**.
- **56 frontend tests**; Svelte/TypeScript check with zero errors and warnings.
- Public-release source gate and whitespace checks.
- Native development build starts successfully.

A deterministic smoke-fixture generator supplies original classroom, standard,
stress and negative PPTX inputs with manifest checksums. Its synthetic PNGs are
not substitutes for the JPEG/photo qualification workloads. Packaged build and
native interaction results follow.

### Packaged macOS smoke results

Test environment: macOS 26.6.2, arm64; a development machine, not either
low-resource qualification profile. The ad-hoc signed application starts from
its production bundle. The build-target executable checked by the size gate is
**13,089,216 bytes**, below the 16,777,216-byte regression ceiling. The signed
executable inside the application bundle is **13,031,264 bytes**; the bundle's
files total **13,034,398 bytes**, excluding filesystem allocation overhead. These
are macOS artifacts and exclude external system runtime requirements.
Bundled-executable SHA-256:
`b0969f1e749bde5ee6604921387f786025c6f013d44800fc10f35e4339a35a95`.

- Created a title using spaces and a newline; saved and reopened it. Both lines
  retain bold 36-point run properties in the PPTX, with distinct numeric shape
  IDs. An empty new paragraph retains its style across an idle draft commit.
  The final packaged canvas keeps the same missing-font fallback after reopening.
- Added and moved a second slide, deleted it, saved, undid the deletion, and
  saved again. Package inspection confirmed the restored two-slide order and
  supported editable text.
- Save updates document identity/status; cancelling Save As keeps the original
  path. Opening another file and window close offer Save/Discard/Cancel.
- The custom macOS application-menu Quit/Cmd+Q opens the same prompt; Cancel
  preserves the document. Clean-document Quit exits the process.
- Opened the generated 20-slide classroom fixture and verified local images
  on the editor canvas. Startup recovery restore/discard controls were exercised
  with test-created recovery copies.
- The editor, inspector toggle, and all six template choices remained reachable
  in a narrowed window around 1024×768. This is a limited smoke check, not a
  full 200%-scaling or assistive-technology audit.

One preview fidelity issue remains: the first classroom slide's thumbnail
omitted its image while the editor canvas and a later thumbnail displayed theirs.
The cause is not established; the separate HTML-image and inline-SVG rendering
paths need inspection. This observation does not establish lost source media.

## Remaining release gates

The source improvements do not establish a daily-driver release. The acceptance
matrix in [LOW_RESOURCE_REQUIREMENTS.md](../LOW_RESOURCE_REQUIREMENTS.md) remains
the checklist for qualification:

- Clean offline Windows/Linux installation and platform-specific save/replace
  behavior. Release workflow configuration alone is insufficient.
- OS-initiated termination (including macOS Dock Quit/log-out) can bypass the
  application-menu/window-close prompt. Recovery is a separate mitigation and
  requires forced-exit qualification; no universal shutdown-interception claim.
- Measured cold start, process-tree memory, responsiveness and idle CPU on
  actual 2 GB/4 GB target devices. No measurements from a faster machine imply
  a pass for these profiles.
- Master/layout fidelity, font substitution reporting, image-description editing,
  broader external PowerPoint/Impress round trips and script/font coverage.
  Chart regressions cover saved model/OOXML behavior, not synchronization of
  embedded Excel workbooks or every shared-chart/series-count combination.
- Full keyboard/screen-reader, increased scaling, localization and reduced-motion
  audits across the advertised platforms.
- At least five target-user task sessions before claiming the product is easy
  to use for classroom/community work.

This work changes no account requirement and adds no application network calls,
remote assets, analytics, or remote services.
