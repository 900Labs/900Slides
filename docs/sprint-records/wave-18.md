# Wave 18 — v0.4.0 accessibility checker + WCAG 2.2 AA

Status: Proposed
Owner: 900 Labs
Scope target: `docs/ROADMAP.md` v0.4.0 ("Accessibility checker. Surfaces
missing alt text, low contrast, reading-order issues, missing slide titles"
and "WCAG 2.2 AA measurement. Accessibility checker now reports a numeric
score per deck.")
Last updated: 2026-07-29

Wave 18 adds an **accessibility checker** that audits a deck for common a11y
issues and reports a **WCAG 2.2 AA conformance score**. The checker runs
offline against the deck model — no network, no browser engine.

## What this wave delivers

| # | Component | Crate / file | New vs. extend |
| --- | --- | --- | --- |
| 1 | Model | `crates/slides-core/src/lib.rs` | `alt_text` on ImageShape; heading detection |
| 2 | A11y crate | `crates/slides-core/` (module) | New: checker + WCAG scoring |
| 3 | Desktop | `apps/desktop/` | Accessibility panel: issues list + score |

## The shared contract — model changes (component 1)

### alt_text on images

Add `alt_text: Option<String>` (`#[serde(default)]`) to `ImageShape`:

```rust
pub struct ImageShape {
    pub id: String,
    pub transform: Transform,
    pub media_ref: String,
    pub crop: ...,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,
}
```

Old decks deserialize with `None` (no alt text — flagged by the checker).

No other model changes needed — the checker derives everything else from
existing model fields (text runs, theme colors, shape positions).

## Component 2 — Accessibility checker

Lives in `slides-core` as a new module (`accessibility.rs`). A pure function
that takes a `&Deck` and returns issues + a score.

```rust
/// One accessibility issue found by the checker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessibilityIssue {
    pub severity: IssueSeverity,
    pub category: IssueCategory,
    pub slide_id: Option<String>,
    pub shape_index: Option<usize>,
    pub message: String,
    pub fix_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Error,
    Warning,
    Suggestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCategory {
    MissingAltText,
    LowContrast,
    MissingTitle,
    ReadingOrder,
    SmallText,
    EmptySlide,
}

/// The result of checking a deck for accessibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessibilityReport {
    pub issues: Vec<AccessibilityIssue>,
    /// WCAG 2.2 AA conformance score (0-100). 100 = no errors or warnings.
    pub score: u32,
    pub total_slides: usize,
    pub slides_with_issues: usize,
}

/// Checks a deck for accessibility issues and returns a report.
pub fn check_accessibility(deck: &Deck) -> AccessibilityReport;
```

### Checks performed

1. **Missing alt text**: every `ImageShape` with `alt_text: None` or empty
   → Error. Fix hint: "Add descriptive alt text to this image."

2. **Low contrast**: compute the contrast ratio between text color and
   background. WCAG 2.2 AA requires 4.5:1 for normal text, 3:1 for large text
   (≥18pt or ≥14pt bold). Use the relative luminance formula from WCAG:
   `L = 0.2126*R + 0.7152*G + 0.0722*B` (with sRGB gamma correction), then
   `contrast = (L_lighter + 0.05) / (L_darker + 0.05)`. Flag text runs whose
   contrast falls below the threshold → Warning.

3. **Missing title**: a slide with no heading paragraph (H1-H6) or no text
   box in the upper portion of the slide → Warning. Fix hint: "Add a title
   to this slide."

4. **Reading order**: shapes ordered top-to-bottom, left-to-right. If a
   shape's position suggests it should be read before a preceding shape
   (e.g. a title below the body text), flag it → Suggestion.

5. **Small text**: any text run with font size below 12pt (152400 EMU) →
   Suggestion.

6. **Empty slide**: a slide with no shapes → Warning.

### WCAG 2.2 AA score

Score = 100 minus penalties:
- Each Error: -10 points (minimum score 0).
- Each Warning: -3 points.
- Each Suggestion: -1 point.
- Cap at 0. A perfect deck (no issues) scores 100.

## Component 3 — Desktop (`apps/desktop/`)

- An **Accessibility panel** accessible from the toolbar (or a menu item).
  Shows:
  - The WCAG 2.2 AA score as a number + colored badge (green ≥90, yellow
    ≥70, red <70).
  - A list of issues, grouped by category, each showing severity icon,
    slide number, message, and fix hint.
  - Click an issue → navigates to the slide and selects the shape.
  - A "Re-check" button to re-run the checker after fixes.
- A Tauri command `check_accessibility() -> AccessibilityReportDto` that
  calls `slides_core::accessibility::check_accessibility` on the current deck.

## Dependency ordering

1. **Model + checker** (components 1+2) — single worktree in slides-core.
2. **Desktop** (component 3) — after model merges.

## Acceptance criteria

1. An image without alt text is flagged as an Error.
2. Low-contrast text (e.g. light gray on white) is flagged as a Warning.
3. A slide without a title is flagged.
4. The WCAG score is a number 0-100 that decreases with issues.
5. The desktop panel shows issues with click-to-navigate.
6. Old decks (no alt_text field) deserialize unchanged.
7. Quality gate green. Privacy gate passes. No telemetry.
