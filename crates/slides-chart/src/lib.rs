//! Deterministic SVG chart rendering for the 900Slides deck model.
//!
//! The crate exposes a single public entry point: [`render_chart_svg`], which
//! turns a [`slides_core::ChartShape`] into a self-contained SVG string. The
//! output is deterministic: attributes and elements are emitted in a stable
//! order, and no hash-map iteration is used.

use slides_core::{ChartData, ChartShape, ChartType, Color};

/// Fixed deterministic theme-derived palette used for series and pie wedges.
const PALETTE: [Color; 6] = [
    Color::rgb(79, 129, 189),
    Color::rgb(192, 80, 77),
    Color::rgb(155, 187, 89),
    Color::rgb(128, 100, 162),
    Color::rgb(75, 172, 198),
    Color::rgb(247, 150, 70),
];

const TEXT_FILL: &str = "#333333";
const AXIS_STROKE: &str = "#888888";
const GRID_STROKE: &str = "#DDDDDD";
const LEGEND_FILL: &str = "#FAFAFA";
const LEGEND_STROKE: &str = "#CCCCCC";

/// Renders a chart shape as a self-contained, deterministic SVG string.
///
/// The returned SVG uses `width_emu` and `height_emu` for the `viewBox` and is
/// suitable for embedding or previewing. The renderer is defensive: it never
/// panics on empty, single-value, or zero-range data.
pub fn render_chart_svg(chart: &ChartShape, width_emu: f64, height_emu: f64) -> String {
    let mut svg = String::with_capacity(16384);

    svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" ");
    svg.push_str("viewBox=\"0 0 ");
    svg.push_str(&fmt_num(width_emu));
    svg.push(' ');
    svg.push_str(&fmt_num(height_emu));
    svg.push_str("\">\n");

    // White background so the chart is opaque regardless of slide background.
    svg.push_str("  <rect x=\"0\" y=\"0\" width=\"");
    svg.push_str(&fmt_num(width_emu));
    svg.push_str("\" height=\"");
    svg.push_str(&fmt_num(height_emu));
    svg.push_str("\" fill=\"#ffffff\"/>\n");

    let title_h = title_height(chart, height_emu);
    let top_margin = height_emu * 0.06 + title_h;
    let bottom_margin = height_emu * 0.14;
    let left_margin = width_emu * 0.14;
    let legend = legend_needed(chart);
    let right_margin = if legend {
        width_emu * 0.24
    } else {
        width_emu * 0.08
    };

    let plot_x = left_margin;
    let plot_y = top_margin;
    let plot_w = width_emu - left_margin - right_margin;
    let plot_h = height_emu - top_margin - bottom_margin;

    // Ensure a non-degenerate plot area even on tiny viewBoxes.
    let plot_w = plot_w.max(1.0);
    let plot_h = plot_h.max(1.0);

    render_title(&mut svg, chart, width_emu, title_h, height_emu);

    match &chart.data {
        ChartData::Category { categories, series } => match chart.chart_type {
            ChartType::Bar => render_bar(
                &mut svg, categories, series, plot_x, plot_y, plot_w, plot_h, height_emu,
            ),
            ChartType::Column => render_column(
                &mut svg, categories, series, plot_x, plot_y, plot_w, plot_h, height_emu,
            ),
            ChartType::Line => render_line(
                &mut svg, categories, series, plot_x, plot_y, plot_w, plot_h, height_emu,
            ),
            ChartType::Area => render_area(
                &mut svg, categories, series, plot_x, plot_y, plot_w, plot_h, height_emu,
            ),
            ChartType::Pie => render_pie(
                &mut svg, categories, series, plot_x, plot_y, plot_w, plot_h, height_emu,
            ),
            ChartType::Scatter => {
                // Scatter requires XY data; render a fallback rather than panic.
                render_fallback(&mut svg, plot_x, plot_y, plot_w, plot_h);
            }
        },
        ChartData::XY { series } => match chart.chart_type {
            ChartType::Scatter => {
                render_scatter(&mut svg, series, plot_x, plot_y, plot_w, plot_h, height_emu)
            }
            _ => {
                // Category types do not accept XY data; render a fallback.
                render_fallback(&mut svg, plot_x, plot_y, plot_w, plot_h);
            }
        },
    }

    if legend {
        render_legend(
            &mut svg,
            chart,
            width_emu - right_margin + width_emu * 0.02,
            top_margin,
            right_margin - width_emu * 0.04,
            plot_h,
            height_emu,
        );
    }

    svg.push_str("</svg>");
    svg
}

fn title_height(chart: &ChartShape, height_emu: f64) -> f64 {
    match &chart.title {
        Some(t) if !t.is_empty() => height_emu * 0.08,
        _ => 0.0,
    }
}

fn render_title(
    svg: &mut String,
    chart: &ChartShape,
    width_emu: f64,
    title_h: f64,
    height_emu: f64,
) {
    if title_h == 0.0 {
        return;
    }
    let y = height_emu * 0.05 + title_h * 0.25;
    svg.push_str("  <text x=\"");
    svg.push_str(&fmt_num(width_emu / 2.0));
    svg.push_str("\" y=\"");
    svg.push_str(&fmt_num(y));
    svg.push_str("\" text-anchor=\"middle\" font-size=\"");
    svg.push_str(&fmt_num(height_emu * 0.045));
    svg.push_str("\" font-weight=\"bold\" fill=\"");
    svg.push_str(TEXT_FILL);
    svg.push_str("\">");
    svg.push_str(&escape_xml(chart.title.as_deref().unwrap_or("")));
    svg.push_str("</text>\n");
}

fn legend_needed(chart: &ChartShape) -> bool {
    if chart.chart_type == ChartType::Pie {
        return true;
    }
    match &chart.data {
        ChartData::Category { series, .. } => series.len() > 1,
        ChartData::XY { series } => series.len() > 1,
    }
}

fn render_fallback(svg: &mut String, plot_x: f64, plot_y: f64, plot_w: f64, plot_h: f64) {
    svg.push_str("  <rect x=\"");
    svg.push_str(&fmt_num(plot_x));
    svg.push_str("\" y=\"");
    svg.push_str(&fmt_num(plot_y));
    svg.push_str("\" width=\"");
    svg.push_str(&fmt_num(plot_w));
    svg.push_str("\" height=\"");
    svg.push_str(&fmt_num(plot_h));
    svg.push_str("\" fill=\"#F5F5F5\" stroke=\"");
    svg.push_str(AXIS_STROKE);
    svg.push_str("\"/>\n");
}

// ---------------------------------------------------------------------------
// Category-based charts
// ---------------------------------------------------------------------------

fn category_range(series: &[slides_core::CategorySeries]) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for s in series {
        for v in &s.values {
            let v = *v;
            if v.is_finite() {
                if v < min {
                    min = v;
                }
                if v > max {
                    max = v;
                }
            }
        }
    }
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }
    if min == max {
        if max == 0.0 {
            return (0.0, 1.0);
        }
        return (0.0, max * 2.0);
    }
    let padding = (max - min) * 0.05;
    (min - padding, max + padding)
}

#[allow(clippy::too_many_arguments)]
fn render_value_axis(
    svg: &mut String,
    min: f64,
    max: f64,
    plot_x: f64,
    plot_y: f64,
    plot_w: f64,
    plot_h: f64,
    height_emu: f64,
    horizontal: bool,
) {
    let ticks = ticks(min, max, 5);

    if horizontal {
        // X-axis line at the bottom of the plot area.
        svg.push_str("  <line x1=\"");
        svg.push_str(&fmt_num(plot_x));
        svg.push_str("\" y1=\"");
        svg.push_str(&fmt_num(plot_y + plot_h));
        svg.push_str("\" x2=\"");
        svg.push_str(&fmt_num(plot_x + plot_w));
        svg.push_str("\" y2=\"");
        svg.push_str(&fmt_num(plot_y + plot_h));
        svg.push_str("\" stroke=\"");
        svg.push_str(AXIS_STROKE);
        svg.push_str("\" stroke-width=\"");
        svg.push_str(&fmt_num(height_emu * 0.0015));
        svg.push_str("\"/>\n");

        for &v in &ticks {
            let x = plot_x + (v - min) / (max - min) * plot_w;
            // Light vertical grid line.
            svg.push_str("  <line x1=\"");
            svg.push_str(&fmt_num(x));
            svg.push_str("\" y1=\"");
            svg.push_str(&fmt_num(plot_y));
            svg.push_str("\" x2=\"");
            svg.push_str(&fmt_num(x));
            svg.push_str("\" y2=\"");
            svg.push_str(&fmt_num(plot_y + plot_h));
            svg.push_str("\" stroke=\"");
            svg.push_str(GRID_STROKE);
            svg.push_str("\" stroke-width=\"");
            svg.push_str(&fmt_num(height_emu * 0.001));
            svg.push_str("\"/>\n");
            // Tick label.
            svg.push_str("  <text x=\"");
            svg.push_str(&fmt_num(x));
            svg.push_str("\" y=\"");
            svg.push_str(&fmt_num(plot_y + plot_h + height_emu * 0.04));
            svg.push_str("\" text-anchor=\"middle\" font-size=\"");
            svg.push_str(&fmt_num(height_emu * 0.025));
            svg.push_str("\" fill=\"");
            svg.push_str(TEXT_FILL);
            svg.push_str("\">");
            svg.push_str(&escape_xml(&fmt_tick(v)));
            svg.push_str("</text>\n");
        }
    } else {
        // Y-axis line at the left of the plot area.
        svg.push_str("  <line x1=\"");
        svg.push_str(&fmt_num(plot_x));
        svg.push_str("\" y1=\"");
        svg.push_str(&fmt_num(plot_y));
        svg.push_str("\" x2=\"");
        svg.push_str(&fmt_num(plot_x));
        svg.push_str("\" y2=\"");
        svg.push_str(&fmt_num(plot_y + plot_h));
        svg.push_str("\" stroke=\"");
        svg.push_str(AXIS_STROKE);
        svg.push_str("\" stroke-width=\"");
        svg.push_str(&fmt_num(height_emu * 0.0015));
        svg.push_str("\"/>\n");

        for &v in &ticks {
            let y = plot_y + plot_h - (v - min) / (max - min) * plot_h;
            // Light horizontal grid line.
            svg.push_str("  <line x1=\"");
            svg.push_str(&fmt_num(plot_x));
            svg.push_str("\" y1=\"");
            svg.push_str(&fmt_num(y));
            svg.push_str("\" x2=\"");
            svg.push_str(&fmt_num(plot_x + plot_w));
            svg.push_str("\" y2=\"");
            svg.push_str(&fmt_num(y));
            svg.push_str("\" stroke=\"");
            svg.push_str(GRID_STROKE);
            svg.push_str("\" stroke-width=\"");
            svg.push_str(&fmt_num(height_emu * 0.001));
            svg.push_str("\"/>\n");
            // Tick label.
            svg.push_str("  <text x=\"");
            svg.push_str(&fmt_num(plot_x - height_emu * 0.015));
            svg.push_str("\" y=\"");
            svg.push_str(&fmt_num(y + height_emu * 0.009));
            svg.push_str("\" text-anchor=\"end\" font-size=\"");
            svg.push_str(&fmt_num(height_emu * 0.025));
            svg.push_str("\" fill=\"");
            svg.push_str(TEXT_FILL);
            svg.push_str("\">");
            svg.push_str(&escape_xml(&fmt_tick(v)));
            svg.push_str("</text>\n");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_category_axis(
    svg: &mut String,
    categories: &[String],
    plot_x: f64,
    plot_y: f64,
    plot_w: f64,
    plot_h: f64,
    height_emu: f64,
    horizontal: bool,
) {
    let count = categories.len();
    if count == 0 {
        return;
    }
    let font_size = height_emu * 0.025;

    if horizontal {
        // Category labels along the bottom x-axis.
        let step = plot_w / count as f64;
        for (i, cat) in categories.iter().enumerate() {
            let x = plot_x + (i as f64 + 0.5) * step;
            svg.push_str("  <text x=\"");
            svg.push_str(&fmt_num(x));
            svg.push_str("\" y=\"");
            svg.push_str(&fmt_num(plot_y + plot_h + height_emu * 0.055));
            svg.push_str("\" text-anchor=\"middle\" font-size=\"");
            svg.push_str(&fmt_num(font_size));
            svg.push_str("\" fill=\"");
            svg.push_str(TEXT_FILL);
            svg.push_str("\">");
            svg.push_str(&escape_xml(cat));
            svg.push_str("</text>\n");
        }
    } else {
        // Category labels along the left y-axis (for horizontal bars).
        let step = plot_h / count as f64;
        for (i, cat) in categories.iter().enumerate() {
            let y = plot_y + (i as f64 + 0.5) * step;
            svg.push_str("  <text x=\"");
            svg.push_str(&fmt_num(plot_x - height_emu * 0.015));
            svg.push_str("\" y=\"");
            svg.push_str(&fmt_num(y + height_emu * 0.009));
            svg.push_str("\" text-anchor=\"end\" font-size=\"");
            svg.push_str(&fmt_num(font_size));
            svg.push_str("\" fill=\"");
            svg.push_str(TEXT_FILL);
            svg.push_str("\">");
            svg.push_str(&escape_xml(cat));
            svg.push_str("</text>\n");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_column(
    svg: &mut String,
    categories: &[String],
    series: &[slides_core::CategorySeries],
    plot_x: f64,
    plot_y: f64,
    plot_w: f64,
    plot_h: f64,
    height_emu: f64,
) {
    let (min, max) = category_range(series);
    let range = max - min;
    let range = if range == 0.0 { 1.0 } else { range };
    let cat_count = categories.len().max(1);
    let series_count = series.len().max(1);
    let group_w = plot_w / cat_count as f64;
    let bar_w = group_w / (series_count as f64 + 1.0);
    let baseline = plot_y + plot_h - (-min) / range * plot_h;

    render_value_axis(
        svg, min, max, plot_x, plot_y, plot_w, plot_h, height_emu, false,
    );
    render_category_axis(
        svg, categories, plot_x, plot_y, plot_w, plot_h, height_emu, true,
    );

    for (i, ser) in series.iter().enumerate() {
        let color = series_color(i);
        for (j, &v) in ser.values.iter().enumerate() {
            if j >= cat_count {
                break;
            }
            let x = plot_x + j as f64 * group_w + (i as f64 + 0.5) * bar_w;
            let y = plot_y + plot_h - ((v - min) / range) * plot_h;
            let h = baseline - y;
            svg.push_str("  <rect x=\"");
            svg.push_str(&fmt_num(x));
            svg.push_str("\" y=\"");
            svg.push_str(&fmt_num(y.min(baseline)));
            svg.push_str("\" width=\"");
            svg.push_str(&fmt_num(bar_w * 0.9));
            svg.push_str("\" height=\"");
            svg.push_str(&fmt_num(h.abs()));
            svg.push_str("\" fill=\"");
            svg.push_str(&color);
            svg.push_str("\" stroke=\"none\"/>\n");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_bar(
    svg: &mut String,
    categories: &[String],
    series: &[slides_core::CategorySeries],
    plot_x: f64,
    plot_y: f64,
    plot_w: f64,
    plot_h: f64,
    height_emu: f64,
) {
    let (min, max) = category_range(series);
    let range = max - min;
    let range = if range == 0.0 { 1.0 } else { range };
    let cat_count = categories.len().max(1);
    let series_count = series.len().max(1);
    let group_h = plot_h / cat_count as f64;
    let bar_h = group_h / (series_count as f64 + 1.0);
    let baseline = plot_x + (-min) / range * plot_w;

    render_value_axis(
        svg, min, max, plot_x, plot_y, plot_w, plot_h, height_emu, true,
    );
    render_category_axis(
        svg, categories, plot_x, plot_y, plot_w, plot_h, height_emu, false,
    );

    for (i, ser) in series.iter().enumerate() {
        let color = series_color(i);
        for (j, &v) in ser.values.iter().enumerate() {
            if j >= cat_count {
                break;
            }
            let y = plot_y + j as f64 * group_h + (i as f64 + 0.5) * bar_h;
            let x = plot_x + ((v - min) / range) * plot_w;
            let w = x - baseline;
            svg.push_str("  <rect x=\"");
            svg.push_str(&fmt_num(x.min(baseline)));
            svg.push_str("\" y=\"");
            svg.push_str(&fmt_num(y));
            svg.push_str("\" width=\"");
            svg.push_str(&fmt_num(w.abs()));
            svg.push_str("\" height=\"");
            svg.push_str(&fmt_num(bar_h * 0.9));
            svg.push_str("\" fill=\"");
            svg.push_str(&color);
            svg.push_str("\" stroke=\"none\"/>\n");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_line(
    svg: &mut String,
    categories: &[String],
    series: &[slides_core::CategorySeries],
    plot_x: f64,
    plot_y: f64,
    plot_w: f64,
    plot_h: f64,
    height_emu: f64,
) {
    let (min, max) = category_range(series);
    let range = max - min;
    let range = if range == 0.0 { 1.0 } else { range };
    let cat_count = categories.len().max(1);

    render_value_axis(
        svg, min, max, plot_x, plot_y, plot_w, plot_h, height_emu, false,
    );
    render_category_axis(
        svg, categories, plot_x, plot_y, plot_w, plot_h, height_emu, true,
    );

    for (i, ser) in series.iter().enumerate() {
        let color = series_color(i);
        let mut points = String::new();
        for (j, &v) in ser.values.iter().enumerate() {
            if j >= cat_count {
                break;
            }
            let x = plot_x + (j as f64 + 0.5) * (plot_w / cat_count as f64);
            let y = plot_y + plot_h - ((v - min) / range) * plot_h;
            if !points.is_empty() {
                points.push(' ');
            }
            points.push_str(&fmt_num(x));
            points.push(',');
            points.push_str(&fmt_num(y));
        }
        svg.push_str("  <polyline fill=\"none\" stroke=\"");
        svg.push_str(&color);
        svg.push_str("\" stroke-width=\"");
        svg.push_str(&fmt_num(plot_h * 0.005));
        svg.push_str("\" points=\"");
        svg.push_str(&points);
        svg.push_str("\"/>\n");
    }
}

#[allow(clippy::too_many_arguments)]
fn render_area(
    svg: &mut String,
    categories: &[String],
    series: &[slides_core::CategorySeries],
    plot_x: f64,
    plot_y: f64,
    plot_w: f64,
    plot_h: f64,
    height_emu: f64,
) {
    let (min, max) = category_range(series);
    let range = max - min;
    let range = if range == 0.0 { 1.0 } else { range };
    let cat_count = categories.len().max(1);
    let baseline_y = plot_y + plot_h - (-min) / range * plot_h;

    render_value_axis(
        svg, min, max, plot_x, plot_y, plot_w, plot_h, height_emu, false,
    );
    render_category_axis(
        svg, categories, plot_x, plot_y, plot_w, plot_h, height_emu, true,
    );

    for (i, ser) in series.iter().enumerate() {
        let color = series_color(i);
        let mut d = String::new();
        let step = plot_w / cat_count as f64;
        let first_x = plot_x + 0.5 * step;
        d.push('M');
        d.push_str(&fmt_num(first_x));
        d.push(' ');
        d.push_str(&fmt_num(baseline_y));
        for (j, &v) in ser.values.iter().enumerate() {
            if j >= cat_count {
                break;
            }
            let x = plot_x + (j as f64 + 0.5) * step;
            let y = plot_y + plot_h - ((v - min) / range) * plot_h;
            d.push('L');
            d.push_str(&fmt_num(x));
            d.push(' ');
            d.push_str(&fmt_num(y));
        }
        let last_x = plot_x + (cat_count as f64 - 0.5) * step;
        d.push('L');
        d.push_str(&fmt_num(last_x));
        d.push(' ');
        d.push_str(&fmt_num(baseline_y));
        d.push('Z');

        svg.push_str("  <path fill=\"");
        svg.push_str(&color);
        svg.push_str("\" fill-opacity=\"0.6\" stroke=\"none\" d=\"");
        svg.push_str(&d);
        svg.push_str("\"/>\n");
    }
}

// ---------------------------------------------------------------------------
// Pie chart
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_pie(
    svg: &mut String,
    categories: &[String],
    series: &[slides_core::CategorySeries],
    plot_x: f64,
    plot_y: f64,
    plot_w: f64,
    plot_h: f64,
    height_emu: f64,
) {
    let cx = plot_x + plot_w / 2.0;
    let cy = plot_y + plot_h / 2.0;
    let radius = (plot_w.min(plot_h) / 2.0) * 0.85;

    let values: Vec<f64> = series
        .first()
        .map(|s| s.values.iter().copied().take(categories.len()).collect())
        .unwrap_or_default();
    let total: f64 = values.iter().filter(|v| v.is_finite()).sum();
    let total = if total == 0.0 { 1.0 } else { total };

    let mut start_angle = 0.0;
    for (i, v) in values.iter().enumerate() {
        let v = if v.is_finite() { *v } else { 0.0 };
        let angle = (v / total) * 2.0 * std::f64::consts::PI;
        let end_angle = start_angle + angle;
        let d = wedge_path(cx, cy, radius, start_angle, end_angle);
        svg.push_str("  <path fill=\"");
        svg.push_str(&series_color(i));
        svg.push_str("\" stroke=\"#ffffff\" stroke-width=\"");
        svg.push_str(&fmt_num(height_emu * 0.003));
        svg.push_str("\" d=\"");
        svg.push_str(&d);
        svg.push_str("\"/>\n");
        start_angle = end_angle;
    }
}

fn wedge_path(cx: f64, cy: f64, r: f64, start: f64, end: f64) -> String {
    let x1 = cx + r * start.cos();
    let y1 = cy + r * start.sin();
    let x2 = cx + r * end.cos();
    let y2 = cy + r * end.sin();
    let large_arc = if end - start > std::f64::consts::PI {
        "1"
    } else {
        "0"
    };
    let mut d = String::with_capacity(128);
    d.push_str("M ");
    d.push_str(&fmt_num(cx));
    d.push(' ');
    d.push_str(&fmt_num(cy));
    d.push_str(" L ");
    d.push_str(&fmt_num(x1));
    d.push(' ');
    d.push_str(&fmt_num(y1));
    d.push_str(" A ");
    d.push_str(&fmt_num(r));
    d.push(' ');
    d.push_str(&fmt_num(r));
    d.push_str(" 0 ");
    d.push_str(large_arc);
    d.push_str(" 1 ");
    d.push_str(&fmt_num(x2));
    d.push(' ');
    d.push_str(&fmt_num(y2));
    d.push_str(" Z");
    d
}

// ---------------------------------------------------------------------------
// Scatter chart
// ---------------------------------------------------------------------------

fn xy_ranges(series: &[slides_core::XYSeries]) -> ((f64, f64), (f64, f64)) {
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for s in series {
        for p in &s.points {
            if p.x.is_finite() {
                if p.x < x_min {
                    x_min = p.x;
                }
                if p.x > x_max {
                    x_max = p.x;
                }
            }
            if p.y.is_finite() {
                if p.y < y_min {
                    y_min = p.y;
                }
                if p.y > y_max {
                    y_max = p.y;
                }
            }
        }
    }
    if !x_min.is_finite() || !x_max.is_finite() {
        x_min = 0.0;
        x_max = 1.0;
    }
    if !y_min.is_finite() || !y_max.is_finite() {
        y_min = 0.0;
        y_max = 1.0;
    }
    if x_min == x_max {
        if x_max == 0.0 {
            x_max = 1.0;
        } else {
            x_max *= 2.0;
        }
    }
    if y_min == y_max {
        if y_max == 0.0 {
            y_max = 1.0;
        } else {
            y_max *= 2.0;
        }
    }
    let x_pad = (x_max - x_min) * 0.05;
    let y_pad = (y_max - y_min) * 0.05;
    (
        (x_min - x_pad, x_max + x_pad),
        (y_min - y_pad, y_max + y_pad),
    )
}

fn render_scatter(
    svg: &mut String,
    series: &[slides_core::XYSeries],
    plot_x: f64,
    plot_y: f64,
    plot_w: f64,
    plot_h: f64,
    height_emu: f64,
) {
    let ((x_min, x_max), (y_min, y_max)) = xy_ranges(series);
    let x_range = x_max - x_min;
    let y_range = y_max - y_min;
    let x_range = if x_range == 0.0 { 1.0 } else { x_range };
    let y_range = if y_range == 0.0 { 1.0 } else { y_range };
    let radius = plot_h * 0.012;

    render_value_axis(
        svg, x_min, x_max, plot_x, plot_y, plot_w, plot_h, height_emu, true,
    );
    render_value_axis(
        svg, y_min, y_max, plot_x, plot_y, plot_w, plot_h, height_emu, false,
    );

    for (i, ser) in series.iter().enumerate() {
        let color = series_color(i);
        for p in &ser.points {
            let x = plot_x + (p.x - x_min) / x_range * plot_w;
            let y = plot_y + plot_h - (p.y - y_min) / y_range * plot_h;
            svg.push_str("  <circle cx=\"");
            svg.push_str(&fmt_num(x));
            svg.push_str("\" cy=\"");
            svg.push_str(&fmt_num(y));
            svg.push_str("\" r=\"");
            svg.push_str(&fmt_num(radius));
            svg.push_str("\" fill=\"");
            svg.push_str(&color);
            svg.push_str("\" stroke=\"#ffffff\" stroke-width=\"");
            svg.push_str(&fmt_num(radius * 0.3));
            svg.push_str("\"/>\n");
        }
    }
}

// ---------------------------------------------------------------------------
// Legend
// ---------------------------------------------------------------------------

fn render_legend(
    svg: &mut String,
    chart: &ChartShape,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    height_emu: f64,
) {
    let font_size = height_emu * 0.025;
    let swatch = font_size * 0.8;
    let line_h = font_size * 1.6;

    // Background panel.
    svg.push_str("  <rect x=\"");
    svg.push_str(&fmt_num(x));
    svg.push_str("\" y=\"");
    svg.push_str(&fmt_num(y));
    svg.push_str("\" width=\"");
    svg.push_str(&fmt_num(w));
    svg.push_str("\" height=\"");
    svg.push_str(&fmt_num(h));
    svg.push_str("\" fill=\"");
    svg.push_str(LEGEND_FILL);
    svg.push_str("\" stroke=\"");
    svg.push_str(LEGEND_STROKE);
    svg.push_str("\" stroke-width=\"");
    svg.push_str(&fmt_num(height_emu * 0.001));
    svg.push_str("\"/>\n");

    let entries: Vec<(String, String)> = match &chart.data {
        ChartData::Category { categories, series } => {
            if chart.chart_type == ChartType::Pie {
                categories
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c.clone(), series_color(i)))
                    .collect()
            } else {
                series
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (s.name.clone(), series_color(i)))
                    .collect()
            }
        }
        ChartData::XY { series } => series
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.clone(), series_color(i)))
            .collect(),
    };

    for (i, (label, color)) in entries.iter().enumerate() {
        let item_y = y + line_h * (i as f64 + 0.8);
        svg.push_str("  <rect x=\"");
        svg.push_str(&fmt_num(x + font_size * 0.4));
        svg.push_str("\" y=\"");
        svg.push_str(&fmt_num(item_y - swatch * 0.7));
        svg.push_str("\" width=\"");
        svg.push_str(&fmt_num(swatch));
        svg.push_str("\" height=\"");
        svg.push_str(&fmt_num(swatch));
        svg.push_str("\" fill=\"");
        svg.push_str(color);
        svg.push_str("\" stroke=\"none\"/>\n");

        svg.push_str("  <text x=\"");
        svg.push_str(&fmt_num(x + font_size * 1.5));
        svg.push_str("\" y=\"");
        svg.push_str(&fmt_num(item_y));
        svg.push_str("\" font-size=\"");
        svg.push_str(&fmt_num(font_size));
        svg.push_str("\" fill=\"");
        svg.push_str(TEXT_FILL);
        svg.push_str("\">");
        svg.push_str(&escape_xml(label));
        svg.push_str("</text>\n");
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn series_color(index: usize) -> String {
    color_hex(PALETTE[index % PALETTE.len()])
}

fn color_hex(c: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

fn ticks(min: f64, max: f64, count: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(count + 1);
    let range = max - min;
    if range == 0.0 || !range.is_finite() {
        out.push(min);
        return out;
    }
    for i in 0..=count {
        let t = i as f64 / count as f64;
        out.push(min + t * range);
    }
    out
}

fn fmt_tick(v: f64) -> String {
    if v == 0.0 || v.abs() < 1e-12 {
        return "0".to_string();
    }
    let abs = v.abs();
    if !(0.001..1_000_000.0).contains(&abs) {
        format!("{:.2e}", v)
    } else if abs >= 1.0 {
        format!("{:.2}", v)
    } else {
        format!("{:.3}", v)
    }
}

fn fmt_num(v: f64) -> String {
    if v.is_nan() || v.is_infinite() || v == 0.0 {
        return "0".to_string();
    }
    let s = format!("{:.6}", v);
    if s.contains('.') {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.').to_string();
        if trimmed.is_empty() || trimmed == "-" {
            "0".to_string()
        } else {
            trimmed
        }
    } else {
        s
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slides_core::{CategorySeries, ChartShape, ChartType, Rect, Transform, XYPoint, XYSeries};

    fn dummy_transform() -> Transform {
        Transform {
            frame: Rect::new(0.0, 0.0, 1_000_000.0, 1_000_000.0),
            rotation: 0.0,
        }
    }

    fn category_chart(
        chart_type: ChartType,
        categories: Vec<&str>,
        series: Vec<(&str, Vec<f64>)>,
        title: Option<&str>,
    ) -> ChartShape {
        let data = ChartData::Category {
            categories: categories.into_iter().map(String::from).collect(),
            series: series
                .into_iter()
                .map(|(name, values)| CategorySeries {
                    name: String::from(name),
                    values,
                })
                .collect(),
        };
        ChartShape::new(dummy_transform(), chart_type, data, title.map(String::from)).unwrap()
    }

    fn xy_chart(
        chart_type: ChartType,
        series: Vec<(&str, Vec<(f64, f64)>)>,
        title: Option<&str>,
    ) -> ChartShape {
        let data = ChartData::XY {
            series: series
                .into_iter()
                .map(|(name, pts)| XYSeries {
                    name: String::from(name),
                    points: pts.into_iter().map(|(x, y)| XYPoint::new(x, y)).collect(),
                })
                .collect(),
        };
        ChartShape::new(dummy_transform(), chart_type, data, title.map(String::from)).unwrap()
    }

    fn render(t: ChartType) -> String {
        let chart = match t {
            ChartType::Bar => category_chart(
                ChartType::Bar,
                vec!["A", "B", "C"],
                vec![("S1", vec![1.0, 2.0, 3.0]), ("S2", vec![2.0, 3.0, 1.0])],
                None,
            ),
            ChartType::Column => category_chart(
                ChartType::Column,
                vec!["A", "B", "C"],
                vec![("S1", vec![1.0, 2.0, 3.0]), ("S2", vec![2.0, 3.0, 1.0])],
                None,
            ),
            ChartType::Line => category_chart(
                ChartType::Line,
                vec!["A", "B", "C"],
                vec![("S1", vec![1.0, 2.0, 3.0])],
                None,
            ),
            ChartType::Area => category_chart(
                ChartType::Area,
                vec!["A", "B", "C"],
                vec![("S1", vec![1.0, 2.0, 3.0])],
                None,
            ),
            ChartType::Pie => category_chart(
                ChartType::Pie,
                vec!["A", "B", "C"],
                vec![("", vec![3.0, 2.0, 1.0])],
                None,
            ),
            ChartType::Scatter => xy_chart(
                ChartType::Scatter,
                vec![("S1", vec![(1.0, 1.0), (2.0, 2.0), (3.0, 1.0)])],
                None,
            ),
        };
        render_chart_svg(&chart, 1_000_000.0, 800_000.0)
    }

    #[test]
    fn determinism_all_types() {
        for t in [
            ChartType::Bar,
            ChartType::Column,
            ChartType::Line,
            ChartType::Area,
            ChartType::Pie,
            ChartType::Scatter,
        ] {
            let a = render(t);
            let b = render(t);
            assert_eq!(a, b, "chart type {:?} is not deterministic", t);
        }
    }

    #[test]
    fn column_contains_rects() {
        let svg = render(ChartType::Column);
        assert!(svg.contains("<rect"), "{}", svg);
    }

    #[test]
    fn bar_contains_rects() {
        let svg = render(ChartType::Bar);
        assert!(svg.contains("<rect"), "{}", svg);
    }

    #[test]
    fn line_contains_polyline() {
        let svg = render(ChartType::Line);
        assert!(svg.contains("<polyline"), "{}", svg);
    }

    #[test]
    fn area_contains_path() {
        let svg = render(ChartType::Area);
        assert!(svg.contains("<path"), "{}", svg);
    }

    #[test]
    fn pie_contains_path() {
        let svg = render(ChartType::Pie);
        assert!(svg.contains("<path"), "{}", svg);
        assert!(svg.contains("M "), "{}", svg);
    }

    #[test]
    fn scatter_contains_circles() {
        let svg = render(ChartType::Scatter);
        assert!(svg.contains("<circle"), "{}", svg);
    }

    #[test]
    fn title_renders_when_set() {
        let chart = category_chart(
            ChartType::Column,
            vec!["A"],
            vec![("S1", vec![1.0])],
            Some("Revenue &lt; Growth"),
        );
        let svg = render_chart_svg(&chart, 1_000_000.0, 800_000.0);
        assert!(svg.contains("Revenue"), "{}", svg);
        assert!(
            svg.contains("&amp;lt;"),
            "title should be XML-escaped: {}",
            svg
        );
        assert!(
            svg.contains("font-weight=\"bold\""),
            "title should be bold: {}",
            svg
        );
    }

    #[test]
    fn title_absent_when_empty() {
        let chart = category_chart(ChartType::Column, vec!["A"], vec![("S1", vec![1.0])], None);
        let svg = render_chart_svg(&chart, 1_000_000.0, 800_000.0);
        assert!(
            !svg.contains("font-weight=\"bold\""),
            "unexpected title element: {}",
            svg
        );
    }

    #[test]
    fn xml_escape_category_label() {
        let chart = category_chart(
            ChartType::Column,
            vec!["<b>Bold</b>"],
            vec![("S1", vec![1.0])],
            None,
        );
        let svg = render_chart_svg(&chart, 1_000_000.0, 800_000.0);
        assert!(svg.contains("&lt;b&gt;Bold&lt;/b&gt;"), "{}", svg);
        assert!(!svg.contains("<b>Bold</b>"), "raw HTML leaked: {}", svg);
    }

    #[test]
    fn single_category_does_not_panic() {
        let chart = category_chart(
            ChartType::Column,
            vec!["Only"],
            vec![("S1", vec![5.0])],
            None,
        );
        let svg = render_chart_svg(&chart, 1_000_000.0, 800_000.0);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn zero_range_does_not_panic() {
        let chart = category_chart(
            ChartType::Column,
            vec!["A", "B"],
            vec![("S1", vec![2.0, 2.0])],
            None,
        );
        let svg = render_chart_svg(&chart, 1_000_000.0, 800_000.0);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn pie_legend_lists_categories() {
        let chart = category_chart(
            ChartType::Pie,
            vec!["Apple", "Banana", "Cherry"],
            vec![("", vec![3.0, 2.0, 1.0])],
            None,
        );
        let svg = render_chart_svg(&chart, 1_000_000.0, 800_000.0);
        assert!(svg.contains("Apple"), "{}", svg);
        assert!(svg.contains("Banana"), "{}", svg);
        assert!(svg.contains("Cherry"), "{}", svg);
    }
}
