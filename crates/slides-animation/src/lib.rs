//! Deterministic build and transition playback.
//!
//! This crate turns a [`slides_core::Animation`] model into a deterministic
//! timeline of [`BuildFrame`]s and exposes CSS helpers (class names and
//! `@keyframes` definitions) that the renderer can consume.
//!
//! # Determinism
//!
//! [`build_timeline`] walks the animation's steps in their declared `Vec`
//! order. It uses no `HashMap`, no sorting, and no randomization: two identical
//! [`slides_core::Animation`] inputs produce byte-identical
//! [`Vec<BuildFrame>`] output. [`css_class`] and [`keyframes`] return static
//! [`&str`] and are therefore deterministic by construction.

use std::collections::{BTreeMap, BTreeSet};

use slides_core::{Animation, BuildEffect, BuildStep, Rect, Shape, Slide, Transform};

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// A single keyframe moment in a build sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFrame {
    /// Index of the [`Animation::steps`] entry this frame was derived from.
    pub step_index: usize,
    /// Index into `slide.shapes` of the shape this frame animates.
    pub shape_index: usize,
    /// Start of the effect, in milliseconds, relative to the step's own click.
    pub start_ms: u32,
    /// End of the effect, in milliseconds, relative to the step's own click.
    pub end_ms: u32,
    /// The reveal or hide effect applied to the shape.
    pub effect: slides_core::BuildEffect,
}

/// Computes the deterministic frame sequence for a slide's build animation.
///
/// Steps are emitted in their declared order, one frame per step. Each frame's
/// `start_ms` is `0` and its `end_ms` is the step's `duration_ms` (each step
/// fires on its own presenter click, so times are local to the step). Same
/// input always yields identical output: no `HashMap` iteration is involved.
pub fn build_timeline(animation: &Animation) -> Vec<BuildFrame> {
    animation
        .steps
        .iter()
        .enumerate()
        .map(|(step_index, step)| build_frame(step_index, step))
        .collect()
}

/// Builds a single [`BuildFrame`] from an indexed build step.
fn build_frame(step_index: usize, step: &BuildStep) -> BuildFrame {
    BuildFrame {
        step_index,
        shape_index: step.shape_index,
        start_ms: 0,
        end_ms: step.duration_ms,
        effect: step.effect,
    }
}

/// The CSS class name for a build effect.
///
/// Each [`BuildEffect`] maps to a distinct, non-empty class name so the
/// renderer can tag animated shapes and reference the matching `@keyframes`.
pub fn css_class(effect: BuildEffect) -> &'static str {
    match effect {
        BuildEffect::Fade => "build-fade",
        BuildEffect::SlideInLeft => "build-slide-in-left",
        BuildEffect::SlideInRight => "build-slide-in-right",
        BuildEffect::SlideInTop => "build-slide-in-top",
        BuildEffect::SlideInBottom => "build-slide-in-bottom",
        BuildEffect::Appear => "build-appear",
        BuildEffect::Disappear => "build-disappear",
        BuildEffect::MotionPath => "build-motion-path",
    }
}

/// The CSS `@keyframes` definition for a build effect, as a deterministic
/// string.
///
/// Each definition carries a stable `@keyframes` name (matching the value
/// returned by [`css_class`]) and a `from`/`to` pair that encodes the effect's
/// direction (e.g. fading opacity or travelling in from a screen edge).
pub fn keyframes(effect: BuildEffect) -> &'static str {
    match effect {
        BuildEffect::Fade => "@keyframes build-fade { from { opacity: 0; } to { opacity: 1; } }",
        BuildEffect::SlideInLeft => {
            "@keyframes build-slide-in-left { from { transform: translateX(-100%); } to { transform: translateX(0); } }"
        }
        BuildEffect::SlideInRight => {
            "@keyframes build-slide-in-right { from { transform: translateX(100%); } to { transform: translateX(0); } }"
        }
        BuildEffect::SlideInTop => {
            "@keyframes build-slide-in-top { from { transform: translateY(-100%); } to { transform: translateY(0); } }"
        }
        BuildEffect::SlideInBottom => {
            "@keyframes build-slide-in-bottom { from { transform: translateY(100%); } to { transform: translateY(0); } }"
        }
        BuildEffect::Appear => {
            "@keyframes build-appear { from { visibility: hidden; } to { visibility: visible; } }"
        }
        BuildEffect::Disappear => {
            "@keyframes build-disappear { from { opacity: 1; } to { opacity: 0; } }"
        }
        BuildEffect::MotionPath => {
            // Motion paths are interpolated along their waypoints by the
            // trigger-aware timeline (Wave 19, component 2); this placeholder
            // keeps the keyframes registry exhaustive for component 1.
            "@keyframes build-motion-path { from { transform: translate(0, 0); } to { transform: translate(0, 0); } }"
        }
    }
}

/// The CSS class name for an individual build step, e.g. `"build-0"` for
/// `step_index == 0`.
///
/// This lets the renderer target a specific click-step independently of the
/// effect class.
pub fn css_class_for_step(step_index: usize) -> String {
    format!("build-{step_index}")
}

/// A morph frame: one shape's interpolation from the previous slide to the
/// next.
///
/// Deterministic: the same slide pair always produces identical frames.
#[derive(Debug, Clone, PartialEq)]
pub struct MorphFrame {
    /// The shape id being morphed (matches on both slides).
    pub shape_id: String,
    /// Source transform (on the previous slide). `None` if the shape is new on
    /// the next slide (fade-in).
    pub from: Option<MorphTransform>,
    /// Target transform (on the next slide). `None` if the shape is being
    /// removed (fade-out).
    pub to: Option<MorphTransform>,
}

/// Interpolatable transform state for a morph.
#[derive(Debug, Clone, PartialEq)]
pub struct MorphTransform {
    /// Horizontal position, in EMU.
    pub x: f64,
    /// Vertical position, in EMU.
    pub y: f64,
    /// Width, in EMU.
    pub width: f64,
    /// Height, in EMU.
    pub height: f64,
    /// Rotation around the frame center, in degrees.
    pub rotation: f64,
}

/// Computes the morph frame sequence for a transition from `prev` to `next`.
///
/// Matches shapes by stable id. Shapes present on both slides interpolate
/// (`from: Some`, `to: Some`); shapes only on `next` fade in
/// (`from: None`, `to: Some`); shapes only on `prev` fade out
/// (`from: Some`, `to: None`).
///
/// [`Shape::Passthrough`] objects and shapes with an empty id are skipped.
/// Deterministic: the matching pass uses [`BTreeMap`]s and the union of ids is
/// drawn from a [`BTreeSet`], so the result is sorted by `shape_id`
/// alphabetically — no [`HashMap`](std::collections::HashMap) iteration is
/// involved in the output ordering.
pub fn morph_timeline(prev: &Slide, next: &Slide) -> Vec<MorphFrame> {
    let prev_map = collect_transforms(prev);
    let next_map = collect_transforms(next);

    let mut ids: BTreeSet<String> = BTreeSet::new();
    ids.extend(prev_map.keys().cloned());
    ids.extend(next_map.keys().cloned());

    ids.into_iter()
        .map(|id| {
            let from = prev_map.get(&id).cloned();
            let to = next_map.get(&id).cloned();
            MorphFrame {
                shape_id: id,
                from,
                to,
            }
        })
        .collect()
}

/// Collects the id-addressed morph transforms for a slide's shapes.
///
/// [`Shape::Passthrough`] objects and shapes with an empty id are skipped. The
/// result is a [`BTreeMap`] so iteration order is stable.
fn collect_transforms(slide: &Slide) -> BTreeMap<String, MorphTransform> {
    let mut map = BTreeMap::new();
    for shape in &slide.shapes {
        let id = shape.id();
        if id.is_empty() {
            continue;
        }
        if let Some(transform) = shape_transform(shape) {
            map.insert(id.to_string(), transform);
        }
    }
    map
}

/// Extracts a [`MorphTransform`] from a shape, or `None` for
/// [`Shape::Passthrough`].
///
/// [`Shape::TextBox`] carries only a [`Rect`] (no rotation), so its rotation is
/// `0.0`; every other editable variant exposes a [`Transform`].
fn shape_transform(shape: &Shape) -> Option<MorphTransform> {
    match shape {
        Shape::TextBox(text_box) => Some(rect_to_morph(&text_box.frame, 0.0)),
        Shape::Image(image) => Some(transform_to_morph(&image.transform)),
        Shape::Geometric(geometric) => Some(transform_to_morph(&geometric.transform)),
        Shape::Table(table) => Some(transform_to_morph(&table.transform)),
        Shape::Chart(chart) => Some(transform_to_morph(&chart.transform)),
        Shape::Passthrough(_) => None,
    }
}

/// Builds a [`MorphTransform`] from a [`Rect`] and an explicit rotation.
fn rect_to_morph(frame: &Rect, rotation: f64) -> MorphTransform {
    MorphTransform {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: frame.height,
        rotation,
    }
}

/// Builds a [`MorphTransform`] from a shape [`Transform`].
fn transform_to_morph(transform: &Transform) -> MorphTransform {
    rect_to_morph(&transform.frame, transform.rotation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slides_core::{
        Animation, BuildEffect, BuildStep, GeometricShape, Geometry, PassthroughObject, Rect,
        Shape, Style, TextBox, Transform,
    };

    /// Builds a slide carrying `shapes`, with all other fields defaulted.
    fn slide(shapes: Vec<Shape>) -> Slide {
        Slide {
            shapes,
            ..Slide::default()
        }
    }

    #[test]
    fn deterministic_timeline_same_input_same_output() {
        let animation = Animation::new(vec![
            BuildStep::new(0, BuildEffect::Fade, 500),
            BuildStep::new(1, BuildEffect::SlideInLeft, 1000),
            BuildStep::new(2, BuildEffect::Disappear, 250),
        ]);
        let first = build_timeline(&animation);
        let second = build_timeline(&animation);
        assert_eq!(first, second);
    }

    #[test]
    fn build_timeline_correct_times() {
        let animation = Animation::new(vec![
            BuildStep::new(0, BuildEffect::Fade, 500),
            BuildStep::new(1, BuildEffect::Appear, 1000),
            BuildStep::new(2, BuildEffect::SlideInRight, 250),
        ]);
        let timeline = build_timeline(&animation);

        assert_eq!(timeline.len(), 3);

        // Each step starts at 0 relative to its own click and ends at its duration.
        assert_eq!(timeline[0].step_index, 0);
        assert_eq!(timeline[0].start_ms, 0);
        assert_eq!(timeline[0].end_ms, 500);

        assert_eq!(timeline[1].step_index, 1);
        assert_eq!(timeline[1].start_ms, 0);
        assert_eq!(timeline[1].end_ms, 1000);

        assert_eq!(timeline[2].step_index, 2);
        assert_eq!(timeline[2].start_ms, 0);
        assert_eq!(timeline[2].end_ms, 250);
    }

    #[test]
    fn build_timeline_empty_animation() {
        let animation = Animation::new(vec![]);
        let timeline = build_timeline(&animation);
        assert!(timeline.is_empty());
    }

    #[test]
    fn css_class_each_effect() {
        let classes = [
            css_class(BuildEffect::Fade),
            css_class(BuildEffect::SlideInLeft),
            css_class(BuildEffect::SlideInRight),
            css_class(BuildEffect::SlideInTop),
            css_class(BuildEffect::SlideInBottom),
            css_class(BuildEffect::Appear),
            css_class(BuildEffect::Disappear),
        ];

        for class in classes {
            assert!(!class.is_empty(), "empty css class");
        }
        // Every effect must map to a distinct class name.
        for i in 0..classes.len() {
            for j in (i + 1)..classes.len() {
                assert_ne!(classes[i], classes[j], "duplicate css class");
            }
        }
    }

    #[test]
    fn keyframes_each_effect() {
        let effects = [
            BuildEffect::Fade,
            BuildEffect::SlideInLeft,
            BuildEffect::SlideInRight,
            BuildEffect::SlideInTop,
            BuildEffect::SlideInBottom,
            BuildEffect::Appear,
            BuildEffect::Disappear,
        ];

        for effect in effects {
            let kf = keyframes(effect);
            assert!(
                kf.contains("@keyframes"),
                "missing @keyframes for {effect:?}"
            );
        }

        // Directional checks: each effect encodes its travel direction.
        assert!(keyframes(BuildEffect::Fade).contains("opacity: 0"));
        assert!(keyframes(BuildEffect::Fade).contains("opacity: 1"));
        assert!(keyframes(BuildEffect::SlideInLeft).contains("translateX(-100%)"));
        assert!(keyframes(BuildEffect::SlideInRight).contains("translateX(100%)"));
        assert!(keyframes(BuildEffect::SlideInTop).contains("translateY(-100%)"));
        assert!(keyframes(BuildEffect::SlideInBottom).contains("translateY(100%)"));
        assert!(keyframes(BuildEffect::Appear).contains("visibility: hidden"));
        assert!(keyframes(BuildEffect::Appear).contains("visibility: visible"));
        assert!(keyframes(BuildEffect::Disappear).contains("opacity: 1"));
        assert!(keyframes(BuildEffect::Disappear).contains("opacity: 0"));
    }

    #[test]
    fn css_class_for_step_increments() {
        assert_eq!(css_class_for_step(0), "build-0");
        assert_eq!(css_class_for_step(1), "build-1");
        assert_eq!(css_class_for_step(3), "build-3");
    }

    #[test]
    fn disappear_effect_handled() {
        let animation = Animation::new(vec![BuildStep::new(2, BuildEffect::Disappear, 400)]);
        let timeline = build_timeline(&animation);

        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].step_index, 0);
        assert_eq!(timeline[0].shape_index, 2);
        assert_eq!(timeline[0].start_ms, 0);
        assert_eq!(timeline[0].end_ms, 400);
        assert_eq!(timeline[0].effect, BuildEffect::Disappear);
    }

    #[test]
    fn morph_timeline_matching_shape_interpolates() {
        let prev = slide(vec![Shape::Geometric(GeometricShape {
            id: "shape-1".to_string(),
            transform: Transform {
                frame: Rect::new(0.0, 0.0, 100.0, 50.0),
                rotation: 0.0,
            },
            geometry: Geometry::Rectangle,
            style: Style::default(),
        })]);
        let next = slide(vec![Shape::Geometric(GeometricShape {
            id: "shape-1".to_string(),
            transform: Transform {
                frame: Rect::new(200.0, 100.0, 100.0, 50.0),
                rotation: 45.0,
            },
            geometry: Geometry::Rectangle,
            style: Style::default(),
        })]);

        let frames = morph_timeline(&prev, &next);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].shape_id, "shape-1");
        assert_eq!(
            frames[0].from,
            Some(MorphTransform {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
                rotation: 0.0,
            })
        );
        assert_eq!(
            frames[0].to,
            Some(MorphTransform {
                x: 200.0,
                y: 100.0,
                width: 100.0,
                height: 50.0,
                rotation: 45.0,
            })
        );
    }

    #[test]
    fn morph_timeline_new_shape_fades_in() {
        let prev = slide(vec![]);
        let next = slide(vec![Shape::TextBox(TextBox {
            id: "shape-1".to_string(),
            frame: Rect::new(10.0, 20.0, 30.0, 40.0),
            paragraphs: vec![],
        })]);

        let frames = morph_timeline(&prev, &next);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].shape_id, "shape-1");
        assert!(frames[0].from.is_none());
        assert!(frames[0].to.is_some());
    }

    #[test]
    fn morph_timeline_removed_shape_fades_out() {
        let prev = slide(vec![Shape::TextBox(TextBox {
            id: "shape-1".to_string(),
            frame: Rect::new(10.0, 20.0, 30.0, 40.0),
            paragraphs: vec![],
        })]);
        let next = slide(vec![]);

        let frames = morph_timeline(&prev, &next);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].shape_id, "shape-1");
        assert!(frames[0].from.is_some());
        assert!(frames[0].to.is_none());
    }

    #[test]
    fn morph_timeline_empty_ids_skipped() {
        let prev = slide(vec![Shape::TextBox(TextBox {
            id: String::new(),
            frame: Rect::new(0.0, 0.0, 10.0, 10.0),
            paragraphs: vec![],
        })]);
        let next = slide(vec![Shape::TextBox(TextBox {
            id: String::new(),
            frame: Rect::new(1.0, 1.0, 10.0, 10.0),
            paragraphs: vec![],
        })]);

        let frames = morph_timeline(&prev, &next);

        assert!(frames.is_empty(), "shapes with empty ids must not match");
    }

    #[test]
    fn morph_timeline_passthrough_skipped() {
        let prev = slide(vec![Shape::Passthrough(PassthroughObject {
            id: "passthrough-1".to_string(),
            label: String::new(),
            source_part: String::new(),
            raw_bytes: vec![],
            frame: Some(Rect::new(0.0, 0.0, 10.0, 10.0)),
        })]);
        let next = slide(vec![Shape::Passthrough(PassthroughObject {
            id: "passthrough-1".to_string(),
            label: String::new(),
            source_part: String::new(),
            raw_bytes: vec![],
            frame: Some(Rect::new(5.0, 5.0, 10.0, 10.0)),
        })]);

        let frames = morph_timeline(&prev, &next);

        assert!(frames.is_empty(), "passthrough shapes must not be included");
    }

    #[test]
    fn morph_timeline_deterministic() {
        let make = || {
            slide(vec![
                Shape::TextBox(TextBox {
                    id: "charlie".to_string(),
                    frame: Rect::new(0.0, 0.0, 10.0, 10.0),
                    paragraphs: vec![],
                }),
                Shape::TextBox(TextBox {
                    id: "alpha".to_string(),
                    frame: Rect::new(1.0, 1.0, 10.0, 10.0),
                    paragraphs: vec![],
                }),
                Shape::TextBox(TextBox {
                    id: "bravo".to_string(),
                    frame: Rect::new(2.0, 2.0, 10.0, 10.0),
                    paragraphs: vec![],
                }),
            ])
        };
        let prev = make();
        let next = make();

        let first = morph_timeline(&prev, &next);
        let second = morph_timeline(&prev, &next);

        assert_eq!(first, second);
        let ids: Vec<&str> = first.iter().map(|f| f.shape_id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn morph_timeline_no_matching_ids_returns_empty() {
        let prev = slide(vec![Shape::TextBox(TextBox {
            id: "only-prev".to_string(),
            frame: Rect::new(0.0, 0.0, 10.0, 10.0),
            paragraphs: vec![],
        })]);
        let next = slide(vec![Shape::TextBox(TextBox {
            id: "only-next".to_string(),
            frame: Rect::new(5.0, 5.0, 10.0, 10.0),
            paragraphs: vec![],
        })]);

        let frames = morph_timeline(&prev, &next);

        // No shape id is shared, so there are no interpolation (both-sided) frames.
        assert!(
            frames.iter().all(|f| !(f.from.is_some() && f.to.is_some())),
            "no matching ids means no interpolation frames"
        );
        // Each slide's shape surfaces as a single-sided frame, sorted by id.
        let ids: Vec<&str> = frames.iter().map(|f| f.shape_id.as_str()).collect();
        assert_eq!(ids, vec!["only-next", "only-prev"]);
    }
}
