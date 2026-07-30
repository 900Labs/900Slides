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

use slides_core::{Animation, BuildEffect, BuildStep, Rect, Shape, Slide, Transform, Trigger};

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// A single keyframe moment in a build sequence.
///
/// `start_ms`/`end_ms` are expressed relative to the start of the step's
/// [`click_group`](Self::click_group) (i.e. relative to the presenter click
/// that triggers that group), so a [`Trigger::WithPrevious`] or
/// [`Trigger::AfterPrevious`] step can carry a non-zero `start_ms` while still
/// sharing the previous step's click group.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildFrame {
    /// Index of the [`Animation::steps`] entry this frame was derived from.
    pub step_index: usize,
    /// Index into `slide.shapes` of the shape this frame animates.
    pub shape_index: usize,
    /// Which presenter click triggers this step: `0` for the first click
    /// group, `1` for the second, and so on. [`Trigger::WithPrevious`] and
    /// [`Trigger::AfterPrevious`] steps inherit the previous step's group.
    pub click_group: usize,
    /// Start of the effect, in milliseconds, relative to the group's click.
    pub start_ms: u32,
    /// End of the effect, in milliseconds, relative to the group's click.
    pub end_ms: u32,
    /// Delay (in ms) this step introduces after its trigger reference point
    /// (the click for [`Trigger::OnClick`], the previous start for
    /// [`Trigger::WithPrevious`], the previous end for
    /// [`Trigger::AfterPrevious`]). Stored separately from `start_ms` so the
    /// renderer can report per-step delay.
    pub offset_ms: u32,
    /// The reveal or hide effect applied to the shape.
    pub effect: slides_core::BuildEffect,
    /// For a [`BuildEffect::MotionPath`] segment, the `(from, to)` waypoint
    /// pair this frame linearly interpolates between (in EMU). `None` for
    /// non-motion frames.
    pub motion_path: Option<(Rect, Rect)>,
}

/// Computes the deterministic, trigger-aware frame sequence for a slide's
/// build animation.
///
/// Walks `animation.steps` in declared order, honoring each step's `trigger`:
/// - [`Trigger::OnClick`] begins a new click group; the step starts at
///   `delay_ms` within that group.
/// - [`Trigger::WithPrevious`] joins the previous step's click group and starts
///   at `previous.start_ms + delay_ms`.
/// - [`Trigger::AfterPrevious`] joins the previous step's click group and
///   starts at `previous.end_ms + delay_ms`.
///
/// The very first step always behaves as [`Trigger::OnClick`] (there is no
/// previous step to fire with or after), so old decks — which carry no
/// `trigger` field and deserialize every step as `OnClick` with no delay —
/// produce the same one-frame-per-step, start-at-zero timeline as before.
///
/// A [`BuildEffect::MotionPath`] step carrying at least two waypoints expands
/// into one [`BuildFrame`] per segment (linear interpolation between
/// consecutive waypoint pairs), the step's duration split evenly across
/// segments. Every other step emits a single frame.
///
/// Deterministic: no `HashMap`, no sorting, no randomness — identical inputs
/// always yield byte-identical output.
pub fn build_timeline(animation: &Animation) -> Vec<BuildFrame> {
    let mut frames = Vec::new();
    let mut prev_group: usize = 0;
    let mut prev_start: u32 = 0;
    let mut prev_end: u32 = 0;

    for (step_index, step) in animation.steps.iter().enumerate() {
        let (click_group, start_ms) =
            step_start(step_index, step, prev_group, prev_start, prev_end);
        let end_ms = start_ms.saturating_add(step.duration_ms);

        push_step_frames(&mut frames, step_index, step, click_group, start_ms, end_ms);

        prev_group = click_group;
        prev_start = start_ms;
        prev_end = end_ms;
    }

    frames
}

/// Computes an instant (reduce-motion) frame sequence.
///
/// Used when the per-slide `reduce_motion` override (or a system-level
/// reduce-motion preference) is active: every step resolves to a
/// zero-duration, zero-offset frame so the presenter reveals shapes
/// immediately with no animation. Click grouping still honors `trigger`
/// (a [`Trigger::WithPrevious`]/[`Trigger::AfterPrevious`] step stays in the
/// previous step's group), but all timing collapses to zero. Motion-path steps
/// emit a single instant frame rather than per-segment interpolation.
pub fn build_timeline_reduced(animation: &Animation) -> Vec<BuildFrame> {
    let mut frames = Vec::with_capacity(animation.steps.len());
    let mut prev_group: usize = 0;
    for (step_index, step) in animation.steps.iter().enumerate() {
        let click_group = if step_index == 0 {
            0
        } else {
            match step.trigger {
                Trigger::OnClick => prev_group + 1,
                _ => prev_group,
            }
        };
        frames.push(BuildFrame {
            step_index,
            shape_index: step.shape_index,
            click_group,
            start_ms: 0,
            end_ms: 0,
            offset_ms: 0,
            effect: step.effect,
            motion_path: None,
        });
        prev_group = click_group;
    }
    frames
}

/// Resolves the click group and start time (ms, relative to the group's click)
/// for `step`, given the previously emitted step's group/timing.
fn step_start(
    step_index: usize,
    step: &BuildStep,
    prev_group: usize,
    prev_start: u32,
    prev_end: u32,
) -> (usize, u32) {
    // The first step always fires on the opening click: there is no previous
    // step to chain off, so WithPrevious/AfterPrevious degrade to OnClick.
    if step_index == 0 {
        return (0, step.delay_ms);
    }
    match step.trigger {
        Trigger::OnClick => (prev_group + 1, step.delay_ms),
        Trigger::WithPrevious => (prev_group, prev_start.saturating_add(step.delay_ms)),
        Trigger::AfterPrevious => (prev_group, prev_end.saturating_add(step.delay_ms)),
    }
}

/// Emits the [`BuildFrame`]s for a single step — one per motion-path segment
/// when applicable, otherwise a single frame — into `frames`.
fn push_step_frames(
    frames: &mut Vec<BuildFrame>,
    step_index: usize,
    step: &BuildStep,
    click_group: usize,
    start_ms: u32,
    end_ms: u32,
) {
    if step.effect == BuildEffect::MotionPath {
        if let Some(waypoints) = step.motion_path.as_ref().filter(|w| w.len() >= 2) {
            push_motion_segments(
                frames,
                step_index,
                step,
                click_group,
                start_ms,
                end_ms,
                waypoints,
            );
            return;
        }
    }

    frames.push(BuildFrame {
        step_index,
        shape_index: step.shape_index,
        click_group,
        start_ms,
        end_ms,
        offset_ms: step.delay_ms,
        effect: step.effect,
        motion_path: None,
    });
}

/// Splits a [`BuildEffect::MotionPath`] step into one [`BuildFrame`] per
/// segment, linearly interpolating between consecutive `waypoints` and
/// dividing the step's duration evenly across segments (any remainder from the
/// integer split is folded into the final segment so the full duration is
/// preserved).
fn push_motion_segments(
    frames: &mut Vec<BuildFrame>,
    step_index: usize,
    step: &BuildStep,
    click_group: usize,
    start_ms: u32,
    end_ms: u32,
    waypoints: &[Rect],
) {
    let segments = waypoints.len() - 1;
    let total = end_ms.saturating_sub(start_ms);
    let per = total / segments as u32;
    let last_extra = total - per * segments as u32;

    for i in 0..segments {
        let seg_start = if i == 0 {
            start_ms
        } else {
            start_ms.saturating_add(per * i as u32)
        };
        let mut seg_end = start_ms.saturating_add(per * (i + 1) as u32);
        if i == segments - 1 {
            seg_end = seg_end.saturating_add(last_extra);
        }
        frames.push(BuildFrame {
            step_index,
            shape_index: step.shape_index,
            click_group,
            start_ms: seg_start,
            end_ms: seg_end,
            // Only the first segment carries the step's delay; subsequent
            // segments chain off the previous segment with no extra offset.
            offset_ms: if i == 0 { step.delay_ms } else { 0 },
            effect: BuildEffect::MotionPath,
            motion_path: Some((waypoints[i], waypoints[i + 1])),
        });
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
            // The actual motion-path keyframes are generated per-step by
            // [`motion_path_keyframes`], which translates along the step's
            // waypoints. This static placeholder keeps the registry exhaustive
            // (and is returned when a motion-path step has no waypoints).
            "@keyframes build-motion-path { from { transform: translate(0, 0); } to { transform: translate(0, 0); } }"
        }
    }
}

/// Generates a deterministic `@keyframes` definition that translates a shape
/// along `waypoints` (EMU offsets relative to the shape's resting position).
///
/// The keyframe name matches the class returned by [`css_class`] for
/// [`BuildEffect::MotionPath`] (`"build-motion-path"`). Stops are evenly
/// spaced — `waypoints.len()` stops at `0%`, `100/(n-1)%`, ..., `100%` — each
/// translating to its waypoint's `(x, y)` offset, so consecutive stops perform
/// linear interpolation between waypoint pairs. With fewer than two waypoints
/// a no-op keyframe is returned. Deterministic for a given input.
pub fn motion_path_keyframes(waypoints: &[Rect]) -> String {
    const NAME: &str = "build-motion-path";
    if waypoints.len() < 2 {
        return format!(
            "@keyframes {NAME} {{ from {{ transform: translate(0px, 0px); }} to {{ transform: translate(0px, 0px); }} }}"
        );
    }

    let segments = waypoints.len() - 1;
    let mut stops = String::new();
    for (i, point) in waypoints.iter().enumerate() {
        let pct = i as f64 / segments as f64 * 100.0;
        stops.push_str(&format!(
            "{pct:.2}% {{ transform: translate({}px, {}px); }} ",
            point.x, point.y
        ));
    }
    format!("@keyframes {NAME} {{ {stops}}}")
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
        Shape, Style, TextBox, Transform, Trigger,
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
    fn build_timeline_on_click_separate_groups() {
        let animation = Animation::new(vec![
            BuildStep::new(0, BuildEffect::Fade, 200),
            BuildStep::new(1, BuildEffect::Appear, 300),
            BuildStep::new(2, BuildEffect::SlideInLeft, 400),
        ]);
        let timeline = build_timeline(&animation);

        assert_eq!(timeline.len(), 3);
        let groups: Vec<usize> = timeline.iter().map(|f| f.click_group).collect();
        assert_eq!(
            groups,
            vec![0, 1, 2],
            "each OnClick step is its own click group"
        );
        // No delays -> every step starts at the moment of its click.
        for frame in &timeline {
            assert_eq!(frame.start_ms, 0);
        }
    }

    #[test]
    fn build_timeline_with_previous_same_group() {
        let first = BuildStep::new(0, BuildEffect::Fade, 200);
        let mut second = BuildStep::new(1, BuildEffect::Appear, 200);
        second.trigger = Trigger::WithPrevious;
        let animation = Animation::new(vec![first, second]);
        let timeline = build_timeline(&animation);

        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].click_group, 0);
        assert_eq!(
            timeline[1].click_group, 0,
            "WithPrevious joins the previous step's group"
        );
        // Fires simultaneously with the previous step (both delay 0).
        assert_eq!(timeline[1].start_ms, timeline[0].start_ms);
    }

    #[test]
    fn build_timeline_after_previous_chains() {
        let first = BuildStep::new(0, BuildEffect::Fade, 200);
        let mut second = BuildStep::new(1, BuildEffect::Appear, 150);
        second.trigger = Trigger::AfterPrevious;
        let animation = Animation::new(vec![first, second]);
        let timeline = build_timeline(&animation);

        assert_eq!(timeline[0].click_group, 0);
        assert_eq!(
            timeline[1].click_group, 0,
            "AfterPrevious stays in the previous step's group"
        );
        assert_eq!(
            timeline[1].start_ms, timeline[0].end_ms,
            "starts when the previous step completes"
        );
        assert_eq!(timeline[1].start_ms, 200);
        assert_eq!(timeline[1].end_ms, 350);
    }

    #[test]
    fn build_timeline_delay_offsets_start() {
        let mut step = BuildStep::new(0, BuildEffect::Fade, 200);
        step.delay_ms = 100;
        let animation = Animation::new(vec![step]);
        let timeline = build_timeline(&animation);

        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].start_ms, 100, "delay shifts the start");
        assert_eq!(timeline[0].offset_ms, 100);
        assert_eq!(timeline[0].end_ms, 300);
    }

    #[test]
    fn build_timeline_motion_path_generates_segments() {
        let mut step = BuildStep::new(0, BuildEffect::MotionPath, 1000);
        step.motion_path = Some(vec![
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Rect::new(100.0, 0.0, 10.0, 10.0),
            Rect::new(100.0, 100.0, 10.0, 10.0),
        ]);
        let animation = Animation::new(vec![step]);
        let timeline = build_timeline(&animation);

        // 3 waypoints -> 2 interpolation segments.
        assert_eq!(timeline.len(), 2);
        // Duration split evenly: 1000ms / 2 segments = 500ms each.
        assert_eq!(timeline[0].start_ms, 0);
        assert_eq!(timeline[0].end_ms, 500);
        assert_eq!(timeline[1].start_ms, 500);
        assert_eq!(timeline[1].end_ms, 1000);
        // Each segment carries its waypoint pair for linear interpolation.
        assert_eq!(
            timeline[0].motion_path,
            Some((
                Rect::new(0.0, 0.0, 10.0, 10.0),
                Rect::new(100.0, 0.0, 10.0, 10.0)
            ))
        );
        assert_eq!(
            timeline[1].motion_path,
            Some((
                Rect::new(100.0, 0.0, 10.0, 10.0),
                Rect::new(100.0, 100.0, 10.0, 10.0)
            ))
        );
        for frame in &timeline {
            assert_eq!(frame.effect, BuildEffect::MotionPath);
            assert_eq!(frame.click_group, 0);
        }
    }

    #[test]
    fn build_timeline_motion_path_without_waypoints_falls_back() {
        // A MotionPath step with no waypoints degrades to a single frame.
        let step = BuildStep::new(0, BuildEffect::MotionPath, 500);
        let timeline = build_timeline(&Animation::new(vec![step]));
        assert_eq!(timeline.len(), 1);
        assert!(timeline[0].motion_path.is_none());
        assert_eq!(timeline[0].start_ms, 0);
        assert_eq!(timeline[0].end_ms, 500);
    }

    #[test]
    fn build_timeline_reduced_returns_instant() {
        let first = BuildStep::new(0, BuildEffect::Fade, 200);
        let mut second = BuildStep::new(1, BuildEffect::MotionPath, 300);
        second.trigger = Trigger::AfterPrevious;
        second.motion_path = Some(vec![
            Rect::new(0.0, 0.0, 1.0, 1.0),
            Rect::new(10.0, 10.0, 1.0, 1.0),
        ]);
        let animation = Animation::new(vec![first, second]);
        let timeline = build_timeline_reduced(&animation);

        assert_eq!(timeline.len(), 2);
        for frame in &timeline {
            assert_eq!(frame.start_ms, 0, "reduce-motion: instant start");
            assert_eq!(frame.end_ms, 0, "reduce-motion: zero duration");
            assert_eq!(frame.offset_ms, 0);
            assert!(frame.motion_path.is_none(), "no motion-path interpolation");
        }
        // Grouping still reflects triggers (second stays in first's group).
        assert_eq!(timeline[0].click_group, 0);
        assert_eq!(timeline[1].click_group, 0);
    }

    #[test]
    fn motion_path_keyframes_emits_waypoint_stops() {
        let waypoints = vec![
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Rect::new(100.0, 0.0, 10.0, 10.0),
            Rect::new(100.0, 100.0, 10.0, 10.0),
        ];
        let css = motion_path_keyframes(&waypoints);

        assert!(
            css.contains("@keyframes build-motion-path"),
            "named keyframes"
        );
        assert!(css.contains("0.00%"), "starts at 0%");
        assert!(css.contains("50.00%"), "midway stop");
        assert!(css.contains("100.00%"), "ends at 100%");
        assert!(
            css.contains("translate(0px, 0px)"),
            "translates from the origin"
        );
        assert!(css.contains("translate(100px"), "translates to a waypoint");
    }

    #[test]
    fn motion_path_keyframes_single_point_is_noop() {
        let css = motion_path_keyframes(&[Rect::new(5.0, 5.0, 1.0, 1.0)]);
        assert!(css.contains("@keyframes build-motion-path"));
        assert!(css.contains("translate(0px, 0px)"));
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
