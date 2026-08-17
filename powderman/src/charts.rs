//! Charts: specs in, drawings out.
//!
//! A chart is a Vega-Lite spec living at `/charts/<name>` in the settings
//! document. This module resolves one for rendering — filling in the feeds it
//! names, turning the workbench chrome off, and declining to draw a window
//! with nothing in it — and supplies the editor and sidebar around that.
//!
//! Split out of `editors.rs`, which was about what an area shows; this is
//! about one editor's language.

use dioxus::prelude::*;
use immersion::{Panel, PropertyEditor};

use crate::editors::stamp_of;
use crate::ui::State;

/// The chart editor: a Vega-Lite spec, drawn.
///
/// The spec is a document — it lives in settings under `/charts/<name>`, so
/// the data editor browses it, a widget could edit it by pointer, and an
/// agent writes one with `set_setting` and no new tool. What arrives here is
/// the spec plus whatever data its feed names; the vendored renderer draws.
pub(crate) fn ed_chart(s: &State, doc: &serde_json::Value, target: Option<String>) -> Element {
    let names = chart_names(doc);
    let Some(name) = target.filter(|t| !t.is_empty()) else {
        return rsx! {
            div { class: "empty",
                if names.is_empty() {
                    "No charts yet \u{2014} add one under /charts in the settings document."
                } else {
                    "Pick a chart \u{2014} the target chip in the header."
                }
            }
        };
    };
    let key = name.trim_start_matches("/charts/");
    let Some(spec) = doc.pointer(&format!("/charts/{}", escape_pointer(key))) else {
        return rsx! { div { class: "empty", "no chart called {key}" } };
    };
    let _ = spec;
    rsx! {
        div { class: "code-view",
            div { class: "code-path", "{key}" }
            {chart_element(s, doc, key)}
        }
    }
}

/// One named chart, resolved and handed to the renderer. Shared by the chart
/// editor and any editor that wants a chart of its own — the machine editor
/// draws its CPU and memory plots through here, so those two are documents
/// anyone can edit rather than a drawing routine only a Rust change can
/// alter.
pub(crate) fn chart_element(s: &State, doc: &serde_json::Value, name: &str) -> Element {
    let Some(spec) = doc.pointer(&format!("/charts/{}", escape_pointer(name))) else {
        return rsx! { div { class: "empty", "no chart called {name}" } };
    };
    match resolve_spec(s, doc, spec) {
        Ok(resolved) if nothing_to_draw(&resolved) => rsx! {
            div { class: "empty", "no data in this window" }
        },
        Err(e) => rsx! { div { class: "chart-error-inline", "{e}" } },
        Ok(resolved) => {
            let json = serde_json::to_string(&resolved).unwrap_or_default();
            let stamp = stamp_of(name, &json);
            rsx! {
                pre { class: "code-src-payload", "data-im-chart-src": "{stamp}", "{json}" }
                div { class: "chart-host", "data-im-chart": "{stamp}" }
            }
        }
    }
}

/// The chart editor's sidebar: the spec itself, editable, plus a way to make
/// a new one. This is where a person creates a chart — an agent already can,
/// with `set_setting` and a pointer, and it would be a poor sort of parity if
/// the person could not.
pub(crate) fn chart_sidebar(
    doc: &serde_json::Value,
    target: Option<String>,
    on_setting: Callback<(String, serde_json::Value)>,
    on_error: Callback<immersion::EditorError>,
) -> Element {
    let name = target
        .as_deref()
        .map(|t| t.trim_start_matches("/charts/").to_string());
    // A name nobody has used yet, so the button never overwrites a chart.
    let taken = chart_names(doc);
    let fresh = (1..)
        .map(|n| {
            if n == 1 {
                "new chart".to_string()
            } else {
                format!("new chart {n}")
            }
        })
        .find(|c| !taken.contains(c))
        .unwrap_or_else(|| "new chart".to_string());
    let template = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": { "name": "cpu" },
        "mark": "line",
        "encoding": {
            "x": { "field": "at", "type": "temporal", "title": null },
            "y": { "field": "value", "type": "quantitative" }
        }
    });
    let fresh_pointer = format!("/charts/{}", escape_pointer(&fresh));
    rsx! {
        div { class: "area-props",
            Panel { title: "Chart",
                button {
                    class: "im-btn chart-new",
                    title: "add a chart to the settings document",
                    onclick: move |_| on_setting.call((fresh_pointer.clone(), template.clone())),
                    "+ New chart"
                }
                if let Some(name) = name {
                    div { class: "area-props-row",
                        span { class: "k", "Name" }
                        span { "{name}" }
                    }
                    PropertyEditor {
                        doc: doc.clone(),
                        fields: vec![
                            immersion::Field::new(
                                &format!("/charts/{}", escape_pointer(&name)),
                                "Spec",
                                immersion::FieldKind::Json,
                            )
                            .with_hint("Vega-Lite; data.name picks a feed"),
                        ],
                        on_edit: on_setting,
                        on_error,
                    }
                }
            }
        }
    }
}

/// The chart names, for the picker and the empty state.
pub(crate) fn chart_names(doc: &serde_json::Value) -> Vec<String> {
    doc["charts"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// Replace a named feed with the host's own data, and refuse a spec that is
/// not one. Validation is deliberately structural rather than a full schema
/// check: the renderer reports its own errors precisely, and the value of
/// checking here is catching the two mistakes that would otherwise render a
/// blank panel — a spec that is not an object, and a feed nobody serves.
fn resolve_spec(
    s: &State,
    doc: &serde_json::Value,
    spec: &serde_json::Value,
) -> Result<serde_json::Value, immersion::EditorError> {
    if !spec.is_object() {
        return Err(immersion::EditorError::message(
            "a chart is a Vega-Lite spec: a JSON object",
        ));
    }
    let mut out = spec.clone();
    // Feeds are resolved wherever they appear, not just at the top: a layered
    // or faceted spec carries a `data` per layer, and resolving only the
    // outer one leaves the inner layers asking for a name the renderer has
    // never heard of.
    resolve_feeds(s, doc, &mut out)?;
    let obj = out.as_object_mut().expect("checked above");
    // The workbench is dark and Vega's defaults are not, so a spec arrives
    // with the chrome turned off: no white plate, no view border. The rest of
    // the theming is CSS against the rendered SVG, where the palette tokens
    // already live — a colour written here would be a second palette.
    obj.entry("background")
        .or_insert(serde_json::Value::String("transparent".into()));
    // Fill the area. Both halves are needed: "container" asks the renderer to
    // measure its parent, and the fitting autosize is what makes it re-measure
    // rather than draw at the default size and clip.
    obj.entry("width")
        .or_insert(serde_json::Value::String("container".into()));
    obj.entry("height")
        .or_insert(serde_json::Value::String("container".into()));
    obj.entry("autosize")
        .or_insert(serde_json::json!({ "type": "fit", "contains": "padding" }));
    if let Some(cfg) = obj.get_mut("config").and_then(|c| c.as_object_mut()) {
        cfg.entry("view")
            .or_insert(serde_json::json!({ "stroke": null }));
    } else {
        obj.insert(
            "config".into(),
            serde_json::json!({ "view": { "stroke": null } }),
        );
    }
    Ok(out)
}

/// Replace every `data: { name: … }` in a spec with the host's values for
/// that feed. Walks objects and arrays, so layers, concatenations and facets
/// are reached wherever Vega-Lite allows a data block.
fn resolve_feeds(
    s: &State,
    doc: &serde_json::Value,
    node: &mut serde_json::Value,
) -> Result<(), immersion::EditorError> {
    match node {
        serde_json::Value::Object(map) => {
            if let Some(feed) = map
                .get("data")
                .and_then(|d| d.get("name"))
                .and_then(|n| n.as_str())
                .map(str::to_string)
            {
                let values = feed_values(s, doc, &feed).ok_or_else(|| {
                    immersion::EditorError::message(format!(
                        "no feed called {feed:?} \u{2014} this host serves {}",
                        FEEDS.join(", ")
                    ))
                })?;
                map.insert(
                    "data".into(),
                    serde_json::json!({ "values": values, "name": feed }),
                );
            }
            for (key, child) in map.iter_mut() {
                // `data` is done; descending into it would rewrite the values
                // just inserted, and nothing inside a data block is a spec.
                if key != "data" {
                    resolve_feeds(s, doc, child)?;
                }
            }
            // After the children, not before: a layer's feed is resolved by
            // the recursion above, so dropping empties first would inspect
            // layers that had no values yet and keep all of them. Vega warns
            // "Infinite extent" for every field of an empty layer and draws it
            // anyway; the honest rendering of "no run was alive in this
            // window" is no shading, not a warning per axis.
            if let Some(layers) = map.get_mut("layer").and_then(|l| l.as_array_mut()) {
                layers.retain(|l| !is_empty_layer(l));
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for item in items {
                resolve_feeds(s, doc, item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Whether a resolved spec has any rows at all. A chart of an empty window
/// says so, rather than drawing bare axes over nothing — which is what Vega
/// does, loudly, if handed empty values.
fn nothing_to_draw(spec: &serde_json::Value) -> bool {
    match spec.get("layer").and_then(|l| l.as_array()) {
        // Every layer named a feed and every feed was empty.
        Some(layers) => layers.is_empty(),
        None => spec
            .pointer("/data/values")
            .and_then(|v| v.as_array())
            .is_some_and(|rows| rows.is_empty()),
    }
}

/// Whether a layer has been left with no rows to draw. Only a layer that
/// named a feed counts: a layer with inline values, or one that inherits the
/// parent's data, is the author's business.
fn is_empty_layer(layer: &serde_json::Value) -> bool {
    layer
        .pointer("/data/values")
        .and_then(|v| v.as_array())
        .is_some_and(|rows| rows.is_empty())
}

/// The feeds a spec may name. Small and explicit: a chart that asks for
/// something else gets told what is on offer rather than an empty panel.
const FEEDS: &[&str] = &["cpu", "memory", "runs", "fleet", "annotations"];

fn feed_values(s: &State, doc: &serde_json::Value, feed: &str) -> Option<Vec<serde_json::Value>> {
    let series = |points: &[(i64, f64)]| {
        points
            .iter()
            .map(|(at, value)| serde_json::json!({ "at": at, "value": value }))
            .collect::<Vec<_>>()
    };
    Some(match feed {
        "cpu" => series(&s.cpu),
        "memory" => {
            // As a percentage of the box, which is what a reader of a memory
            // chart actually wants to know.
            let total = s.machine.get("box.mem_total").copied().unwrap_or(1.0);
            s.mem
                .iter()
                .map(|(at, used)| {
                    serde_json::json!({ "at": at, "value": used / total.max(1.0) * 100.0 })
                })
                .collect()
        }
        "runs" => s
            .runs
            .iter()
            .map(|r| {
                serde_json::json!({
                    "workflow": r.workflow,
                    "status": r.status,
                    "at": r.updated_at,
                })
            })
            .collect(),
        // The run windows, for charts that shade them behind a series.
        "annotations" => s
            .annotations
            .iter()
            .map(|a| {
                serde_json::json!({
                    "from": a.from,
                    "to": a.to,
                    "status": a.status,
                    "workflow": a.workflow,
                })
            })
            .collect(),
        "fleet" => s
            .fleet
            .iter()
            .map(|a| {
                serde_json::json!({
                    "name": a.name,
                    "status": a.status,
                    "rss": a.procs.iter().map(|p| p.rss).sum::<f64>(),
                })
            })
            .collect(),
        // A chart may also name a document the host keeps: anything under
        // /charts is a spec, and a sibling array under /feeds is data.
        _ => doc.pointer(&format!("/feeds/{feed}"))?.as_array()?.clone(),
    })
}

/// JSON-pointer escaping for a chart name, which is a user-chosen key and may
/// contain a slash.
fn escape_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod chart_editor {
    use super::*;

    fn state() -> State {
        State {
            cpu: vec![(1, 12.0), (2, 40.0)],
            mem: vec![(1, 2.0), (2, 4.0)],
            machine: [("box.mem_total".to_string(), 8.0)].into_iter().collect(),
            runs: vec![crate::ui::RunView {
                id: "a".into(),
                workflow: "sweep".into(),
                status: "done".into(),
                note: None,
                error: None,
                updated_at: 9,
                steps: vec![],
            }],
            ..Default::default()
        }
    }

    #[test]
    fn a_named_feed_becomes_the_spec_s_data() {
        // The point of the design: the spec says which data it wants, the
        // host supplies it, and what reaches the renderer is one document.
        let doc = crate::daemon::settings_defaults();
        let spec = doc
            .pointer("/charts/memory")
            .expect("the seeded memory chart");
        let out = resolve_spec(&state(), &doc, spec).expect("resolves");
        let values = out["data"]["values"].as_array().expect("values inlined");
        assert_eq!(values.len(), 2);
        assert!(out["data"].get("name").is_some(), "the feed is still named");
        assert_eq!(out["mark"]["type"], "area", "the rest of the spec survives");
    }

    /// A layered spec carries one feed per layer. Resolving only the outer
    /// data block leaves the inner layers asking the renderer for a name it
    /// has never heard of, which is a chart that silently draws nothing —
    /// the shape the machine editor's cpu plot takes.
    #[test]
    fn every_layer_s_feed_is_resolved() {
        let doc = crate::daemon::settings_defaults();
        let spec = doc.pointer("/charts/cpu").expect("the seeded cpu chart");
        // With a run in the window, so both layers have rows — an empty layer
        // is dropped now, and this test is about resolution, not dropping.
        let mut s = state();
        s.annotations = vec![crate::ui::Annotation {
            id: "a".into(),
            workflow: "sweep".into(),
            from: 1,
            to: 2,
            status: "done".into(),
        }];
        let out = resolve_spec(&s, &doc, spec).expect("resolves");
        let layers = out["layer"].as_array().expect("layered");
        assert_eq!(layers.len(), 2);
        for layer in layers {
            assert!(
                layer["data"]["values"].is_array(),
                "a layer kept an unresolved feed: {layer}"
            );
        }
        // The annotation layer gets the run windows, the line layer the series.
        assert_eq!(layers[0]["data"]["values"].as_array().unwrap().len(), 1);
        assert_eq!(layers[1]["data"]["values"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn memory_is_served_as_a_percentage() {
        let doc = crate::daemon::settings_defaults();
        let spec = serde_json::json!({ "data": { "name": "memory" }, "mark": "line" });
        let out = resolve_spec(&state(), &doc, &spec).expect("resolves");
        // 4 of 8 bytes is 50%, which is what a memory chart should plot.
        assert_eq!(out["data"]["values"][1]["value"], 50.0);
    }

    #[test]
    fn a_spec_that_cannot_be_drawn_says_why() {
        let doc = crate::daemon::settings_defaults();
        let not_object = serde_json::json!([1, 2, 3]);
        let e = resolve_spec(&state(), &doc, &not_object).expect_err("refused");
        assert!(e.message.contains("JSON object"), "{}", e.message);

        let bad_feed = serde_json::json!({ "data": { "name": "nope" }, "mark": "line" });
        let e = resolve_spec(&state(), &doc, &bad_feed).expect_err("refused");
        assert!(e.message.contains("nope"), "{}", e.message);
        assert!(
            e.message.contains("cpu"),
            "and lists what is served: {}",
            e.message
        );
    }

    #[test]
    fn the_picker_offers_the_charts_that_exist() {
        let doc = crate::daemon::settings_defaults();
        let names = chart_names(&doc);
        assert!(names.contains(&"cpu".to_string()), "{names:?}");
        assert_eq!(crate::editors::target_noun("chart"), "Chart");
    }
}

#[cfg(test)]
mod empty_charts {
    use super::*;

    /// The reported symptom: a woken demo drew charts whose annotation layer
    /// had no rows, and Vega logged "Infinite extent" for every field of it.
    /// An empty layer is dropped instead — after its feed resolves, which is
    /// the part the first attempt got wrong.
    #[test]
    fn an_empty_layer_is_dropped_after_its_feed_resolves() {
        let doc = crate::daemon::settings_defaults();
        // A state with cpu samples but no runs: exactly a quiet box.
        let s = State {
            cpu: vec![(1, 10.0), (2, 20.0)],
            annotations: vec![],
            ..Default::default()
        };
        let spec = doc.pointer("/charts/cpu").expect("the seeded cpu chart");
        let out = resolve_spec(&s, &doc, spec).expect("resolves");
        let layers = out["layer"].as_array().expect("still layered");
        assert_eq!(
            layers.len(),
            1,
            "the annotation layer went, the line stayed"
        );
        assert_eq!(layers[0]["data"]["values"].as_array().unwrap().len(), 2);
        assert!(!nothing_to_draw(&out), "there is still a line to draw");
    }

    #[test]
    fn a_chart_with_no_data_at_all_says_so() {
        let doc = crate::daemon::settings_defaults();
        let s = State::default(); // nothing anywhere
        let out = resolve_spec(&s, &doc, doc.pointer("/charts/cpu").unwrap()).expect("resolves");
        assert!(
            nothing_to_draw(&out),
            "every layer was empty, so there is nothing to draw: {out}"
        );
        // And the single-feed shape too, which has no layers to count.
        let flat = serde_json::json!({ "data": { "name": "cpu" }, "mark": "line" });
        let out = resolve_spec(&s, &doc, &flat).expect("resolves");
        assert!(nothing_to_draw(&out));
    }
}

/// The bottom line: which chart, and how many points are in the window it is
/// drawing. A chart that looks empty and a chart with nothing to draw look
/// the same until something says which it is.
pub(crate) fn footer(d: &crate::editors::Draw) -> String {
    let Some(name) = d
        .arg
        .as_deref()
        .map(|t| t.trim_start_matches("/charts/"))
        .filter(|n| !n.is_empty())
    else {
        return String::new();
    };
    let points = d.state.cpu.len().max(d.state.mem.len());
    format!("{name} · {points} points")
}

/// This editor's entry in the registry: what it is called, how it is drawn in
/// a header, whether it takes a target, and what the status bar says while it
/// has focus. Declared beside the editor so adding one is one file.
pub(crate) fn kind() -> immersion::EditorKind {
    immersion::EditorKind {
        id: "chart",
        label: "Chart",
        icon: "chart-line",
        hints: &[("Chip", "Pick chart"), ("N", "Edit spec")],
        targets: true,
    }
}
