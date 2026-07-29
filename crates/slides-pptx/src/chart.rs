//! Chart part parsing, patching, and generation for PPTX load/save.
//!
//! Chart data lives in separate `ppt/charts/chartN.xml` parts referenced by a
//! slide relationship. The loader resolves those parts and maps the OOXML
//! `c:chartSpace` to a [`slides_core::ChartShape`]. The saver patches only the
//! data-bearing sections (title, categories, series names, values, and scatter
//! x/y points) while copying the rest of the chart XML byte-for-byte.

use std::collections::HashMap;
use std::io::Write;

use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};
use slides_core::{
    CategorySeries, ChartData, ChartShape, ChartType, Rect, Transform, XYPoint, XYSeries,
};

use crate::error::{Error, Result};
use crate::load::{parse_attr_f64, qname_str, rel_attribute};

/// URI carried by `<a:graphicData>` for chart frames.
pub const CHART_GRAPHIC_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";

const C_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Returns true when a captured `<p:graphicFrame>` carries chart data.
pub fn is_chart_frame(captured: &str) -> bool {
    captured.contains(CHART_GRAPHIC_URI)
}

/// Extracts the `r:id` from a `<c:chart>` element inside a slide-level graphic
/// frame. Returns `None` if the element is not a chart frame.
pub fn extract_chart_rid(captured: &str) -> Option<String> {
    let mut reader = Reader::from_str(captured);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                if qname_str(e.name()) == "chart" {
                    return rel_attribute(&e, "id");
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Parses the bounding frame of a captured `<p:graphicFrame>`.
pub fn parse_graphic_frame_frame(captured: &str) -> Option<Transform> {
    let mut reader = Reader::from_str(captured);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut in_xfrm = false;
    let mut off: Option<(f64, f64)> = None;
    let mut ext: Option<(f64, f64)> = None;
    let mut rot: Option<f64> = None;

    loop {
        match reader.read_event_into(&mut buf).ok()? {
            Event::Start(e) | Event::Empty(e) => {
                let local = qname_str(e.name());
                match local.as_str() {
                    "xfrm" => {
                        in_xfrm = true;
                        rot = parse_attr_f64(&e, "rot");
                    }
                    "off" if in_xfrm => {
                        off = Some((
                            parse_attr_f64(&e, "x").unwrap_or(0.0),
                            parse_attr_f64(&e, "y").unwrap_or(0.0),
                        ));
                    }
                    "ext" if in_xfrm => {
                        ext = Some((
                            parse_attr_f64(&e, "cx").unwrap_or(0.0),
                            parse_attr_f64(&e, "cy").unwrap_or(0.0),
                        ));
                    }
                    _ => {}
                }
            }
            Event::End(e) => {
                if qname_str(e.name()) == "xfrm" {
                    if let (Some((x, y)), Some((cx, cy))) = (off, ext) {
                        return Some(Transform {
                            frame: Rect::new(x, y, cx, cy),
                            rotation: rot.map(|r| r / 60_000.0).unwrap_or(0.0),
                        });
                    }
                    in_xfrm = false;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Parsing state for a chart XML document.
#[derive(Default, Debug)]
struct ChartParseState {
    chart_type: Option<ChartType>,
    chart_type_set: bool,
    title: Option<String>,
    in_title: bool,
    in_title_tx: bool,
    in_title_rich: bool,
    in_title_p: bool,
    in_title_r: bool,
    in_title_t: bool,
    in_plot_area: bool,
    in_ser: bool,
    in_tx: bool,
    in_cat: bool,
    in_val: bool,
    in_x_val: bool,
    in_y_val: bool,
    in_str_ref: bool,
    in_num_ref: bool,
    in_str_cache: bool,
    in_num_cache: bool,
    in_pt: bool,
    in_v: bool,
    pt_index: Option<usize>,
    text: String,
    categories: Vec<String>,
    series_names: Vec<String>,
    current_series_name: String,
    current_values: Vec<f64>,
    current_points: Vec<XYPoint>,
    current_x_values: Vec<f64>,
    current_y_values: Vec<f64>,
    category_series: Vec<CategorySeries>,
    xy_series: Vec<XYSeries>,
    /// Track the first encountered cat values to use as shared categories.
    first_cat: Option<Vec<String>>,
}

impl ChartParseState {
    fn finalize_series(&mut self) {
        if self.chart_type == Some(ChartType::Scatter) {
            let points: Vec<XYPoint> = if self.current_x_values.len() == self.current_y_values.len()
            {
                self.current_x_values
                    .iter()
                    .zip(self.current_y_values.iter())
                    .map(|(&x, &y)| XYPoint::new(x, y))
                    .collect()
            } else if !self.current_y_values.is_empty() {
                // If only Y values are present, treat X as 0,1,2,...
                self.current_y_values
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| XYPoint::new(i as f64, y))
                    .collect()
            } else {
                Vec::new()
            };
            self.xy_series.push(XYSeries {
                name: std::mem::take(&mut self.current_series_name),
                points,
            });
            self.current_x_values.clear();
            self.current_y_values.clear();
        } else {
            self.category_series.push(CategorySeries {
                name: std::mem::take(&mut self.current_series_name),
                values: std::mem::take(&mut self.current_values),
            });
        }
    }

    fn build_chart_data(&mut self, chart_type: ChartType) -> Option<ChartData> {
        if chart_type == ChartType::Scatter {
            if self.xy_series.is_empty() {
                return None;
            }
            Some(ChartData::XY {
                series: std::mem::take(&mut self.xy_series),
            })
        } else {
            if self.category_series.is_empty() {
                return None;
            }
            let categories = std::mem::take(&mut self.first_cat).unwrap_or_default();
            Some(ChartData::Category {
                categories,
                series: std::mem::take(&mut self.category_series),
            })
        }
    }
}

/// Parses a chart XML part into a [`ChartShape`].
///
/// Returns `None` when the chart type is unrecognized, the series are missing,
/// or the XML is malformed. The caller should fall back to passthrough.
pub fn parse_chart_xml(xml: &str, transform: Transform) -> Option<ChartShape> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut st = ChartParseState::default();

    loop {
        let ev = match reader.read_event_into(&mut buf) {
            Ok(e) => e,
            Err(_) => return None,
        };
        match &ev {
            Event::Start(e) => chart_start(e, &mut st),
            Event::Empty(e) => {
                chart_start(e, &mut st);
                let local = qname_str(e.name());
                chart_end(&local, &mut st);
            }
            Event::End(e) => {
                let local = qname_str(e.name());
                chart_end(&local, &mut st);
            }
            Event::Text(t) => {
                if st.in_v {
                    st.text.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let chart_type = st.chart_type?;
    let data = st.build_chart_data(chart_type)?;
    ChartShape::new(transform, chart_type, data, st.title).ok()
}

/// Parses a captured `<p:graphicFrame>` that references a chart part.
///
/// Returns `Some(chart)` when the frame carries a chart, the relationship can be
/// resolved, and the chart XML is modelable. On failure returns `None`; the
/// caller can preserve the frame as an opaque passthrough object.
#[allow(clippy::too_many_arguments)]
pub fn parse_chart_frame(
    captured: &str,
    slide_id: &str,
    chart_by_rid: &HashMap<String, String>,
    chart_bytes_by_rid: &HashMap<String, Vec<u8>>,
    shape_index: usize,
    ledger: &mut crate::ledger::LossLedger,
    chart_source_parts: &mut HashMap<String, HashMap<usize, String>>,
    original_chart_bytes: &mut HashMap<String, Vec<u8>>,
) -> Option<ChartShape> {
    let rid = extract_chart_rid(captured)?;
    let part_path = chart_by_rid.get(&rid)?.clone();
    let bytes = chart_bytes_by_rid.get(&rid)?;
    let transform = parse_graphic_frame_frame(captured).unwrap_or_default();
    let xml = String::from_utf8_lossy(bytes);
    let chart = parse_chart_xml(&xml, transform).or_else(|| {
        ledger.add(crate::ledger::LossWarning::new(
            slide_id,
            format!("chart part {part_path} could not be modeled; preserved as opaque object"),
        ));
        None
    })?;

    chart_source_parts
        .entry(slide_id.to_string())
        .or_default()
        .insert(shape_index, part_path.clone());
    original_chart_bytes.insert(part_path, bytes.clone());
    Some(chart)
}

fn chart_start(e: &BytesStart<'_>, st: &mut ChartParseState) {
    let local = qname_str(e.name());
    match local.as_str() {
        "title" => st.in_title = true,
        "tx" if st.in_title => st.in_title_tx = true,
        "rich" if st.in_title_tx => st.in_title_rich = true,
        "p" if st.in_title_rich => st.in_title_p = true,
        "r" if st.in_title_p => st.in_title_r = true,
        "t" if st.in_title_r => {
            st.in_title_t = true;
            st.in_v = true;
        }
        "plotArea" => st.in_plot_area = true,
        "barChart" if st.in_plot_area && !st.chart_type_set => {
            st.chart_type = Some(ChartType::Column);
            st.chart_type_set = true;
        }
        "barDir" if st.in_plot_area && st.chart_type == Some(ChartType::Column) => {
            if let Some(dir) = attr_by_local_name(e, "val") {
                if dir == "bar" {
                    st.chart_type = Some(ChartType::Bar);
                }
            }
        }
        "lineChart" if st.in_plot_area && !st.chart_type_set => {
            st.chart_type = Some(ChartType::Line);
            st.chart_type_set = true;
        }
        "areaChart" if st.in_plot_area && !st.chart_type_set => {
            st.chart_type = Some(ChartType::Area);
            st.chart_type_set = true;
        }
        "pieChart" if st.in_plot_area && !st.chart_type_set => {
            st.chart_type = Some(ChartType::Pie);
            st.chart_type_set = true;
        }
        "scatterChart" if st.in_plot_area && !st.chart_type_set => {
            st.chart_type = Some(ChartType::Scatter);
            st.chart_type_set = true;
        }
        "ser" if st.in_plot_area => {
            st.in_ser = true;
            st.current_series_name.clear();
            st.current_values.clear();
            st.current_x_values.clear();
            st.current_y_values.clear();
            st.current_points.clear();
        }
        "tx" if st.in_ser => st.in_tx = true,
        "cat" if st.in_ser => st.in_cat = true,
        "val" if st.in_ser => st.in_val = true,
        "xVal" if st.in_ser => st.in_x_val = true,
        "yVal" if st.in_ser => st.in_y_val = true,
        "strRef" if (st.in_tx || st.in_cat || st.in_title) => st.in_str_ref = true,
        "numRef" if (st.in_val || st.in_x_val || st.in_y_val) => st.in_num_ref = true,
        "strCache" if st.in_str_ref => st.in_str_cache = true,
        "numCache" if st.in_num_ref => st.in_num_cache = true,
        "pt" if (st.in_str_cache || st.in_num_cache) => {
            st.in_pt = true;
            st.pt_index = attr_by_local_name(e, "idx").and_then(|v| v.parse().ok());
            st.text.clear();
        }
        "v" if st.in_pt => st.in_v = true,
        _ => {}
    }
}

fn chart_end(local: &str, st: &mut ChartParseState) {
    match local {
        "title" => st.in_title = false,
        "tx" if st.in_title && st.in_title_tx => {
            st.in_title_tx = false;
            st.in_title_rich = false;
            st.in_title_p = false;
            st.in_title_r = false;
            st.in_title_t = false;
        }
        "rich" => st.in_title_rich = false,
        "p" if st.in_title_rich => st.in_title_p = false,
        "r" if st.in_title_p => st.in_title_r = false,
        "t" if st.in_title_r => {
            st.in_title_t = false;
            st.in_v = false;
            if st.in_title {
                if st.title.is_none() {
                    st.title = Some(std::mem::take(&mut st.text));
                } else {
                    st.title.as_mut().unwrap().push_str(&st.text);
                }
            }
        }
        "plotArea" => st.in_plot_area = false,
        "ser" if st.in_ser => {
            st.in_ser = false;
            st.finalize_series();
        }
        "tx" if st.in_ser && st.in_tx => {
            st.in_tx = false;
            st.in_str_ref = false;
            st.in_str_cache = false;
            if !st.current_series_name.is_empty() {
                st.series_names.push(st.current_series_name.clone());
            }
        }
        "cat" if st.in_ser && st.in_cat => {
            st.in_cat = false;
            st.in_str_ref = false;
            st.in_str_cache = false;
            if st.first_cat.is_none() && !st.categories.is_empty() {
                st.first_cat = Some(std::mem::take(&mut st.categories));
            }
        }
        "val" if st.in_ser && st.in_val => {
            st.in_val = false;
            st.in_num_ref = false;
            st.in_num_cache = false;
        }
        "xVal" if st.in_ser && st.in_x_val => {
            st.in_x_val = false;
            st.in_num_ref = false;
            st.in_num_cache = false;
        }
        "yVal" if st.in_ser && st.in_y_val => {
            st.in_y_val = false;
            st.in_num_ref = false;
            st.in_num_cache = false;
        }
        "strCache" => st.in_str_cache = false,
        "numCache" => st.in_num_cache = false,
        "strRef" => st.in_str_ref = false,
        "numRef" => st.in_num_ref = false,
        "pt" if st.in_pt => {
            st.in_pt = false;
            if st.in_str_cache && st.in_title {
                // title text is captured by <t> end
            } else if st.in_str_cache && st.in_tx {
                st.current_series_name = std::mem::take(&mut st.text);
            } else if st.in_str_cache && st.in_cat {
                st.categories.push(std::mem::take(&mut st.text));
            } else if st.in_num_cache && (st.in_val || st.in_y_val) {
                if let Ok(v) = st.text.trim().parse::<f64>() {
                    st.current_values.push(v);
                }
                st.text.clear();
            } else if st.in_num_cache && st.in_x_val {
                if let Ok(v) = st.text.trim().parse::<f64>() {
                    st.current_x_values.push(v);
                }
                st.text.clear();
            }
        }
        "v" if st.in_v => {
            st.in_v = false;
        }
        _ => {}
    }
}

fn attr_by_local_name(e: &BytesStart<'_>, name: &str) -> Option<String> {
    for attr in e.attributes() {
        let attr = attr.ok()?;
        if attr.key.local_name().as_ref() == name.as_bytes() {
            return attr.unescape_value().ok().map(|v| v.into_owned());
        }
    }
    None
}

/// XML-escapes a small set of characters for attribute and text content.
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Chart XML patching
// ---------------------------------------------------------------------------

/// Patching state for a chart XML document.
#[derive(Debug)]
struct ChartPatchState {
    chart_type: ChartType,
    title: Option<String>,
    data: Option<ChartData>,
    /// Series index while iterating through c:ser elements.
    series_index: usize,
    /// Index inside the current c:strCache/c:numCache pt list.
    pt_index: usize,
    /// Whether we are currently inside a data section that should be skipped
    /// and rewritten.
    skip_depth: usize,
    /// Name of the element that opened the current skip region, used to know
    /// when to resume copying.
    skip_target: Option<String>,
    /// Whether we are currently inside a c:title whose a:p should be rewritten.
    in_title: bool,
    in_title_tx: bool,
    in_title_rich: bool,
    in_title_p: bool,
    in_plot_area: bool,
    in_ser: bool,
    in_tx: bool,
    in_cat: bool,
    in_val: bool,
    in_x_val: bool,
    in_y_val: bool,
    in_str_cache: bool,
    in_num_cache: bool,
    /// Tracks the element that was rewritten to adjust the end tag name.
    rewritten_start: Option<String>,
    /// Whether the original chart type element was already encountered.
    chart_type_seen: bool,
}

impl ChartPatchState {
    fn new(chart: &ChartShape) -> Self {
        Self {
            chart_type: chart.chart_type,
            title: chart.title.clone(),
            data: Some(chart.data.clone()),
            series_index: 0,
            pt_index: 0,
            skip_depth: 0,
            skip_target: None,
            in_title: false,
            in_title_tx: false,
            in_title_rich: false,
            in_title_p: false,
            in_plot_area: false,
            in_ser: false,
            in_tx: false,
            in_cat: false,
            in_val: false,
            in_x_val: false,
            in_y_val: false,
            in_str_cache: false,
            in_num_cache: false,
            rewritten_start: None,
            chart_type_seen: false,
        }
    }
}

/// Patches a chart XML document in place, rewriting data sections only.
///
/// The returned bytes preserve all structure, attributes, and whitespace outside
/// the data-bearing sections. Only `c:title`, `c:strCache`/`c:numCache`
/// values, and the chart-type element tag are updated.
pub fn patch_chart_xml(original: &str, chart: &ChartShape) -> Result<Vec<u8>> {
    let mut reader = Reader::from_str(original);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut writer = Writer::new(&mut out);

    let mut st = ChartPatchState::new(chart);

    loop {
        let ev = reader.read_event_into(&mut buf)?;
        match &ev {
            Event::Start(e) => {
                let local = qname_str(e.name());

                if st.skip_depth > 0 {
                    st.skip_depth += 1;
                    buf.clear();
                    continue;
                }

                if local == "title" {
                    st.in_title = true;
                } else if local == "tx" && st.in_title && !st.in_title_tx {
                    st.in_title_tx = true;
                } else if local == "rich" && st.in_title_tx {
                    st.in_title_rich = true;
                } else if local == "p" && st.in_title_rich {
                    st.in_title_p = true;
                } else if local == "plotArea" {
                    st.in_plot_area = true;
                } else if is_chart_type_element(&local) && st.in_plot_area && !st.chart_type_seen {
                    st.chart_type_seen = true;
                    st.rewritten_start = Some(local.clone());
                    let new_tag = chart_type_tag(&local, st.chart_type, e);
                    writer.get_mut().write_all(new_tag.as_bytes())?;
                    buf.clear();
                    continue;
                } else if local == "ser" && st.in_plot_area {
                    st.in_ser = true;
                    st.in_tx = false;
                    st.in_cat = false;
                    st.in_val = false;
                    st.in_x_val = false;
                    st.in_y_val = false;
                    st.pt_index = 0;
                } else if local == "tx" && st.in_ser {
                    st.in_tx = true;
                } else if local == "cat" && st.in_ser {
                    st.in_cat = true;
                } else if local == "val" && st.in_ser {
                    st.in_val = true;
                } else if local == "xVal" && st.in_ser {
                    st.in_x_val = true;
                } else if local == "yVal" && st.in_ser {
                    st.in_y_val = true;
                } else if local == "strCache"
                    && ((st.in_ser && (st.in_tx || st.in_cat)) || (st.in_title && st.in_title_tx))
                {
                    st.in_str_cache = true;
                    st.skip_depth = 1;
                    st.skip_target = Some("strCache".to_string());
                    write_replacement_str_cache(&mut writer, &st, &mut reader, &mut buf)?;
                    st.in_str_cache = false;
                    st.skip_depth = 0;
                    st.skip_target = None;
                    buf.clear();
                    continue;
                } else if local == "numCache"
                    && st.in_ser
                    && (st.in_val || st.in_x_val || st.in_y_val)
                {
                    st.in_num_cache = true;
                    st.skip_depth = 1;
                    st.skip_target = Some("numCache".to_string());
                    write_replacement_num_cache(&mut writer, &st, &mut reader, &mut buf)?;
                    st.in_num_cache = false;
                    st.skip_depth = 0;
                    st.skip_target = None;
                    buf.clear();
                    continue;
                }

                writer.write_event(Event::Start(e.clone()))?;
            }
            Event::End(e) => {
                let local = qname_str(e.name());

                if st.skip_depth > 0 {
                    st.skip_depth = st.skip_depth.saturating_sub(1);
                    if st.skip_depth == 0 {
                        st.skip_target = None;
                    }
                    buf.clear();
                    continue;
                }

                if let Some(start) = st.rewritten_start.as_ref() {
                    if qname_str(e.name()) == *start {
                        let end_name = chart_type_end_name(start, st.chart_type);
                        writer.write_event(Event::End(BytesEnd::new(end_name)))?;
                        st.rewritten_start = None;
                        buf.clear();
                        continue;
                    }
                }

                if local == "title" {
                    st.in_title = false;
                } else if local == "tx" && st.in_title && st.in_title_tx && !st.in_ser {
                    st.in_title_tx = false;
                    st.in_title_rich = false;
                    st.in_title_p = false;
                } else if local == "rich" && st.in_title_tx {
                    st.in_title_rich = false;
                    st.in_title_p = false;
                } else if local == "p" && st.in_title_rich {
                    st.in_title_p = false;
                } else if local == "plotArea" {
                    st.in_plot_area = false;
                } else if local == "ser" && st.in_ser {
                    st.in_ser = false;
                    st.series_index += 1;
                } else if local == "tx" && st.in_ser && st.in_tx {
                    st.in_tx = false;
                } else if local == "cat" && st.in_ser && st.in_cat {
                    st.in_cat = false;
                } else if local == "val" && st.in_ser && st.in_val {
                    st.in_val = false;
                } else if local == "xVal" && st.in_ser && st.in_x_val {
                    st.in_x_val = false;
                } else if local == "yVal" && st.in_ser && st.in_y_val {
                    st.in_y_val = false;
                }

                writer.write_event(Event::End(e.clone()))?;
            }
            Event::Empty(e) => {
                let local = qname_str(e.name());

                if st.skip_depth > 0 {
                    buf.clear();
                    continue;
                }

                if is_chart_type_element(&local) && st.in_plot_area && !st.chart_type_seen {
                    st.chart_type_seen = true;
                    let new_tag = chart_type_tag(&local, st.chart_type, e);
                    writer.get_mut().write_all(new_tag.as_bytes())?;
                    buf.clear();
                    continue;
                }

                if local == "p" && st.in_title_rich && !st.in_title_p {
                    // Empty title paragraph: write a paragraph with the title text.
                    write_title_paragraph(&mut writer, st.title.as_deref())?;
                    buf.clear();
                    continue;
                }

                writer.write_event(Event::Empty(e.clone()))?;
            }
            Event::Text(t) => {
                if st.skip_depth > 0 {
                    buf.clear();
                    continue;
                }
                writer.write_event(Event::Text(t.clone()))?;
            }
            Event::Eof => break,
            _ => {
                if st.skip_depth == 0 {
                    writer.write_event(ev.clone())?;
                }
            }
        }
        buf.clear();
    }
    Ok(out)
}

fn is_chart_type_element(local: &str) -> bool {
    matches!(
        local,
        "barChart" | "lineChart" | "areaChart" | "pieChart" | "scatterChart"
    )
}

fn chart_type_end_name(original: &str, chart_type: ChartType) -> String {
    let prefix = chart_type_element_name(chart_type);
    // Keep the original namespace prefix if present.
    if let Some(colon) = original.find(':') {
        format!("{}:{}", &original[..colon], prefix)
    } else {
        prefix.to_string()
    }
}

fn chart_type_element_name(chart_type: ChartType) -> &'static str {
    match chart_type {
        ChartType::Bar | ChartType::Column => "barChart",
        ChartType::Line => "lineChart",
        ChartType::Area => "areaChart",
        ChartType::Pie => "pieChart",
        ChartType::Scatter => "scatterChart",
    }
}

fn chart_type_tag(original: &str, chart_type: ChartType, e: &BytesStart<'_>) -> String {
    let mut attrs = String::new();
    for attr in e.attributes().flatten() {
        let key_local = attr.key.local_name();
        let key = std::str::from_utf8(key_local.as_ref()).unwrap_or_default();
        let val = attr.unescape_value().unwrap_or_default();
        attrs.push_str(&format!(" {key}=\"{}\"", escape_xml(&val)));
    }
    let prefix = chart_type_element_name(chart_type);
    let ns_prefix = original
        .find(':')
        .map(|c| format!("{}:", &original[..c]))
        .unwrap_or_default();
    format!("<{ns_prefix}{prefix}{attrs}>")
}

fn write_title_paragraph<W: Write>(writer: &mut Writer<W>, title: Option<&str>) -> Result<()> {
    let text = title.unwrap_or("");
    writer.get_mut().write_all(
        format!(
            "<a:p><a:r><a:rPr/><a:t xml:space=\"preserve\">{}</a:t></a:r></a:p>",
            escape_xml(text)
        )
        .as_bytes(),
    )?;
    Ok(())
}

fn write_replacement_str_cache<W: Write>(
    writer: &mut Writer<W>,
    st: &ChartPatchState,
    reader: &mut Reader<&[u8]>,
    buf: &mut Vec<u8>,
) -> Result<()> {
    let values: Vec<String> = if st.in_title {
        st.title.clone().into_iter().collect()
    } else if st.in_tx {
        st.data
            .as_ref()
            .and_then(|d| match d {
                ChartData::Category { series, .. } => {
                    series.get(st.series_index).map(|s| vec![s.name.clone()])
                }
                ChartData::XY { series } => {
                    series.get(st.series_index).map(|s| vec![s.name.clone()])
                }
            })
            .unwrap_or_default()
    } else if st.in_cat {
        st.data
            .as_ref()
            .and_then(|d| match d {
                ChartData::Category { categories, .. } => Some(categories.clone()),
                _ => None,
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let count = values.len();
    writer
        .get_mut()
        .write_all(format!("<c:strCache><c:ptCount val=\"{count}\"/>").as_bytes())?;
    for (i, v) in values.iter().enumerate() {
        writer.get_mut().write_all(
            format!("<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>", escape_xml(v)).as_bytes(),
        )?;
    }
    writer.get_mut().write_all(b"</c:strCache>")?;

    // Consume the original strCache subtree.
    skip_subtree(reader, buf, "strCache")?;
    Ok(())
}

fn write_replacement_num_cache<W: Write>(
    writer: &mut Writer<W>,
    st: &ChartPatchState,
    reader: &mut Reader<&[u8]>,
    buf: &mut Vec<u8>,
) -> Result<()> {
    let values: Vec<f64> = if st.in_val {
        st.data
            .as_ref()
            .and_then(|d| match d {
                ChartData::Category { series, .. } => {
                    series.get(st.series_index).map(|s| s.values.clone())
                }
                _ => None,
            })
            .unwrap_or_default()
    } else if st.in_y_val {
        st.data
            .as_ref()
            .and_then(|d| match d {
                ChartData::XY { series } => series
                    .get(st.series_index)
                    .map(|s| s.points.iter().map(|p| p.y).collect()),
                _ => None,
            })
            .unwrap_or_default()
    } else if st.in_x_val {
        st.data
            .as_ref()
            .and_then(|d| match d {
                ChartData::XY { series } => series
                    .get(st.series_index)
                    .map(|s| s.points.iter().map(|p| p.x).collect()),
                _ => None,
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let count = values.len();
    writer
        .get_mut()
        .write_all(format!("<c:numCache><c:ptCount val=\"{count}\"/>").as_bytes())?;
    for (i, v) in values.iter().enumerate() {
        writer
            .get_mut()
            .write_all(format!("<c:pt idx=\"{i}\"><c:v>{v}</c:v></c:pt>").as_bytes())?;
    }
    writer.get_mut().write_all(b"</c:numCache>")?;

    skip_subtree(reader, buf, "numCache")?;
    Ok(())
}

fn skip_subtree<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    buf: &mut Vec<u8>,
    target: &str,
) -> Result<()> {
    let mut depth = 0usize;
    loop {
        match reader.read_event_into(buf)? {
            Event::Start(e) => {
                if qname_str(e.name()) == target {
                    depth += 1;
                }
            }
            Event::End(e) => {
                if qname_str(e.name()) == target {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
            }
            Event::Eof => return Err(Error::MissingPart("truncated chart cache".into())),
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

/// Generates a complete chart XML part for a newly inserted chart.
pub fn generate_chart_xml(chart: &ChartShape) -> Vec<u8> {
    let title_xml = chart
        .title
        .as_ref()
        .map(|t| {
            format!(
                r#"<c:title><c:tx><c:rich><a:bodyPr xmlns:a="{A_NS}"/><a:lstStyle xmlns:a="{A_NS}"/><a:p xmlns:a="{A_NS}"><a:r><a:rPr/><a:t xml:space="preserve">{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val="0"/></c:title>"#,
                escape_xml(t)
            )
        })
        .unwrap_or_default();

    let plot_area_xml = generate_plot_area(chart);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:chartSpace xmlns:c="{C_NS}" xmlns:a="{A_NS}" xmlns:r="{R_NS}">
  <c:chart>
    {title_xml}
    <c:plotArea>
      {plot_area_xml}
    </c:plotArea>
    <c:plotVisOnly val="1"/>
  </c:chart>
</c:chartSpace>"#
    )
    .into_bytes()
}

fn generate_plot_area(chart: &ChartShape) -> String {
    let chart_type_xml = match chart.chart_type {
        ChartType::Bar => r#"<c:barChart><c:barDir val="bar"/><c:grouping val="clustered"/>"#,
        ChartType::Column => r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/>"#,
        ChartType::Line => r#"<c:lineChart><c:grouping val="standard"/>"#,
        ChartType::Area => r#"<c:areaChart><c:grouping val="standard"/>"#,
        ChartType::Pie => r#"<c:pieChart>"#,
        ChartType::Scatter => r#"<c:scatterChart><c:scatterStyle val="lineMarker"/>"#,
    };

    let mut series_xml = String::new();
    match &chart.data {
        ChartData::Category { categories, series } => {
            for (idx, s) in series.iter().enumerate() {
                series_xml.push_str(&generate_category_series(s, categories, idx));
            }
        }
        ChartData::XY { series } => {
            for (idx, s) in series.iter().enumerate() {
                series_xml.push_str(&generate_xy_series(s, idx));
            }
        }
    }

    let end_tag = match chart.chart_type {
        ChartType::Bar | ChartType::Column => "</c:barChart>",
        ChartType::Line => "</c:lineChart>",
        ChartType::Area => "</c:areaChart>",
        ChartType::Pie => "</c:pieChart>",
        ChartType::Scatter => "</c:scatterChart>",
    };

    format!("{chart_type_xml}{series_xml}{end_tag}")
}

fn generate_category_series(series: &CategorySeries, categories: &[String], idx: usize) -> String {
    let cat_count = categories.len();
    let mut cat_pts = String::new();
    for (i, cat) in categories.iter().enumerate() {
        cat_pts.push_str(&format!(
            "<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>",
            escape_xml(cat)
        ));
    }
    let val_count = series.values.len();
    let mut val_pts = String::new();
    for (i, v) in series.values.iter().enumerate() {
        val_pts.push_str(&format!("<c:pt idx=\"{i}\"><c:v>{v}</c:v></c:pt>"));
    }

    format!(
        r#"<c:ser><c:idx val="{idx}"/><c:order val="{idx}"/>\
<c:tx><c:strRef><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>{}</c:v></c:pt></c:strCache></c:strRef></c:tx>\
<c:cat><c:strRef><c:strCache><c:ptCount val="{cat_count}"/>{cat_pts}</c:strCache></c:strRef></c:cat>\
<c:val><c:numRef><c:numCache><c:ptCount val="{val_count}"/>{val_pts}</c:numCache></c:numRef></c:val></c:ser>"#,
        escape_xml(&series.name)
    )
}

fn generate_xy_series(series: &XYSeries, idx: usize) -> String {
    let x_count = series.points.len();
    let mut x_pts = String::new();
    let mut y_pts = String::new();
    for (i, p) in series.points.iter().enumerate() {
        x_pts.push_str(&format!("<c:pt idx=\"{i}\"><c:v>{x}</c:v></c:pt>", x = p.x));
        y_pts.push_str(&format!("<c:pt idx=\"{i}\"><c:v>{y}</c:v></c:pt>", y = p.y));
    }

    format!(
        r#"<c:ser><c:idx val="{idx}"/><c:order val="{idx}"/>\
<c:tx><c:strRef><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>{}</c:v></c:pt></c:strCache></c:strRef></c:tx>\
<c:xVal><c:numRef><c:numCache><c:ptCount val="{x_count}"/>{x_pts}</c:numCache></c:numRef></c:xVal>\
<c:yVal><c:numRef><c:numCache><c:ptCount val="{x_count}"/>{y_pts}</c:numCache></c:numRef></c:yVal></c:ser>"#,
        escape_xml(&series.name)
    )
}

/// Builds a `<p:graphicFrame>` element for a chart shape, referencing the chart
/// part via `r:id`. Used both for appending new charts and for regenerating a
/// chart frame when the original slide XML is being rewritten from scratch.
pub fn chart_graphic_frame_xml(chart: &ChartShape, id: &str, name: &str, rid: &str) -> String {
    let f = chart.transform.frame;
    let rot = if chart.transform.rotation != 0.0 {
        format!(
            " rot=\"{}\"",
            (chart.transform.rotation * 60_000.0).round() as i64
        )
    } else {
        String::new()
    };
    format!(
        "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"{id}\" name=\"{name}\"/>\
         <p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>\
         <p:xfrm{rot}><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></p:xfrm>\
         <a:graphic><a:graphicData uri=\"{CHART_GRAPHIC_URI}\">\
         <c:chart xmlns:c=\"{C_NS}\" r:id=\"{rid}\"/>\
         </a:graphicData></a:graphic></p:graphicFrame>",
        f.x, f.y, f.width, f.height
    )
}

/// Returns the next available `chart<N>.xml` index in the package.
pub fn next_chart_index(archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> usize {
    let mut max = 0usize;
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name();
            if let Some(rest) = name.strip_prefix("ppt/charts/chart") {
                if let Some(num) = rest.strip_suffix(".xml") {
                    if let Ok(n) = num.parse::<usize>() {
                        max = max.max(n);
                    }
                }
            }
        }
    }
    max + 1
}

/// Returns the content-type override for a chart part.
pub const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";

/// Returns the chart part path for a given index.
pub fn chart_part_path(index: usize) -> String {
    format!("ppt/charts/chart{index}.xml")
}
