//! Accessibility checker and WCAG 2.2 AA scoring.
//!
//! [`check_accessibility`] audits a [`Deck`] offline (no network, no browser
//! engine) for common accessibility issues — missing image alt text, low text
//! contrast, missing slide titles, poor reading order, undersized text, and
//! empty slides — and rolls them up into a 0–100 WCAG 2.2 AA conformance score.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{Color, Deck, Shape, Slide};

/// Font size, in EMU, at and above which WCAG treats non-bold text as "large"
/// (18 pt). 1 pt = 12,700 EMU.
const LARGE_TEXT_EMU: f64 = 228_600.0;
/// Font size, in EMU, at and above which WCAG treats *bold* text as "large"
/// (14 pt bold).
const LARGE_BOLD_TEXT_EMU: f64 = 177_800.0;
/// Minimum body text size, in EMU, before the checker flags it as too small
/// (12 pt).
const SMALL_TEXT_EMU: f64 = 152_400.0;
/// Fallback slide height, in EMU, used for reading-order analysis when a deck
/// does not pin its [`crate::SlideSize`] (standard 16:9 height).
const DEFAULT_SLIDE_HEIGHT_EMU: f64 = 6_858_000.0;
/// WCAG 2.2 AA contrast threshold for normal-sized text.
const NORMAL_TEXT_THRESHOLD: f64 = 4.5;
/// WCAG 2.2 AA contrast threshold for large text (≥18 pt, or ≥14 pt bold).
const LARGE_TEXT_THRESHOLD: f64 = 3.0;

/// One accessibility issue found by the checker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessibilityIssue {
    /// How severe the issue is.
    pub severity: IssueSeverity,
    /// What kind of issue this is.
    pub category: IssueCategory,
    /// Id of the slide the issue was found on, if applicable.
    pub slide_id: Option<String>,
    /// Index of the offending shape within the slide, if applicable.
    pub shape_index: Option<usize>,
    /// Human-readable description of the issue.
    pub message: String,
    /// Optional suggested remediation.
    pub fix_hint: Option<String>,
}

/// Severity of an [`AccessibilityIssue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    /// A blocking accessibility failure (e.g. an image with no alt text).
    Error,
    /// A likely problem that should be fixed (e.g. low text contrast).
    Warning,
    /// A minor improvement (e.g. slightly small text, reading order).
    Suggestion,
}

/// Category of an [`AccessibilityIssue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCategory {
    /// An image is missing alt text.
    MissingAltText,
    /// Text contrast falls below the WCAG 2.2 AA threshold.
    LowContrast,
    /// A slide has no title heading.
    MissingTitle,
    /// The slide's reading order is unclear.
    ReadingOrder,
    /// Body text is smaller than 12 pt.
    SmallText,
    /// A slide has no content.
    EmptySlide,
}

/// The result of checking a deck for accessibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessibilityReport {
    /// Every issue found, in document order.
    pub issues: Vec<AccessibilityIssue>,
    /// WCAG 2.2 AA conformance score (0–100). 100 means no issues.
    pub score: u32,
    /// Total number of slides in the deck.
    pub total_slides: usize,
    /// Number of slides that have at least one issue.
    pub slides_with_issues: usize,
}

/// Checks a deck for accessibility issues and returns a report with a WCAG 2.2
/// AA conformance score.
///
/// Runs entirely against the in-memory deck model — no telemetry, no network.
#[must_use]
pub fn check_accessibility(deck: &Deck) -> AccessibilityReport {
    let background = deck.theme.background;
    let slide_height = deck
        .slide_size
        .as_ref()
        .map_or(DEFAULT_SLIDE_HEIGHT_EMU, |size| size.height_emu);

    let mut issues = Vec::new();
    for slide in &deck.slides {
        check_slide(slide, background, slide_height, &mut issues);
    }

    let slides_with_issues = issues
        .iter()
        .filter_map(|issue| issue.slide_id.as_ref())
        .collect::<HashSet<_>>()
        .len();

    AccessibilityReport {
        score: compute_score(&issues),
        issues,
        total_slides: deck.slides.len(),
        slides_with_issues,
    }
}

/// Runs all per-slide checks, appending issues to `issues`.
fn check_slide(
    slide: &Slide,
    background: Color,
    slide_height: f64,
    issues: &mut Vec<AccessibilityIssue>,
) {
    let slide_id = slide.id.clone();

    // Empty slide.
    if slide.shapes.is_empty() {
        issues.push(AccessibilityIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::EmptySlide,
            slide_id: Some(slide_id.clone()),
            shape_index: None,
            message: "Slide has no content.".to_string(),
            fix_hint: Some("Add shapes or text to this slide.".to_string()),
        });
    }

    let mut has_heading = false;
    // Top-left positions (y, x) of text boxes that carry visible text, used to
    // assess reading order.
    let mut text_box_origins: Vec<(f64, f64)> = Vec::new();

    for (index, shape) in slide.shapes.iter().enumerate() {
        match shape {
            Shape::Image(image) => {
                let missing = image
                    .alt_text
                    .as_deref()
                    .is_none_or(|text| text.trim().is_empty());
                if missing {
                    issues.push(AccessibilityIssue {
                        severity: IssueSeverity::Error,
                        category: IssueCategory::MissingAltText,
                        slide_id: Some(slide_id.clone()),
                        shape_index: Some(index),
                        message: "Image is missing alt text.".to_string(),
                        fix_hint: Some("Add descriptive alt text to this image.".to_string()),
                    });
                }
            }
            Shape::TextBox(text_box) => {
                let mut has_text = false;
                for paragraph in &text_box.paragraphs {
                    if paragraph.style.heading.is_some() {
                        has_heading = true;
                    }
                    for run in &paragraph.runs {
                        if run.text.trim().is_empty() {
                            continue;
                        }
                        has_text = true;

                        // Small text (checked before contrast so the size is
                        // still available for the large-text threshold below).
                        if let Some(font_size) = run.font_size {
                            if font_size < SMALL_TEXT_EMU {
                                issues.push(AccessibilityIssue {
                                    severity: IssueSeverity::Suggestion,
                                    category: IssueCategory::SmallText,
                                    slide_id: Some(slide_id.clone()),
                                    shape_index: Some(index),
                                    message: format!(
                                        "Text font size is below 12 pt ({:.0} EMU).",
                                        font_size
                                    ),
                                    fix_hint: Some(
                                        "Increase the font size to at least 12 pt.".to_string(),
                                    ),
                                });
                            }
                        }

                        // Low contrast.
                        let text_color = run.color.unwrap_or(Color::black());
                        let is_large = run.font_size.is_some_and(|font_size| {
                            font_size >= LARGE_TEXT_EMU
                                || (run.bold && font_size >= LARGE_BOLD_TEXT_EMU)
                        });
                        let threshold = if is_large {
                            LARGE_TEXT_THRESHOLD
                        } else {
                            NORMAL_TEXT_THRESHOLD
                        };
                        let ratio = contrast_ratio(&text_color, &background);
                        if ratio < threshold {
                            issues.push(AccessibilityIssue {
                                severity: IssueSeverity::Warning,
                                category: IssueCategory::LowContrast,
                                slide_id: Some(slide_id.clone()),
                                shape_index: Some(index),
                                message: format!(
                                    "Text contrast ratio is {:.1}:1, below the WCAG AA minimum of {:.1}:1.",
                                    ratio, threshold
                                ),
                                fix_hint: Some(
                                    "Use a darker or lighter text color for sufficient contrast."
                                        .to_string(),
                                ),
                            });
                        }
                    }
                }
                if has_text {
                    text_box_origins.push((text_box.frame.y, text_box.frame.x));
                }
            }
            _ => {}
        }
    }

    // Missing title.
    if !has_heading {
        issues.push(AccessibilityIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::MissingTitle,
            slide_id: Some(slide_id.clone()),
            shape_index: None,
            message: "Slide is missing a title heading.".to_string(),
            fix_hint: Some("Add a title to this slide.".to_string()),
        });
    }

    // Reading order: the first text box (top-to-bottom, left-to-right) should
    // sit within the top third of the slide.
    if !text_box_origins.is_empty() {
        text_box_origins.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });
        let (first_y, _) = text_box_origins[0];
        if first_y >= slide_height / 3.0 {
            issues.push(AccessibilityIssue {
                severity: IssueSeverity::Suggestion,
                category: IssueCategory::ReadingOrder,
                slide_id: Some(slide_id),
                shape_index: None,
                message: "The first text on the slide is not in the top third; reading order may be unclear.".to_string(),
                fix_hint: Some(
                    "Place the title or lead text near the top of the slide.".to_string(),
                ),
            });
        }
    }
}

/// Computes the WCAG 2.2 AA score: 100 minus weighted penalties, floored at 0.
fn compute_score(issues: &[AccessibilityIssue]) -> u32 {
    let mut errors = 0u32;
    let mut warnings = 0u32;
    let mut suggestions = 0u32;
    for issue in issues {
        match issue.severity {
            IssueSeverity::Error => errors += 1,
            IssueSeverity::Warning => warnings += 1,
            IssueSeverity::Suggestion => suggestions += 1,
        }
    }
    100u32.saturating_sub(errors * 10 + warnings * 3 + suggestions)
}

/// Relative luminance of an sRGB color, per WCAG 2.2.
fn relative_luminance(color: &Color) -> f64 {
    fn channel(c: u8) -> f64 {
        let s = f64::from(c) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

/// WCAG contrast ratio between two colors: `(L_lighter + 0.05) / (L_darker + 0.05)`.
fn contrast_ratio(a: &Color, b: &Color) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let lighter = la.max(lb);
    let darker = la.min(lb);
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::{check_accessibility, IssueCategory, IssueSeverity, LARGE_TEXT_EMU};
    use crate::{
        Color, Deck, HeadingLevel, ImageShape, Paragraph, ParagraphStyle, Rect, Run, Shape, Slide,
        TextBox, Transform,
    };

    /// A run whose text is treated as a heading (H1).
    fn heading_run(text: &str) -> Run {
        Run::new(text)
    }

    fn text_box_at(y: f64, paragraphs: Vec<Paragraph>) -> TextBox {
        TextBox {
            id: String::new(),
            frame: Rect::new(0.0, y, 1_000_000.0, 500_000.0),
            paragraphs,
        }
    }

    fn heading_paragraph(text: &str) -> Paragraph {
        Paragraph {
            runs: vec![heading_run(text)],
            style: ParagraphStyle {
                heading: Some(HeadingLevel::H1),
                ..ParagraphStyle::default()
            },
            ..Paragraph::default()
        }
    }

    fn body_paragraph(run: Run) -> Paragraph {
        Paragraph {
            runs: vec![run],
            ..Paragraph::default()
        }
    }

    fn single_slide_deck(shapes: Vec<Shape>) -> Deck {
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            shapes,
            ..Slide::default()
        });
        deck
    }

    fn image_without_alt() -> Shape {
        Shape::Image(ImageShape {
            id: "img-1".to_string(),
            transform: Transform::default(),
            media_ref: "img1".to_string(),
            crop: None,
            alt_text: None,
        })
    }

    fn image_with_alt(text: &str) -> Shape {
        Shape::Image(ImageShape {
            id: "img-1".to_string(),
            transform: Transform::default(),
            media_ref: "img1".to_string(),
            crop: None,
            alt_text: Some(text.to_string()),
        })
    }

    fn count_issues(report: &super::AccessibilityReport, category: IssueCategory) -> usize {
        report
            .issues
            .iter()
            .filter(|issue| issue.category == category)
            .count()
    }

    #[test]
    fn missing_alt_text_flagged() {
        let deck = single_slide_deck(vec![image_without_alt()]);
        let report = check_accessibility(&deck);
        let missing = report
            .issues
            .iter()
            .filter(|issue| {
                issue.category == IssueCategory::MissingAltText
                    && issue.severity == IssueSeverity::Error
            })
            .count();
        assert_eq!(missing, 1, "an image without alt text must be one Error");
    }

    #[test]
    fn alt_text_present_not_flagged() {
        let deck = single_slide_deck(vec![
            image_with_alt("A diagram of the build pipeline"),
            Shape::TextBox(text_box_at(0.0, vec![heading_paragraph("Title")])),
        ]);
        let report = check_accessibility(&deck);
        assert_eq!(
            count_issues(&report, IssueCategory::MissingAltText),
            0,
            "an image with alt text must not be flagged"
        );
    }

    #[test]
    fn empty_alt_text_is_treated_as_missing() {
        let deck = single_slide_deck(vec![image_with_alt("   ")]);
        let report = check_accessibility(&deck);
        assert_eq!(
            count_issues(&report, IssueCategory::MissingAltText),
            1,
            "whitespace-only alt text counts as missing"
        );
    }

    #[test]
    fn low_contrast_flagged() {
        // Light gray (#cccccc) on the default white background is well below
        // the 4.5:1 AA threshold for normal text.
        let run = Run::new("faint").color(Color::rgb(204, 204, 204));
        let deck = single_slide_deck(vec![Shape::TextBox(text_box_at(
            0.0,
            vec![Paragraph {
                runs: vec![run],
                style: ParagraphStyle {
                    heading: Some(HeadingLevel::H1),
                    ..ParagraphStyle::default()
                },
                ..Paragraph::default()
            }],
        ))]);
        let report = check_accessibility(&deck);
        assert_eq!(
            count_issues(&report, IssueCategory::LowContrast),
            1,
            "low-contrast text must be flagged as a Warning"
        );
        assert!(report.score < 100);
    }

    #[test]
    fn high_contrast_not_flagged() {
        let deck = single_slide_deck(vec![Shape::TextBox(text_box_at(
            0.0,
            vec![heading_paragraph("Title")],
        ))]);
        let report = check_accessibility(&deck);
        assert_eq!(
            count_issues(&report, IssueCategory::LowContrast),
            0,
            "black text on white must not be flagged"
        );
    }

    #[test]
    fn large_text_uses_lower_threshold() {
        // #808080 on white is ~3.95:1 — below 4.5:1 (normal threshold) but
        // above 3:1 (large-text threshold). A 24 pt run should NOT be flagged.
        let run = Run::new("big")
            .color(Color::rgb(128, 128, 128))
            .font_size(304_800.0); // 24 pt = 24 * 12700
        let deck = single_slide_deck(vec![Shape::TextBox(text_box_at(
            0.0,
            vec![Paragraph {
                runs: vec![run],
                style: ParagraphStyle {
                    heading: Some(HeadingLevel::H1),
                    ..ParagraphStyle::default()
                },
                ..Paragraph::default()
            }],
        ))]);
        let report = check_accessibility(&deck);
        assert_eq!(
            count_issues(&report, IssueCategory::LowContrast),
            0,
            "large text uses the 3:1 threshold"
        );
    }

    #[test]
    fn missing_title_flagged() {
        let deck = single_slide_deck(vec![Shape::TextBox(text_box_at(
            0.0,
            vec![body_paragraph(Run::new("body text"))],
        ))]);
        let report = check_accessibility(&deck);
        assert_eq!(
            count_issues(&report, IssueCategory::MissingTitle),
            1,
            "a slide with text but no heading is flagged"
        );
    }

    #[test]
    fn title_present_not_flagged() {
        let deck = single_slide_deck(vec![Shape::TextBox(text_box_at(
            0.0,
            vec![heading_paragraph("Title")],
        ))]);
        let report = check_accessibility(&deck);
        assert_eq!(
            count_issues(&report, IssueCategory::MissingTitle),
            0,
            "a slide with an H1 is not flagged"
        );
    }

    #[test]
    fn empty_slide_flagged() {
        let deck = single_slide_deck(Vec::new());
        let report = check_accessibility(&deck);
        assert_eq!(
            count_issues(&report, IssueCategory::EmptySlide),
            1,
            "a slide with no shapes is flagged"
        );
    }

    #[test]
    fn small_text_flagged() {
        let run = Run::new("tiny").font_size(100_000.0); // < 12 pt (152,400 EMU)
        let deck = single_slide_deck(vec![Shape::TextBox(text_box_at(
            0.0,
            vec![Paragraph {
                runs: vec![run],
                style: ParagraphStyle {
                    heading: Some(HeadingLevel::H1),
                    ..ParagraphStyle::default()
                },
                ..Paragraph::default()
            }],
        ))]);
        let report = check_accessibility(&deck);
        let small = report
            .issues
            .iter()
            .filter(|issue| {
                issue.category == IssueCategory::SmallText
                    && issue.severity == IssueSeverity::Suggestion
            })
            .count();
        assert_eq!(small, 1, "text under 12 pt must be a Suggestion");
    }

    #[test]
    fn reading_order_flagged_when_first_text_is_low() {
        // The only text box sits well past the top third of a 16:9 slide.
        let deck = single_slide_deck(vec![Shape::TextBox(text_box_at(
            5_000_000.0,
            vec![heading_paragraph("Title")],
        ))]);
        let report = check_accessibility(&deck);
        assert_eq!(
            count_issues(&report, IssueCategory::ReadingOrder),
            1,
            "lead text outside the top third is a reading-order suggestion"
        );
    }

    #[test]
    fn score_decreases_with_issues() {
        let deck = single_slide_deck(vec![image_without_alt()]);
        let report = check_accessibility(&deck);
        assert!(
            report.score < 100,
            "a deck with issues must score below 100 (got {})",
            report.score
        );
        assert!(report.slides_with_issues >= 1);
    }

    #[test]
    fn score_floors_at_zero() {
        // Many errors drive the penalty past 100; the score must clamp at 0.
        let images: Vec<Shape> = (0..20).map(|_| image_without_alt()).collect();
        let deck = single_slide_deck(images);
        let report = check_accessibility(&deck);
        assert_eq!(report.score, 0, "score is floored at 0");
    }

    #[test]
    fn perfect_deck_scores_100() {
        let deck = single_slide_deck(vec![
            Shape::TextBox(text_box_at(0.0, vec![heading_paragraph("Title")])),
            image_with_alt("A supporting diagram"),
        ]);
        let report = check_accessibility(&deck);
        assert!(
            report.issues.is_empty(),
            "found unexpected issues: {:?}",
            report.issues
        );
        assert_eq!(report.score, 100, "a clean deck scores 100");
        assert_eq!(report.total_slides, 1);
        assert_eq!(report.slides_with_issues, 0);
    }

    #[test]
    fn old_deck_without_alt_text_deserializes() {
        // Build a deck that uses the new fields, serialize it, then strip the
        // additive fields (alt_text, color, font_size) to simulate an old deck
        // and confirm it deserializes with None defaults.
        let mut deck = Deck::new();
        deck.slides.push(Slide {
            id: "s1".to_string(),
            shapes: vec![
                Shape::Image(ImageShape {
                    id: "img-1".to_string(),
                    transform: Transform::default(),
                    media_ref: "img1".to_string(),
                    crop: None,
                    alt_text: Some("original description".to_string()),
                }),
                Shape::TextBox(TextBox {
                    id: "tb-1".to_string(),
                    frame: Rect::new(0.0, 0.0, 1_000_000.0, 500_000.0),
                    paragraphs: vec![Paragraph {
                        runs: vec![Run::new("hello")
                            .color(Color::rgb(255, 0, 0))
                            .font_size(200_000.0)],
                        ..Paragraph::default()
                    }],
                }),
            ],
            ..Slide::default()
        });

        let mut value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&deck).expect("serialize"))
                .expect("parse json");
        for shape in value["slides"][0]["shapes"].as_array_mut().expect("shapes") {
            let value_obj = shape["value"].as_object_mut().expect("shape value");
            value_obj.remove("alt_text");
            if let Some(paras) = value_obj
                .get_mut("paragraphs")
                .and_then(|p| p.as_array_mut())
            {
                for paragraph in paras {
                    if let Some(runs) = paragraph["runs"].as_array_mut() {
                        for run in runs {
                            let run_obj = run.as_object_mut().expect("run");
                            run_obj.remove("color");
                            run_obj.remove("font_size");
                        }
                    }
                }
            }
        }

        let reparsed: Deck =
            serde_json::from_str(&serde_json::to_string(&value).expect("re-serialize"))
                .expect("old deck deserializes");

        match &reparsed.slides[0].shapes[0] {
            Shape::Image(image) => assert_eq!(image.alt_text, None),
            _ => panic!("expected an image"),
        }
        match &reparsed.slides[0].shapes[1] {
            Shape::TextBox(text_box) => {
                let run = &text_box.paragraphs[0].runs[0];
                assert_eq!(run.color, None);
                assert_eq!(run.font_size, None);
            }
            _ => panic!("expected a text box"),
        }
    }
}
