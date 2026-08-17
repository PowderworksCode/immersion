//! The reference, generated from the things it documents.
//!
//! Every surface this describes already exists as data: the bus registry
//! knows every command and its description, the MCP router knows every tool
//! and the JSON Schema of its parameters, the keymap knows every chord, and
//! the editor registry knows every editor and what it answers to. Written
//! prose beside all of that would be a fifth list to keep in step, and the
//! one that nothing checks.
//!
//! So there is no prose. `reference()` reads the four registries and
//! [`markdown`] renders them; `docs/reference.md` is that rendering, held to
//! the code by a test, and the Help editor draws the same value so the page
//! in the workbench cannot disagree with the file in the repository.
//!
//! `powderman --reference` prints it too, which is how you ask a binary what
//! it can do without starting it.

/// One documented entry, whatever kind of thing it is.
pub(crate) struct Entry {
    pub name: String,
    pub detail: String,
    /// `(name, type, required, description)` for a tool's parameters. Empty
    /// for anything that does not take any.
    pub params: Vec<Param>,
}

pub(crate) struct Param {
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub about: String,
}

/// A titled run of entries — one section of the reference.
pub(crate) struct Section {
    pub title: &'static str,
    /// One line under the heading saying what the section is.
    pub about: &'static str,
    pub entries: Vec<Entry>,
}

/// The whole reference, read out of the registries.
pub(crate) fn reference() -> Vec<Section> {
    vec![
        commands_section(),
        tools_section(),
        keymap_section(),
        editors_section(),
    ]
}

fn commands_section() -> Section {
    let commands = crate::workflows::commands();
    let mut entries: Vec<Entry> = commands
        .iter()
        .map(|c| Entry {
            name: c.name.to_string(),
            detail: c.description.to_string(),
            params: Vec::new(),
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Section {
        title: "Commands",
        about: "The write path. Every layout change — a button, a chord, a \
                gesture, an agent — arrives as one of these.",
        entries,
    }
}

fn tools_section() -> Section {
    let mut entries: Vec<Entry> = crate::mcp::tools()
        .into_iter()
        .map(|t| Entry {
            name: t.name.to_string(),
            detail: t.description.map(|d| d.to_string()).unwrap_or_default(),
            params: params_of(&t.input_schema),
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Section {
        title: "MCP tools",
        about: "What an agent can do, which is everything a person can do to \
                server truth. A command without a tool here is a test failure, \
                not an omission.",
        entries,
    }
}

/// The parameters out of a tool's JSON Schema. The descriptions are the doc
/// comments on the argument structs, so the schema carries prose someone
/// already wrote next to the code.
fn params_of(schema: &serde_json::Map<String, serde_json::Value>) -> Vec<Param> {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    let mut out: Vec<Param> = props
        .iter()
        .map(|(name, def)| Param {
            name: name.clone(),
            kind: schema_kind(schema, def),
            required: required.contains(&name.as_str()),
            about: def
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or_default()
                .to_string(),
        })
        .collect();
    out.sort_by(|a, b| b.required.cmp(&a.required).then(a.name.cmp(&b.name)));
    out
}

/// A readable type for a schema node. Enough for a reference: the exact
/// schema is what the agent receives, and this is what a person reads.
///
/// An enum arrives as a `$ref` into the schema's own definitions, so a naive
/// reader calls the one parameter with a fixed set of values "any" — which is
/// the opposite of what the reader wants to know. Following the reference is
/// the difference between "dir: any" and "dir: row | col".
fn schema_kind(
    root: &serde_json::Map<String, serde_json::Value>,
    def: &serde_json::Value,
) -> String {
    if let Some(name) = def
        .get("$ref")
        .and_then(|r| r.as_str())
        .and_then(|r| r.rsplit('/').next())
    {
        let found = ["$defs", "definitions"]
            .iter()
            .filter_map(|bag| root.get(*bag))
            .filter_map(|bag| bag.get(name))
            .next()
            .cloned();
        if let Some(target) = found {
            return schema_kind(root, &target);
        }
    }
    // Two spellings of the same thing. `enum` is the plain list; schemars
    // writes a Rust unit enum as `oneOf` over `const`s so that each variant
    // can carry its own doc comment — which is the shape every enum here has,
    // so reading only the first reports "any" for exactly the parameter a
    // reader most needs the values of.
    if let Some(list) = def.get("enum").and_then(|e| e.as_array()) {
        return join_values(list);
    }
    for key in ["oneOf", "anyOf"] {
        let Some(list) = def.get(key).and_then(|v| v.as_array()) else {
            continue;
        };
        let consts: Vec<serde_json::Value> = list
            .iter()
            .filter_map(|v| v.get("const"))
            .cloned()
            .collect();
        if consts.len() == list.len() && !consts.is_empty() {
            return join_values(&consts);
        }
    }
    match def.get("type") {
        Some(serde_json::Value::String(t)) => t.clone(),
        // A nullable field arrives as a list of types; the null is the
        // optionality, which the required column already says.
        Some(serde_json::Value::Array(types)) => types
            .iter()
            .filter_map(|t| t.as_str())
            .filter(|t| *t != "null")
            .collect::<Vec<_>>()
            .join(" | "),
        _ => "any".to_string(),
    }
}

/// JSON values as the words a reader would say: strings unquoted, everything
/// else as written.
fn join_values(list: &[serde_json::Value]) -> String {
    list.iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string())
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn keymap_section() -> Section {
    let mut entries: Vec<Entry> = immersion::default_keymap()
        .into_iter()
        .map(|b| Entry {
            name: b.chord.clone(),
            detail: b.action.clone(),
            params: Vec::new(),
        })
        .collect();
    entries.sort_by(|a, b| a.detail.cmp(&b.detail).then(a.name.cmp(&b.name)));
    Section {
        title: "Keys",
        about: "The default bindings. Every one names an action from the \
                lists above, or a per-client view action; Preferences rebinds \
                them without touching this.",
        entries,
    }
}

fn editors_section() -> Section {
    let entries: Vec<Entry> = crate::editors::kinds()
        .into_iter()
        .map(|k| Entry {
            name: k.label.to_string(),
            detail: format!(
                "`{}`{}",
                k.id,
                if k.targets { " — takes a target" } else { "" }
            ),
            params: k
                .hints
                .iter()
                .map(|(what, does)| Param {
                    name: (*what).to_string(),
                    kind: String::new(),
                    required: false,
                    about: (*does).to_string(),
                })
                .collect(),
        })
        .collect();
    Section {
        title: "Editors",
        about: "What an area can show. The hints are what each answers to, \
                and they are what the status bar shows while it has focus.",
        entries,
    }
}

/// The reference as Markdown. `docs/reference.md` is this, byte for byte.
pub(crate) fn markdown(sections: &[Section]) -> String {
    let mut out = String::new();
    out.push_str("# powderman reference\n\n");
    out.push_str(
        "Generated from the registries themselves — the command bus, the MCP\n\
         router, the keymap and the editor registry. Nothing here is written\n\
         prose, so nothing here can be out of date.\n\n\
         Regenerate with `UPDATE_DOCS=1 cargo test -p powderman`; a stale file\n\
         fails the test suite.\n",
    );
    for section in sections {
        out.push_str(&format!("\n## {}\n\n{}\n\n", section.title, section.about));
        for entry in &section.entries {
            out.push_str(&format!("### `{}`\n\n", entry.name));
            if !entry.detail.is_empty() {
                out.push_str(&format!("{}\n\n", entry.detail));
            }
            if entry.params.is_empty() {
                continue;
            }
            let typed = entry.params.iter().any(|p| !p.kind.is_empty());
            if typed {
                out.push_str("| parameter | type | required | what it is |\n");
                out.push_str("|---|---|---|---|\n");
                for p in &entry.params {
                    out.push_str(&format!(
                        "| `{}` | {} | {} | {} |\n",
                        cell(&p.name),
                        cell(&p.kind),
                        if p.required { "yes" } else { "no" },
                        cell(&p.about),
                    ));
                }
            } else {
                for p in &entry.params {
                    out.push_str(&format!("- **{}** — {}\n", p.name, p.about));
                }
            }
            out.push('\n');
        }
    }
    out
}

/// A value going into a Markdown table cell. A type like `row | col` is the
/// most useful thing in the column and also the character that ends it, so
/// the pipes are escaped — without this the enum that was worth following
/// silently splits the row into six columns.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// Where the generated file lives, relative to the workspace root. Only the
/// drift test needs it — the CLI prints to stdout and leaves the writing to
/// whoever redirected it.
#[cfg(test)]
const PATH: &str = "docs/reference.md";

#[cfg(test)]
mod tests {
    use super::*;

    fn on_disk() -> std::path::PathBuf {
        // CARGO_MANIFEST_DIR is powderman/; the docs are one level up.
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the workspace root")
            .join(PATH)
    }

    /// The file is the rendering, held to the code by this. Set UPDATE_DOCS=1
    /// to rewrite it — the same shape as the shim drift check, and for the
    /// same reason: a generated artefact that only CI can produce is one
    /// nobody regenerates.
    #[test]
    fn the_reference_on_disk_matches_the_registries() {
        let want = markdown(&reference());
        let path = on_disk();
        if std::env::var("UPDATE_DOCS").is_ok() {
            std::fs::write(&path, &want).expect("write the reference");
            return;
        }
        let have = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            have, want,
            "\n{} is stale. Regenerate it:\n\n    UPDATE_DOCS=1 cargo test -p powderman\n",
            PATH
        );
    }

    /// The point of generating it: every command is in it, with the words the
    /// registry uses. A section that silently came back empty would still
    /// match a file regenerated from the same emptiness, so the drift test
    /// above cannot catch this one.
    #[test]
    fn every_registry_is_actually_represented() {
        let sections = reference();
        for section in &sections {
            assert!(
                !section.entries.is_empty(),
                "the {} section is empty",
                section.title
            );
        }
        let doc = markdown(&sections);
        for c in crate::workflows::commands().iter() {
            assert!(doc.contains(c.name), "{} is not in the reference", c.name);
            assert!(doc.contains(c.description), "{}'s description is", c.name);
        }
        for k in crate::editors::kinds() {
            assert!(doc.contains(k.label), "the {} editor is missing", k.label);
        }
    }

    /// The reason this beats hand-written docs: the parameter descriptions
    /// are the doc comments on the argument structs, so the reference carries
    /// prose someone already wrote beside the code.
    #[test]
    fn tool_parameters_arrive_with_their_descriptions() {
        if std::env::var("DUMP_SCHEMA").is_ok() {
            for t in crate::mcp::tools() {
                if t.name == "split" {
                    println!("{}", serde_json::to_string_pretty(&t.input_schema).unwrap());
                }
            }
        }
        let tools = reference()
            .into_iter()
            .find(|s| s.title == "MCP tools")
            .expect("the tools section");
        let split = tools
            .entries
            .iter()
            .find(|e| e.name == "split")
            .expect("the split tool");
        let id = split
            .params
            .iter()
            .find(|p| p.name == "id")
            .expect("split takes an area id");
        assert!(id.required, "the area id is not optional");
        assert!(
            !id.about.is_empty(),
            "the schema lost the doc comment on SplitArgs::id"
        );
        assert!(
            split.params.iter().any(|p| !p.required),
            "frac is optional and should say so"
        );
        // The one parameter with a fixed set of values arrives as a $ref into
        // the schema's own definitions. Not following it reports "any" for
        // exactly the parameter a reader most needs the values of.
        let dir = split
            .params
            .iter()
            .find(|p| p.name == "dir")
            .expect("split takes a direction");
        assert_eq!(dir.kind, "row | col", "the enum was not followed");
        // ...and a pipe is the column separator, so the same win breaks the
        // table it lands in unless it is escaped.
        let table = markdown(&reference());
        assert!(
            table.contains(r"| `dir` | row \| col | yes |"),
            "the enum's pipes are splitting the table row"
        );
    }
}
