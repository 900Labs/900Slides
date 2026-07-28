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

use slides_core::{Animation, BuildEffect, BuildStep};

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

#[cfg(test)]
mod tests {
    use super::*;
    use slides_core::{Animation, BuildEffect, BuildStep};

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
}
