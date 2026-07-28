//! OOXML transition and build-in animation parsing (loader side, Wave 5).
//!
//! Only the constrained v0.2.0 subset is modeled: simple slide transitions
//! (`p:transition`) and simple build-in entrance animations (`p:timing`).
//! Anything more complex falls back to `None` plus a loss warning. The raw
//! OOXML is preserved byte-for-byte by the saver for untouched slides, so no
//! data is lost on round-trip.
//!
//! Both `p:transition` and `p:timing` live outside `p:spTree`; the loader
//! captures them as raw strings during the slide walk (see `load.rs`) and hands
//! those strings to [`parse_transition`] / [`parse_animation`] here.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;
use slides_core::{Animation, BuildEffect, BuildStep, Transition, TransitionKind};

use crate::ledger::{LossLedger, LossWarning};
use crate::load::{attr_by_local_name, qname_str};

/// Default transition/build duration in milliseconds when `spd`/`dur` is absent.
const DEFAULT_DURATION_MS: u32 = 500;

// ---------------------------------------------------------------------------
// Transitions
// ---------------------------------------------------------------------------

/// Parses a captured `p:transition` element into a model [`Transition`].
///
/// Returns `None` when the element carries no modeled variant (absent child or
/// `p:cut`). Unrecognized variants record a loss warning and also return
/// `None`; the raw element is preserved by the saver for untouched slides.
pub fn parse_transition(xml: &str, slide_id: &str, ledger: &mut LossLedger) -> Option<Transition> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut spd: Option<String> = None;
    let mut variant: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local = qname_str(e.name());
                if local == "transition" {
                    spd = attr_by_local_name(&e, "spd");
                } else if variant.is_none() {
                    // The first child element inside p:transition is the variant
                    // (p:fade, p:push, p:cut, ...).
                    variant = Some(local);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let duration_ms = spd_to_ms(spd.as_deref());
    let kind = match variant.as_deref() {
        Some("fade") => Some(TransitionKind::Fade),
        Some("push") => Some(TransitionKind::Push),
        Some("wipe") => Some(TransitionKind::Wipe),
        Some("pull") | Some("slide") => Some(TransitionKind::Slide),
        None | Some("cut") => None,
        Some(other) => {
            ledger.add(LossWarning::new(
                slide_id,
                format!("transition variant '{other}' not modeled; preserved as raw OOXML"),
            ));
            return None;
        }
    };

    kind.map(|k| Transition::new(k, duration_ms))
}

/// Resolves a `spd` attribute (slow/med/fast or numeric ms) to milliseconds.
fn spd_to_ms(spd: Option<&str>) -> u32 {
    let Some(value) = spd else {
        return DEFAULT_DURATION_MS;
    };
    match value.to_ascii_lowercase().as_str() {
        "slow" => 1000,
        "med" | "medium" => DEFAULT_DURATION_MS,
        "fast" => 250,
        _ => value.parse::<u32>().unwrap_or(DEFAULT_DURATION_MS),
    }
}

// ---------------------------------------------------------------------------
// Build-in animations (p:timing)
// ---------------------------------------------------------------------------

/// A collected `p:animEffect` entrance effect with its resolved target/duration.
struct AnimEffect {
    transition: Option<String>,
    filter: Option<String>,
    spid: Option<String>,
    dur_ms: u32,
}

/// Result of a single pass over a `p:timing` tree.
struct TimingScan {
    effects: Vec<AnimEffect>,
    /// True if the tree contains animation behavior outside the simple
    /// build-in subset (motion paths, emphasis, exit effects, commands, ...).
    has_unsupported: bool,
    /// True if any animation behavior element was present at all.
    any_behavior: bool,
}

/// Parses a captured `p:timing` element into a model [`Animation`].
///
/// Only the simple build-in subset is modeled: ordered `p:animEffect
/// transition="in"` entrance effects, each targeting one shape by its OOXML
/// `p:cNvPr` id (resolved to a model shape index via `id_to_index`). Complex or
/// unrecognized timing structures fall back to `None` plus a loss warning and
/// never panic; the raw element is preserved by the saver.
pub fn parse_animation(
    xml: &str,
    id_to_index: &HashMap<String, usize>,
    slide_id: &str,
    ledger: &mut LossLedger,
) -> Option<Animation> {
    let scan = scan_timing(xml);

    if scan.has_unsupported {
        ledger.add(LossWarning::new(
            slide_id,
            "complex slide animation not modeled; preserved as raw OOXML".to_string(),
        ));
        return None;
    }

    let mut steps: Vec<BuildStep> = Vec::new();
    for effect in scan.effects {
        // The simple subset only models entrances; `transition` defaults to
        // "in". Any explicit non-"in" effect was already flagged as unsupported
        // above, so this is a defensive no-op.
        if matches!(effect.transition.as_deref(), Some(t) if t != "in") {
            continue;
        }

        let shape_index = match effect
            .spid
            .as_deref()
            .and_then(|id| id_to_index.get(id).copied())
        {
            Some(index) => index,
            None => {
                ledger.add(LossWarning::new(
                    slide_id,
                    format!(
                        "build-in targets unknown shape id {}; step skipped",
                        effect.spid.as_deref().unwrap_or("?")
                    ),
                ));
                continue;
            }
        };

        let Some(effect_kind) = effect.filter.as_deref().and_then(map_filter) else {
            ledger.add(LossWarning::new(
                slide_id,
                format!(
                    "build-in filter {} not modeled; step skipped",
                    effect.filter.as_deref().unwrap_or("?")
                ),
            ));
            continue;
        };

        steps.push(BuildStep::new(shape_index, effect_kind, effect.dur_ms));
    }

    if steps.is_empty() {
        if scan.any_behavior {
            ledger.add(LossWarning::new(
                slide_id,
                "slide animation not modeled; preserved as raw OOXML".to_string(),
            ));
        }
        return None;
    }

    Some(Animation::new(steps))
}

/// Walks a `p:timing` tree once, collecting entrance `p:animEffect` effects and
/// flagging any behavior outside the simple build-in subset.
fn scan_timing(xml: &str) -> TimingScan {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut effects: Vec<AnimEffect> = Vec::new();
    let mut has_unsupported = false;
    let mut any_behavior = false;

    // Depth inside the current <p:animEffect>. We capture the effect's first
    // nested <p:cTn dur="..."> (duration) and <p:spTgt spid="..."> (target).
    let mut anim_depth: i32 = 0;
    let mut p_transition: Option<String> = None;
    let mut p_filter: Option<String> = None;
    let mut p_spid: Option<String> = None;
    let mut p_dur: Option<u32> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "animEffect" => {
                        any_behavior = true;
                        let transition = attr_by_local_name(&e, "transition");
                        let filter = attr_by_local_name(&e, "filter");
                        // Only entrances are modeled; an explicit exit/other
                        // direction marks the whole timing as unsupported.
                        if matches!(transition.as_deref(), Some(t) if t != "in") {
                            has_unsupported = true;
                        }
                        if anim_depth == 0 {
                            p_transition = transition;
                            p_filter = filter;
                            p_spid = None;
                            p_dur = None;
                        }
                        anim_depth += 1;
                    }
                    "set" => any_behavior = true,
                    "anim" | "animMotion" | "animRot" | "animScale" | "animClr" | "cmd" => {
                        any_behavior = true;
                        has_unsupported = true;
                    }
                    "cTn" if anim_depth > 0 && p_dur.is_none() => {
                        if let Some(dur) = attr_by_local_name(&e, "dur") {
                            p_dur = parse_dur(&dur);
                        }
                    }
                    "spTgt" if anim_depth > 0 && p_spid.is_none() => {
                        if let Some(spid) = attr_by_local_name(&e, "spid") {
                            p_spid = Some(spid);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                if qname_str(e.name()) == "animEffect" {
                    anim_depth -= 1;
                    if anim_depth <= 0 {
                        anim_depth = 0;
                        effects.push(AnimEffect {
                            transition: p_transition.take(),
                            filter: p_filter.take(),
                            spid: p_spid.take(),
                            dur_ms: p_dur.take().unwrap_or(DEFAULT_DURATION_MS),
                        });
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    TimingScan {
        effects,
        has_unsupported,
        any_behavior,
    }
}

/// Parses a `dur` attribute into milliseconds. `indefinite` and unparseable
/// values yield `None` (the caller falls back to the default duration).
fn parse_dur(value: &str) -> Option<u32> {
    if value.eq_ignore_ascii_case("indefinite") {
        return None;
    }
    value.parse::<u32>().ok()
}

/// Maps an OOXML `animEffect` filter (e.g. `fade`, `wipe(up)`) to a model
/// [`BuildEffect`]. Returns `None` for unrecognized filters so the caller can
/// skip the step with a loss warning.
fn map_filter(filter: &str) -> Option<BuildEffect> {
    let lower = filter.to_ascii_lowercase();
    if lower.contains("fade") {
        Some(BuildEffect::Fade)
    } else if lower.contains("left") {
        Some(BuildEffect::SlideInLeft)
    } else if lower.contains("right") {
        Some(BuildEffect::SlideInRight)
    } else if lower.contains("up") {
        Some(BuildEffect::SlideInTop)
    } else if lower.contains("down") {
        Some(BuildEffect::SlideInBottom)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spd_maps_named_speeds() {
        assert_eq!(spd_to_ms(None), 500);
        assert_eq!(spd_to_ms(Some("slow")), 1000);
        assert_eq!(spd_to_ms(Some("med")), 500);
        assert_eq!(spd_to_ms(Some("medium")), 500);
        assert_eq!(spd_to_ms(Some("fast")), 250);
        assert_eq!(spd_to_ms(Some("750")), 750);
        assert_eq!(spd_to_ms(Some("garbage")), 500);
    }

    #[test]
    fn filter_maps_fade_and_directions() {
        assert_eq!(map_filter("fade"), Some(BuildEffect::Fade));
        assert_eq!(map_filter("wipe(left)"), Some(BuildEffect::SlideInLeft));
        assert_eq!(map_filter("wipe(right)"), Some(BuildEffect::SlideInRight));
        assert_eq!(map_filter("wipe(up)"), Some(BuildEffect::SlideInTop));
        assert_eq!(map_filter("wipe(down)"), Some(BuildEffect::SlideInBottom));
        assert_eq!(map_filter("cube"), None);
    }

    #[test]
    fn parse_transition_fade_default_duration() {
        let mut ledger = LossLedger::new();
        let xml = "<p:transition xmlns:p=\"p\"><p:fade/></p:transition>";
        let t = parse_transition(xml, "s1", &mut ledger);
        assert_eq!(
            t,
            Some(Transition {
                kind: TransitionKind::Fade,
                duration_ms: 500,
            })
        );
    }

    #[test]
    fn parse_transition_cut_is_none_without_warning() {
        let mut ledger = LossLedger::new();
        let xml = "<p:transition xmlns:p=\"p\"><p:cut/></p:transition>";
        assert_eq!(parse_transition(xml, "s1", &mut ledger), None);
        assert!(ledger.is_empty(), "cut should not warn");
    }

    #[test]
    fn parse_transition_unknown_variant_warns() {
        let mut ledger = LossLedger::new();
        let xml = "<p:transition xmlns:p=\"p\"><p:morph/></p:transition>";
        assert_eq!(parse_transition(xml, "s1", &mut ledger), None);
        assert!(!ledger.is_empty());
    }
}
