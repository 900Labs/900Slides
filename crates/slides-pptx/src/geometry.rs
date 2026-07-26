//! Mapping between OOXML preset geometry (`a:prstGeom prst="..."`) and the
//! `slides_core::Geometry` model.
//!
//! Only the primitives in the deck model are recognized; every other preset is
//! left to the caller to preserve opaquely.

use slides_core::{Geometry, Rect};

/// The OOXML preset geometry names recognized by 900Slides.
const RECT: &str = "rect";
const ROUND_RECT: &str = "roundRect";
const ELLIPSE: &str = "ellipse";
const TRIANGLE: &str = "triangle";
const LINE: &str = "line";
const RIGHT_ARROW: &str = "rightArrow";
const RIGHT_ARROW_CALLOUT: &str = "rightArrowCallout";
const STAR5: &str = "star5";

/// Returns `true` when `prst` is one of the preset geometry names 900Slides
/// models in the deck.
pub(crate) fn is_supported_prst(prst: &str) -> bool {
    matches!(
        prst,
        RECT | ROUND_RECT | ELLIPSE | TRIANGLE | LINE | RIGHT_ARROW | RIGHT_ARROW_CALLOUT | STAR5
    )
}

/// Converts an OOXML `prst` value into a model [`Geometry`].
///
/// `adj_fraction` is the parsed `adj` guide value as a fraction in `0.0..=1.0`
/// (already divided by `100000`); it is only consulted for rounded rectangles.
/// `frame` provides the shape's pixel extent so a rounded rectangle's corner
/// radius can be expressed in EMU.
pub(crate) fn geometry_from_prst(
    prst: &str,
    adj_fraction: Option<f64>,
    frame: Rect,
) -> Option<Geometry> {
    let radius = || {
        let frac = adj_fraction.unwrap_or(0.16667);
        let min_side = frame.width.min(frame.height);
        (frac * min_side).max(0.0)
    };
    match prst {
        RECT => Some(Geometry::Rectangle),
        ROUND_RECT => Some(Geometry::RoundedRectangle { radius: radius() }),
        ELLIPSE => Some(Geometry::Ellipse),
        TRIANGLE => Some(Geometry::Triangle),
        LINE => Some(Geometry::Line),
        RIGHT_ARROW => Some(Geometry::Arrow),
        RIGHT_ARROW_CALLOUT => Some(Geometry::RightArrowCallout),
        STAR5 => Some(Geometry::Star5),
        _ => None,
    }
}

/// Returns the canonical OOXML `prst` name for a model [`Geometry`].
pub(crate) fn prst_from_geometry(geometry: &Geometry) -> &'static str {
    match geometry {
        Geometry::Rectangle => RECT,
        Geometry::RoundedRectangle { .. } => ROUND_RECT,
        Geometry::Ellipse => ELLIPSE,
        Geometry::Triangle => TRIANGLE,
        Geometry::Line => LINE,
        Geometry::Arrow => RIGHT_ARROW,
        Geometry::RightArrowCallout => RIGHT_ARROW_CALLOUT,
        Geometry::Star5 => STAR5,
    }
}

/// Returns the rounded-rectangle `adj` guide value (in OOXML units of
/// `1/100000`) derived from the model radius and the shape extent, or `None`
/// for non-rounded geometries.
pub(crate) fn rounded_rect_adj(geometry: &Geometry, frame: Rect) -> Option<i64> {
    if let Geometry::RoundedRectangle { radius } = geometry {
        let min_side = frame.width.min(frame.height);
        if min_side <= 0.0 {
            return Some(0);
        }
        let frac = (*radius / min_side).clamp(0.0, 0.5);
        Some((frac * 100_000.0).round() as i64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Rect {
        Rect::new(0.0, 0.0, 1_000_000.0, 600_000.0)
    }

    #[test]
    fn recognizes_all_supported_presets() {
        for prst in [
            "rect",
            "roundRect",
            "ellipse",
            "triangle",
            "line",
            "rightArrow",
            "rightArrowCallout",
            "star5",
        ] {
            assert!(is_supported_prst(prst), "{prst} should be supported");
            assert!(
                geometry_from_prst(prst, None, frame()).is_some(),
                "{prst} should map to a geometry"
            );
        }
        assert!(!is_supported_prst("custGeom-marker"));
        assert!(geometry_from_prst("chevron", None, frame()).is_none());
    }

    #[test]
    fn prst_and_geometry_are_inverse() {
        let cases = [
            Geometry::Rectangle,
            // A radius that lands on a whole `adj` value so the round trip is
            // exact: 150000 / min(1000000, 600000) = 0.25 -> adj 25000.
            Geometry::RoundedRectangle { radius: 150_000.0 },
            Geometry::Ellipse,
            Geometry::Triangle,
            Geometry::Line,
            Geometry::Arrow,
            Geometry::RightArrowCallout,
            Geometry::Star5,
        ];
        for geo in cases {
            let prst = prst_from_geometry(&geo);
            let adj_frac = rounded_rect_adj(&geo, frame()).map(|a| a as f64 / 100_000.0);
            let back = geometry_from_prst(prst, adj_frac, frame()).expect("round trip");
            assert!(
                geometry_equal(&geo, &back),
                "geometry {geo:?} did not round-trip through prst {prst:?} (got {back:?})"
            );
        }
    }

    #[test]
    fn rounded_rect_radius_uses_smaller_side() {
        let geo = geometry_from_prst("roundRect", Some(0.25), frame()).unwrap();
        match geo {
            Geometry::RoundedRectangle { radius } => {
                // 0.25 * min(1000000, 600000) = 150000
                assert!((radius - 150_000.0).abs() < 1e-6);
            }
            _ => panic!("expected rounded rectangle"),
        }
    }

    #[test]
    fn rounded_rect_adj_clamps_to_half() {
        let geo = Geometry::RoundedRectangle {
            radius: 10_000_000.0,
        };
        let adj = rounded_rect_adj(&geo, frame()).unwrap();
        assert_eq!(adj, 50_000, "adj must clamp at 50%");
    }

    /// Compares two geometries for equality (derives PartialEq-like behavior
    /// without requiring the model type to derive it in this crate).
    fn geometry_equal(a: &Geometry, b: &Geometry) -> bool {
        match (a, b) {
            (Geometry::Rectangle, Geometry::Rectangle) => true,
            (
                Geometry::RoundedRectangle { radius: ra },
                Geometry::RoundedRectangle { radius: rb },
            ) => (ra - rb).abs() < 1e-6,
            (Geometry::Ellipse, Geometry::Ellipse)
            | (Geometry::Triangle, Geometry::Triangle)
            | (Geometry::Line, Geometry::Line)
            | (Geometry::Arrow, Geometry::Arrow)
            | (Geometry::RightArrowCallout, Geometry::RightArrowCallout)
            | (Geometry::Star5, Geometry::Star5) => true,
            _ => false,
        }
    }
}
