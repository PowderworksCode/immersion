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
    /// A field refused input — the host echoes it to the status report, the
    /// second surface beside the field's own flag. Optional: a host that
    /// ignores errors just gets the field-local flag.
    #[props(default)]
    pub on_error: Option<Callback<EditorError>>,
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
    let on_error = props.on_error.unwrap_or_else(|| Callback::new(|_| {}));
    rsx! {
        div { class: "im-props",
            for f in props.fields.iter().cloned() {
                {field_row(f, &props.doc, props.on_edit, on_error)}
            }
        }
    }
}

fn current<'a>(doc: &'a Value, path: &str) -> Option<&'a Value> {
    doc.pointer(path)
}

fn field_row(
    f: Field,
    doc: &Value,
    on_edit: Callback<(String, Value)>,
    on_error: Callback<EditorError>,
) -> Element {
    let val = current(doc, &f.path).cloned().unwrap_or(Value::Null);
    let control = match &f.kind {
        FieldKind::Text => text_widget(&f.path, &val, on_edit),
        FieldKind::Number { min, max, step } => {
            number_widget(&f.path, &val, *min, *max, *step, on_edit, on_error)
        }
        FieldKind::Slider { min, max, step } => {
            slider_widget(&f.path, &val, *min, *max, *step, on_edit)
        }
        FieldKind::Bool => bool_widget(&f.path, &val, on_edit),
        FieldKind::Select(opts) => select_widget(&f.path, &val, opts, on_edit),
        FieldKind::Radio(opts) => radio_widget(&f.path, &val, opts, on_edit),
        FieldKind::Toggle => toggle_widget(&f.path, &val, on_edit),
        FieldKind::Vector { labels, step } => {
            vector_widget(&f.path, &val, labels, *step, on_edit, on_error)
        }
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

/// The one text-number control, shared by the scalar field and each vector
/// component. A component rather than a function because a field that can
/// refuse input needs somewhere to remember that it did: on a bad expression
/// it flags itself (`im-invalid`, the message in `title`) and hands the error
/// up; the next successful commit — typed or scrubbed — clears the flag.
#[component]
fn NumberInput(
    value: f64,
    step_s: String,
    #[props(default)] min_attr: String,
    #[props(default)] max_attr: String,
    on_commit: Callback<f64>,
    on_error: Callback<EditorError>,
) -> Element {
    let mut error = use_signal(|| None::<String>);
    rsx! {
        input {
            class: if error().is_some() { "im-input im-number im-scrub im-invalid" } else { "im-input im-number im-scrub" },
            // Text, not number: a number input does not merely reject "3*2",
            // it strips the operator and commits 32. The evaluation happens
            // here, on commit, and the document receives a number.
            r#type: "text",
            inputmode: "decimal",
            value: "{value}",
            // The message rides the tooltip, so hovering the red field says
            // why it is red.
            title: error().unwrap_or_default(),
            min: if min_attr.is_empty() { None } else { Some(min_attr.clone()) },
            max: if max_attr.is_empty() { None } else { Some(max_attr.clone()) },
            step: "{step_s}",
            // Read by scrub.js — drag horizontally to change the value.
            "data-im-scrub": "1",
            "data-scrub-step": "{step_s}",
            "data-scrub-min": "{min_attr}",
            "data-scrub-max": "{max_attr}",
            onchange: move |e| match eval_expr(&e.value()) {
                Ok(n) => {
                    error.set(None);
                    on_commit.call(n);
                }
                Err(err) => {
                    error.set(Some(err.to_string()));
                    on_error.call(err);
                }
            },
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
    on_error: Callback<EditorError>,
) -> Element {
    let path = path.to_string();
    let cur = as_f64(val);
    let step_s = step.map(|s| s.to_string()).unwrap_or_else(|| "1".into());
    let min_attr = min.map(|m| m.to_string()).unwrap_or_default();
    let max_attr = max.map(|m| m.to_string()).unwrap_or_default();
    let on_commit = Callback::new(move |n: f64| {
        // Keep integers integer in the document, so a limit reads `100`, not
        // `100.0`, and round-trips to the host cleanly.
        let v = if n.fract() == 0.0 {
            json!(n as i64)
        } else {
            json!(n)
        };
        on_edit.call((path.clone(), v));
    });
    rsx! {
        NumberInput {
            value: cur,
            step_s,
            min_attr,
            max_attr,
            on_commit,
            on_error,
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
    on_error: Callback<EditorError>,
) -> Element {
    let arr = val.as_array().cloned().unwrap_or_default();
    let step_s = step.map(|s| s.to_string()).unwrap_or_else(|| "1".into());
    let n = labels.len();
    rsx! {
        div { class: "im-vector",
            for (i, label) in labels.iter().cloned().enumerate() {
                {
                    // One component changes; the whole array is rewritten, so
                    // the edit stays a single value at a single pointer.
                    let p = path.to_string();
                    let cur = arr.get(i).and_then(Value::as_f64).unwrap_or(0.0);
                    let arr = arr.clone();
                    let on_commit = Callback::new(move |x: f64| {
                        let mut next: Vec<Value> = (0..n)
                            .map(|k| arr.get(k).cloned().unwrap_or(json!(0)))
                            .collect();
                        next[i] = if x.fract() == 0.0 { json!(x as i64) } else { json!(x) };
                        on_edit.call((p.clone(), json!(next)));
                    });
                    vector_part(i, label, cur, &step_s, on_commit, on_error)
                }
            }
        }
    }
}

/// One component of a vector field. Its own function so the row does not nest
/// another four levels deep inside the loop.
fn vector_part(
    i: usize,
    label: String,
    cur: f64,
    step_s: &str,
    on_commit: Callback<f64>,
    on_error: Callback<EditorError>,
) -> Element {
    let step_s = step_s.to_string();
    rsx! {
        label { class: "im-vec-part", key: "{i}",
            span { class: "im-vec-label", "{label}" }
            NumberInput {
                value: cur,
                step_s,
                on_commit,
                on_error,
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
/// for a nested path the document did not have yet./// Evaluate what someone typed into a number field: a plain number, or a small
/// arithmetic expression — `3*2`, `1920/2`, `pi*100`. Blender allows this and
/// it is genuinely useful, but it belongs here rather than in the shim: it runs
/// once, on commit, on a message the server already receives. Only frame-path
/// work — a drag preview, filtering as you type — has to live in the browser.
///
/// The grammar is declared below rather than hand-parsed, so precedence and
/// unary signs are the parser generator's problem and a change is an edit to
/// the grammar rather than to a shunting-yard loop.
///
/// Deliberately not a general evaluator: four operators, parentheses and three
/// constants. There are no variables and no function calls, so there is
/// nothing here for a hostile string to reach.
pub fn eval_number(src: &str) -> Option<f64> {
    eval_expr(src).ok()
}

/// What went wrong, in words a person can act on. One error type for every
/// editor surface: the expression parser fills `column`; a failed command or
/// a bad chart spec leaves it `None`. The host shows the same value two ways,
/// Blender-style — the field flags itself, and the status report echoes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorError {
    pub message: String,
    /// 1-based, so it lines up with a caret under the text as typed. `None`
    /// when the error has no position (a command failure, a whole-document
    /// rejection).
    pub column: Option<usize>,
}

impl EditorError {
    /// An error with no position — a command failure, a rejected document.
    pub fn message(message: impl Into<String>) -> Self {
        EditorError {
            message: message.into(),
            column: None,
        }
    }
}

impl std::fmt::Display for EditorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Evaluate, or say why not.
pub fn eval_expr(src: &str) -> Result<f64, EditorError> {
    use chumsky::Parser;
    let text = src.trim().to_ascii_lowercase();
    if text.is_empty() {
        return Err(EditorError {
            message: "nothing to evaluate".into(),
            column: Some(1),
        });
    }
    let parsed = expression().parse(&text);
    if let Some(v) = parsed.output().copied() {
        // Infinity and NaN are not field values: `1/0` leaves the field alone
        // rather than storing something the document cannot round-trip.
        return if v.is_finite() {
            Ok(v)
        } else {
            Err(EditorError {
                message: "not a finite number".into(),
                column: Some(1),
            })
        };
    }
    // `Rich` borrows the input, so each error is rendered to an owned string
    // here rather than handed up the stack.
    Err(parsed.into_errors().into_iter().next().map_or_else(
        || EditorError {
            message: "not an expression".into(),
            column: Some(1),
        },
        |e| EditorError {
            message: e.to_string(),
            column: Some(e.span().start + 1),
        },
    ))
}

/// The grammar: numbers with optional exponents, three constants, unary signs,
/// the four operators with the usual precedence, and parentheses. `labelled`
/// is what makes a failure readable — without it the expectation set is the
/// raw character classes the parser tried.
fn expression<'a>()
-> impl chumsky::Parser<'a, &'a str, f64, chumsky::extra::Err<chumsky::error::Rich<'a, char>>> {
    use chumsky::prelude::*;

    recursive(|expr| {
        let digits = text::digits(10).at_least(1);
        let number = digits
            .then(just('.').then(text::digits(10)).or_not())
            // The `e` of `1e3` belongs to the literal; the `e` of `2*e` is
            // Euler's number. Only an `e` between digits is an exponent.
            .then(just('e').then(one_of("+-").or_not()).then(digits).or_not())
            .to_slice()
            .from_str::<f64>()
            .unwrapped();

        let konst = choice((
            just("pi").to(std::f64::consts::PI),
            just("tau").to(std::f64::consts::TAU),
            just("e").to(std::f64::consts::E),
        ));

        let atom = choice((number, konst, expr.delimited_by(just('('), just(')'))))
            .labelled("a number")
            .padded();

        // A leading `-` is a sign, not a subtraction, and it may repeat:
        // `2--3` is 5.
        let signed = one_of("+-")
            .padded()
            .repeated()
            .foldr(atom, |sign, v| if sign == '-' { -v } else { v });

        let product = signed.clone().foldl(
            one_of("*/").padded().then(signed).repeated(),
            |l, (op, r)| if op == '*' { l * r } else { l / r },
        );

        product.clone().foldl(
            one_of("+-").padded().then(product).repeated(),
            |l, (op, r)| if op == '+' { l + r } else { l - r },
        )
    })
    .padded()
}

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
    fn a_number_field_takes_arithmetic() {
        assert_eq!(eval_number("100"), Some(100.0));
        assert_eq!(eval_number(" 2.5 "), Some(2.5));
        assert_eq!(eval_number("3*2"), Some(6.0));
        assert_eq!(eval_number("1920/2"), Some(960.0));
        assert_eq!(
            eval_number("2+3*4"),
            Some(14.0),
            "precedence, not left-to-right"
        );
        assert_eq!(eval_number("(2+3)*4"), Some(20.0));
        assert_eq!(eval_number("-5"), Some(-5.0));
        assert_eq!(
            eval_number("10*-2"),
            Some(-20.0),
            "a sign, not a subtraction"
        );
        assert_eq!(eval_number("10/-2"), Some(-5.0));
        assert_eq!(eval_number("2--3"), Some(5.0));
        assert_eq!(eval_number("-(2+3)"), Some(-5.0));
        assert_eq!(eval_number("10*-(1+1)"), Some(-20.0));
        assert_eq!(eval_number("+7"), Some(7.0));
        assert_eq!(eval_number("pi").map(|v| (v * 100.0).round()), Some(314.0));
        // Exponent notation, which the shim handled and a bare digit scanner
        // would reject: the `e` here is part of the literal, not the constant.
        assert_eq!(eval_number("1e3"), Some(1000.0));
        assert_eq!(eval_number("2*1e3"), Some(2000.0));
        assert_eq!(eval_number("2e-3"), Some(0.002));
        assert_eq!(eval_number("2*e").map(|v| (v * 100.0).round()), Some(544.0));
    }

    #[test]
    fn nonsense_leaves_the_field_alone() {
        // None means "not a number" — the widget commits nothing and the field
        // keeps what it had, rather than storing a zero.
        for bad in ["abc", "", "   ", "3*", "(2+3", "2+3)", "1 2", "drop table"] {
            assert_eq!(eval_number(bad), None, "{bad:?} should not evaluate");
        }
    }

    #[test]
    fn a_refused_expression_can_say_why() {
        // eval_number throws the reason away; a caller that wants to show the
        // user something has one. These are the parser's own words, so the
        // assertions are on the useful parts rather than the exact phrasing.
        let e = eval_expr("3*").expect_err("trailing operator");
        assert!(e.message.contains("a number"), "{}", e.message);
        assert_eq!(
            e.column,
            Some(3),
            "points at the operator with nothing after it"
        );

        let e = eval_expr("2+3)").expect_err("unbalanced");
        assert_eq!(e.column, Some(4));
        assert!(e.message.contains(')'), "{}", e.message);

        let e = eval_expr("drop table").expect_err("not arithmetic");
        assert_eq!(e.column, Some(1));

        assert_eq!(eval_expr("  ").unwrap_err().message, "nothing to evaluate");
        assert_eq!(eval_expr("1/0").unwrap_err().message, "not a finite number");
    }

    #[test]
    fn division_by_zero_is_not_a_value() {
        assert_eq!(eval_number("1/0"), None, "infinity is not a field value");
    }

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
