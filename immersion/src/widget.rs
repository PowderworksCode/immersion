//! The widget kit: Blender-style controls, bound to a serde value.
//!
//! A property form is a `serde_json::Value` document plus a schema of
//! [`Field`]s, each addressing a spot in the document by JSON pointer. The
//! [`PropertyEditor`] renders the right control per field, reads its current
//! value out of the document, and reports an edit as `(pointer, new value)` —
//! both serde. It does not know or care *where* the document lives; the host
//! applies the edit (through its command bus, so a widget change persists and
//! undoes like any other). That is the "hooked up to serde in a good way": the
//! binding is a pointer into a value, the edit is a value, and the widget is
//! agnostic about the document's home.
//!
//! Every control commits once, on the liveview budget the rest of the library
//! holds to — a text field on blur/Enter, a slider and checkbox on change
//! (release, not drag), never a message per keystroke or per drag frame.

use dioxus::prelude::*;
use serde_json::{Value, json};

const SCRUB_JS: &str = include_str!("scrub.js");
const COLORPICKER_JS: &str = include_str!("colorpicker.js");

/// What kind of control edits a field, and its constraints.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    Text,
    Number {
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
    },
    /// A value slider with a filled track.
    Slider {
        min: f64,
        max: f64,
        step: f64,
    },
    Bool,
    /// `(value, label)` pairs for a dropdown.
    Select(Vec<(String, String)>),
    /// `(value, label)` pairs shown as a segmented button row — Blender's
    /// expanded enum. Same value as a Select, more direct for two or three
    /// options.
    Radio(Vec<(String, String)>),
    /// A bool shown as a single pressable button (pressed = true), rather than
    /// a checkbox — Blender's toggle button.
    Toggle,
    /// Several numbers on one row, addressed as a JSON array — Blender's
    /// vector fields (a location's X/Y/Z, a size's W/H). The labels name the
    /// components; the value is an array of that length.
    Vector {
        labels: Vec<String>,
        step: Option<f64>,
    },
    /// The native color picker; the value is a `#rrggbb` string.
    Color,
}

/// One editable spot in the document.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// JSON pointer, e.g. `/accent` or `/sweep/limit`.
    pub path: String,
    pub label: String,
    pub kind: FieldKind,
    pub hint: Option<String>,
    /// The factory value. When set, the field's right-click menu offers "Reset
    /// to default", which sets the pointer back to this.
    pub default: Option<Value>,
}

impl Field {
    pub fn new(path: &str, label: &str, kind: FieldKind) -> Self {
        Field {
            path: path.to_string(),
            label: label.to_string(),
            kind,
            hint: None,
            default: None,
        }
    }
    pub fn with_hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }
    pub fn with_default(mut self, default: Value) -> Self {
        self.default = Some(default);
        self
    }
}

#[derive(Props, Clone)]
pub struct PropertyEditorProps {
    /// The document being edited.
    pub doc: Value,
    pub fields: Vec<Field>,
    /// `(json pointer, new value)`. The host applies it to its own document.
    pub on_edit: Callback<(String, Value)>,
}

impl PartialEq for PropertyEditorProps {
    fn eq(&self, other: &Self) -> bool {
        self.doc == other.doc && self.fields == other.fields
    }
}

#[component]
pub fn PropertyEditor(props: PropertyEditorProps) -> Element {
    use_future(|| async {
        dioxus::document::eval(SCRUB_JS);
        dioxus::document::eval(COLORPICKER_JS);
    });
    rsx! {
        div { class: "im-props",
            for f in props.fields.iter().cloned() {
                {field_row(f, &props.doc, props.on_edit)}
            }
        }
    }
}

fn current<'a>(doc: &'a Value, path: &str) -> Option<&'a Value> {
    doc.pointer(path)
}

fn field_row(f: Field, doc: &Value, on_edit: Callback<(String, Value)>) -> Element {
    let val = current(doc, &f.path).cloned().unwrap_or(Value::Null);
    let control = match &f.kind {
        FieldKind::Text => text_widget(&f.path, &val, on_edit),
        FieldKind::Number { min, max, step } => {
            number_widget(&f.path, &val, *min, *max, *step, on_edit)
        }
        FieldKind::Slider { min, max, step } => {
            slider_widget(&f.path, &val, *min, *max, *step, on_edit)
        }
        FieldKind::Bool => bool_widget(&f.path, &val, on_edit),
        FieldKind::Select(opts) => select_widget(&f.path, &val, opts, on_edit),
        FieldKind::Radio(opts) => radio_widget(&f.path, &val, opts, on_edit),
        FieldKind::Toggle => toggle_widget(&f.path, &val, on_edit),
        FieldKind::Vector { labels, step } => vector_widget(&f.path, &val, labels, *step, on_edit),
        FieldKind::Color => color_widget(&f.path, &val, on_edit),
    };
    // A field with a default gets a right-click "Reset to default", routed to
    // set_setting through the same context-menu shim the areas use. Nested
    // inside the area's own menu, so `closest` finds this one first.
    // Blender's field menu: Reset to default (when the field has one), then
    // Copy / Paste value — the last two client-side (clipboard) in the shim.
    let menu = {
        let mut items = Vec::new();
        if let Some(d) = f.default.as_ref() {
            items.push(json!({
                "label": "Reset to default",
                "action": "set_setting",
                "params": { "pointer": f.path, "value": d },
            }));
            items.push(json!({ "sep": true }));
        }
        items.push(
            json!({ "label": "Copy value", "action": "copy_value", "params": { "value": val } }),
        );
        items.push(json!({ "label": "Paste value", "action": "paste_value", "params": { "pointer": f.path } }));
        Some(json!(items).to_string())
    };
    rsx! {
        div { class: "im-field", "data-im-menu": menu,
            label { class: "im-field-label",
                span { "{f.label}" }
                if let Some(h) = f.hint.clone() {
                    span { class: "im-field-hint", "{h}" }
                }
            }
            div { class: "im-field-control", {control} }
        }
    }
}

fn as_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn as_f64(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

fn text_widget(path: &str, val: &Value, on_edit: Callback<(String, Value)>) -> Element {
    let path = path.to_string();
    let cur = as_str(val);
    rsx! {
        input {
            class: "im-input",
            r#type: "text",
            value: "{cur}",
            spellcheck: "false",
            autocomplete: "off",
            // Commit on blur or Enter — one message, not one per keystroke.
            onchange: move |e| on_edit.call((path.clone(), json!(e.value()))),
        }
    }
}

fn number_widget(
    path: &str,
    val: &Value,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    on_edit: Callback<(String, Value)>,
) -> Element {
    let path = path.to_string();
    let cur = as_f64(val);
    let min_s = min.map(|m| m.to_string());
    let max_s = max.map(|m| m.to_string());
    let step_s = step.map(|s| s.to_string()).unwrap_or_else(|| "1".into());
    let min_attr = min.map(|m| m.to_string()).unwrap_or_default();
    let max_attr = max.map(|m| m.to_string()).unwrap_or_default();
    rsx! {
        input {
            class: "im-input im-number im-scrub",
            // Text, not number: a number input rejects "3*2" before the shim
            // can evaluate it. The shim resolves the expression on commit and
            // the parse below still guards what reaches the document.
            r#type: "text",
            inputmode: "decimal",
            value: "{cur}",
            min: min_s,
            max: max_s,
            step: "{step_s}",
            // Read by scrub.js — drag horizontally to change the value.
            "data-im-scrub": "1",
            "data-scrub-step": "{step_s}",
            "data-scrub-min": "{min_attr}",
            "data-scrub-max": "{max_attr}",
            onchange: move |e| {
                if let Ok(n) = e.value().parse::<f64>() {
                    // Keep integers integer in the document, so a limit reads
                    // `100`, not `100.0`, and round-trips to the host cleanly.
                    let v = if n.fract() == 0.0 { json!(n as i64) } else { json!(n) };
                    on_edit.call((path.clone(), v));
                }
            },
        }
    }
}

fn slider_widget(
    path: &str,
    val: &Value,
    min: f64,
    max: f64,
    step: f64,
    on_edit: Callback<(String, Value)>,
) -> Element {
    let path = path.to_string();
    let cur = as_f64(val);
    // Blender's slider is a flat bar filled to the value's fraction of the
    // range, the number centred on it — not a thumb on a rail. The native range
    // input rides on top invisibly so it still drags and commits once; the fill
    // is a gradient stopped at this percentage.
    let pct = if max > min {
        ((cur - min) / (max - min) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    rsx! {
        div { class: "im-slider-row",
            div { class: "im-slider", style: "--im-fill: {pct}%",
                input {
                    class: "im-slider-input",
                    r#type: "range",
                    min: "{min}",
                    max: "{max}",
                    step: "{step}",
                    value: "{cur}",
                    // range fires oninput during drag and onchange on release;
                    // use onchange so the commit is one message at the end.
                    onchange: move |e| {
                        if let Ok(n) = e.value().parse::<f64>() {
                            let v = if n.fract() == 0.0 { json!(n as i64) } else { json!(n) };
                            on_edit.call((path.clone(), v));
                        }
                    },
                }
                span { class: "im-slider-val", "{cur}" }
            }
        }
    }
}

fn bool_widget(path: &str, val: &Value, on_edit: Callback<(String, Value)>) -> Element {
    let path = path.to_string();
    let cur = val.as_bool().unwrap_or(false);
    rsx! {
        input {
            class: "im-check",
            r#type: "checkbox",
            checked: cur,
            onchange: move |e| on_edit.call((path.clone(), json!(e.checked()))),
        }
    }
}

fn select_widget(
    path: &str,
    val: &Value,
    opts: &[(String, String)],
    on_edit: Callback<(String, Value)>,
) -> Element {
    let path = path.to_string();
    let cur = as_str(val);
    let opts = opts.to_vec();
    rsx! {
        select {
            class: "im-select",
            onchange: move |e| on_edit.call((path.clone(), json!(e.value()))),
            for (value, label) in opts.iter().cloned() {
                option { value: "{value}", selected: value == cur, "{label}" }
            }
        }
    }
}

fn radio_widget(
    path: &str,
    val: &Value,
    opts: &[(String, String)],
    on_edit: Callback<(String, Value)>,
) -> Element {
    let path = path.to_string();
    let cur = as_str(val);
    let opts = opts.to_vec();
    rsx! {
        div { class: "im-radio",
            for (value, label) in opts.iter().cloned() {
                {
                    let p = path.clone();
                    let v = value.clone();
                    let active = value == cur;
                    rsx! {
                        button {
                            key: "{value}",
                            class: if active { "im-radio-btn active" } else { "im-radio-btn" },
                            onclick: move |_| on_edit.call((p.clone(), json!(v.clone()))),
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}

fn toggle_widget(path: &str, val: &Value, on_edit: Callback<(String, Value)>) -> Element {
    let path = path.to_string();
    let cur = val.as_bool().unwrap_or(false);
    rsx! {
        button {
            class: if cur { "im-toggle active" } else { "im-toggle" },
            onclick: move |_| on_edit.call((path.clone(), json!(!cur))),
            if cur { "On" } else { "Off" }
        }
    }
}

fn vector_widget(
    path: &str,
    val: &Value,
    labels: &[String],
    step: Option<f64>,
    on_edit: Callback<(String, Value)>,
) -> Element {
    let arr = val.as_array().cloned().unwrap_or_default();
    let step_s = step.map(|s| s.to_string()).unwrap_or_else(|| "1".into());
    rsx! {
        div { class: "im-vector",
            for (i, label) in labels.iter().cloned().enumerate() {
                {vector_part(path, &arr, i, labels.len(), label, &step_s, on_edit)}
            }
        }
    }
}

/// One component of a vector field. Its own function so the row does not nest
/// another four levels deep inside the loop.
fn vector_part(
    path: &str,
    arr: &[Value],
    i: usize,
    n: usize,
    label: String,
    step_s: &str,
    on_edit: Callback<(String, Value)>,
) -> Element {
    let cur = arr.get(i).and_then(Value::as_f64).unwrap_or(0.0);
    let p = path.to_string();
    let arr = arr.to_vec();
    let step_s = step_s.to_string();
    rsx! {
        label { class: "im-vec-part", key: "{i}",
            span { class: "im-vec-label", "{label}" }
            input {
                class: "im-input im-number im-scrub",
                r#type: "number",
                value: "{cur}",
                step: "{step_s}",
                "data-im-scrub": "1",
                "data-scrub-step": "{step_s}",
                // One component changes; the whole array is rewritten, so the
                // edit stays a single value at a single pointer.
                onchange: move |e| {
                    if let Ok(x) = e.value().parse::<f64>() {
                        let mut next: Vec<Value> =
                            (0..n).map(|k| arr.get(k).cloned().unwrap_or(json!(0))).collect();
                        next[i] = if x.fract() == 0.0 { json!(x as i64) } else { json!(x) };
                        on_edit.call((p.clone(), json!(next)));
                    }
                },
            }
        }
    }
}

fn color_widget(path: &str, val: &Value, on_edit: Callback<(String, Value)>) -> Element {
    let path = path.to_string();
    let cur = {
        let s = as_str(val);
        if s.is_empty() {
            "#000000".to_string()
        } else {
            s
        }
    };
    rsx! {
        div { class: "im-color-row",
            // The swatch opens the picker popup (built by the shim); the hex
            // field is the value itself — editable, pasteable, and the single
            // commit path both the popup and typing go through.
            button {
                class: "im-color",
                style: "background: {cur}",
                title: "pick a colour",
                "data-im-color-open": "1",
            }
            input {
                class: "im-color-hex",
                r#type: "text",
                value: "{cur}",
                spellcheck: "false",
                autocomplete: "off",
                "data-im-color-value": "1",
                onchange: move |e| on_edit.call((path.clone(), json!(e.value()))),
            }
        }
    }
}

/// Apply an edit to a document by JSON pointer, growing missing objects along
/// the way. Hosts use this so a widget edit lands where the pointer says, even
/// for a nested path the document did not have yet.
pub fn apply_edit(doc: &mut Value, pointer: &str, value: Value) {
    let parts: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
    if parts.is_empty() || parts == [""] {
        *doc = value;
        return;
    }
    let mut cur = doc;
    for (i, part) in parts.iter().enumerate() {
        if !cur.is_object() {
            *cur = json!({});
        }
        let obj = cur.as_object_mut().expect("just made it an object");
        if i == parts.len() - 1 {
            obj.insert((*part).to_string(), value);
            return;
        }
        cur = obj.entry((*part).to_string()).or_insert_with(|| json!({}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_edit_sets_a_top_level_key() {
        let mut doc = json!({ "accent": "#000000", "splash": true });
        apply_edit(&mut doc, "/accent", json!("#5680c2"));
        assert_eq!(doc["accent"], json!("#5680c2"));
        assert_eq!(doc["splash"], json!(true)); // untouched
    }

    #[test]
    fn apply_edit_grows_a_missing_nested_path() {
        let mut doc = json!({});
        apply_edit(&mut doc, "/sweep/limit", json!(100));
        assert_eq!(doc["sweep"]["limit"], json!(100));
    }

    #[test]
    fn a_field_reads_its_value_by_pointer() {
        let doc = json!({ "sweep": { "limit": 250 } });
        assert_eq!(current(&doc, "/sweep/limit"), Some(&json!(250)));
        assert_eq!(current(&doc, "/missing"), None);
    }
}
