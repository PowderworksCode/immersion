//! The command bus: one typed write path for every layout mutation.
//!
//! Immersion's central rule, carried over from the React original: anything a
//! human can do to the layout, an agent must be able to do too. That holds
//! only if there is exactly one way to mutate the layout — a registry of named
//! commands, each taking JSON params. The header buttons, the gesture shim,
//! the keymap, and (later) an agent endpoint all name a command; none reaches
//! into the tree directly.
//!
//! A command is `fn(&mut Workspaces, &Value) -> Result`. It is a plain
//! function of the workspace value, so it is testable without a browser and
//! composes into a server that has no UI — the same property the original
//! prized in its dockview-api commands, and the reason undo is a snapshot of
//! this value rather than a diff.
//!
//! The library ships the built-ins that operate on its own types (split, join,
//! resize, switch editor, the workspace ops). A host adds domain commands —
//! "open this run in a new area" — with [`Commands::with`], and they route
//! through the same [`Commands::run`], so an agent driving the host reaches
//! them identically.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use serde_json::Value;

use crate::area::{Area, Dir, Region};
use crate::workspace::Workspaces;

/// One named operation on the workbench.
#[derive(Clone)]
pub struct Command {
    pub name: &'static str,
    /// Human- and agent-facing; becomes a palette entry and, later, a tool
    /// description.
    pub description: &'static str,
    /// True for commands that only read or that a palette should de-emphasize;
    /// today it marks the ones undo should not record (pure navigation).
    pub navigational: bool,
    /// Whether this could run against the workbench as it is — Blender's
    /// `poll()`. A menu greys out what fails it, and `run` refuses it, so a
    /// control that cannot do anything says so instead of erroring after the
    /// click.
    ///
    /// The params may be `Null`: a surface deciding whether to *offer* a
    /// command asks before it has any. Answer for the context in that case,
    /// and use the params to be more exact when they are there.
    pub poll: fn(&Workspaces, &Value) -> bool,
    pub run: fn(&mut Workspaces, &Value) -> Result<()>,
}

/// A command that is always offered. Most are — splitting an area, renaming a
/// workspace: there is no state in which they make no sense.
pub fn always(_: &Workspaces, _: &Value) -> bool {
    true
}

/// Needs something to act across: a second area. Join, swap and the seam
/// between two areas all vanish from a menu when there is only one.
pub fn many_areas(ws: &Workspaces, _: &Value) -> bool {
    ws.current().layout.root.leaves().len() > 1
}

/// Needs a second workspace. The whole `workspace.*` family except add,
/// rename and duplicate.
pub fn many_workspaces(ws: &Workspaces, _: &Value) -> bool {
    ws.tabs.len() > 1
}

/// The registry. Ordered so a palette lists commands predictably.
#[derive(Clone, Default)]
pub struct Commands(BTreeMap<&'static str, Command>);

impl Commands {
    /// The commands that operate on the library's own types.
    pub fn builtin() -> Self {
        let mut c = Commands::default();
        for cmd in BUILTINS {
            c.0.insert(cmd.name, cmd.clone());
        }
        c
    }

    /// Add a host command. Chainable: `Commands::builtin().with(open_run)`.
    pub fn with(mut self, cmd: Command) -> Self {
        self.0.insert(cmd.name, cmd);
        self
    }

    pub fn get(&self, name: &str) -> Option<&Command> {
        self.0.get(name)
    }

    /// Run a command against the workbench. The one write path — persistence
    /// and undo wrap *this*, they do not bypass it.
    pub fn run(&self, ws: &mut Workspaces, name: &str, params: &Value) -> Result<()> {
        let cmd = self
            .0
            .get(name)
            .ok_or_else(|| anyhow!("unknown command {name}"))?;
        // Checked here rather than only in the chrome, because the chrome is
        // not the only caller: an agent reaches the same registry, and a
        // command that cannot apply should say so once, in one place, rather
        // than failing differently depending on who asked.
        if !(cmd.poll)(ws, params) {
            return Err(anyhow!("{name} does not apply to the workbench as it is"));
        }
        (cmd.run)(ws, params)
    }

    /// Whether a command could run right now. `params` may be `Null` when the
    /// question is "should this be offered at all".
    pub fn can(&self, ws: &Workspaces, name: &str, params: &Value) -> bool {
        self.0.get(name).is_some_and(|c| (c.poll)(ws, params))
    }

    /// The commands that apply to the workbench as it is. What a palette or a
    /// menu should be built from — Blender lists operators the same way, and
    /// it is why its menus shrink rather than filling with things that error.
    pub fn available<'a>(&'a self, ws: &'a Workspaces) -> impl Iterator<Item = &'a Command> {
        self.0.values().filter(move |c| (c.poll)(ws, &Value::Null))
    }

    /// Whether running `name` should be recorded for undo. Unknown or
    /// navigational commands are not.
    pub fn records_undo(&self, name: &str) -> bool {
        self.0.get(name).is_some_and(|c| !c.navigational)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Command> {
        self.0.values()
    }
}

// --- param helpers ---------------------------------------------------------
// Commands validate their own params, like the original's paramsSchema. A
// missing or wrong-typed field is an error, not a silent no-op — the bus is
// the boundary where bad input stops.

fn u64_field(p: &Value, key: &str) -> Result<u64> {
    p.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("param {key} must be an integer"))
}

fn f32_field(p: &Value, key: &str) -> Result<f32> {
    p.get(key)
        .and_then(Value::as_f64)
        .map(|v| v as f32)
        .ok_or_else(|| anyhow!("param {key} must be a number"))
}

fn str_field<'a>(p: &'a Value, key: &str) -> Result<&'a str> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("param {key} must be a string"))
}

fn region_field(p: &Value, key: &str) -> Result<Region> {
    serde_json::from_value(p.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|_| anyhow!("param {key} must be a region name"))
}

fn dir_field(p: &Value, key: &str) -> Result<Dir> {
    match str_field(p, key)? {
        "row" => Ok(Dir::Row),
        "col" => Ok(Dir::Col),
        other => Err(anyhow!(
            "param {key} must be \"row\" or \"col\", got {other:?}"
        )),
    }
}

// --- the built-ins ---------------------------------------------------------

/// A mutation that found nothing to mutate is an error, not a shrug: "split
/// area 9999" answering Ok while the tree is unchanged is how a UI reports a
/// success banner over a no-op. Every builtin routes its bool/Option result
/// through here.
fn took(applied: bool, what: &str) -> Result<()> {
    if applied {
        Ok(())
    } else {
        Err(anyhow!("{what}: no such area"))
    }
}

const BUILTINS: &[Command] = &[
    Command {
        name: "split",
        description: "Split an area in two",
        navigational: false,
        poll: always,
        run: |ws, p| {
            let id = u64_field(p, "id")?;
            let applied = ws
                .current_layout_mut()
                .split(
                    id,
                    dir_field(p, "dir")?,
                    f32_field(p, "frac").unwrap_or(0.5),
                )
                .is_some();
            took(applied, &format!("split {id}"))
        },
    },
    Command {
        name: "join",
        description: "Close an area; its sibling takes the space",
        navigational: false,
        poll: many_areas,
        run: |ws, p| {
            let id = u64_field(p, "id")?;
            let applied = ws.current_layout_mut().join(id);
            took(applied, &format!("join {id}"))
        },
    },
    Command {
        name: "join_into",
        description: "Merge one area into a sibling",
        navigational: false,
        poll: many_areas,
        run: |ws, p| {
            let (a, b) = (u64_field(p, "survivor")?, u64_field(p, "victim")?);
            let applied = ws.current_layout_mut().join_into(a, b);
            took(applied, &format!("join {b} into {a}"))
        },
    },
    Command {
        name: "ratio",
        description: "Move a seam between two areas",
        navigational: false,
        poll: many_areas,
        run: |ws, p| {
            let id = u64_field(p, "id")?;
            let applied = ws.current_layout_mut().set_seam(
                id,
                u64_field(p, "index").unwrap_or(0) as usize,
                f32_field(p, "ratio")?,
            );
            took(applied, &format!("ratio {id}"))
        },
    },
    Command {
        name: "set_region_width",
        description: "Resize an area's toolbar or sidebar",
        navigational: true,
        poll: always,
        run: |ws, p| {
            let id = u64_field(p, "id")?;
            let applied = ws.current_layout_mut().set_region_width(
                id,
                region_field(p, "region")?,
                u64_field(p, "w")? as u16,
            );
            took(applied, &format!("set_region_width {id}"))
        },
    },
    Command {
        name: "toggle_region",
        description: "Show or hide an area's toolbar or sidebar",
        // A view toggle, persisted with the layout but not something you undo.
        navigational: true,
        poll: always,
        run: |ws, p| {
            let id = u64_field(p, "id")?;
            let applied = ws
                .current_layout_mut()
                .toggle_region(id, region_field(p, "region")?);
            took(applied, &format!("toggle_region {id}"))
        },
    },
    Command {
        name: "duplicate_area",
        description: "Split an area and show the same editor in the new half",
        navigational: false,
        poll: always,
        run: |ws, p| {
            let id = u64_field(p, "id")?;
            let l = ws.current_layout_mut();
            let src = match l.root.find(id) {
                Some(Area::Leaf { editor, arg, .. }) => Some((editor.clone(), arg.clone())),
                _ => None,
            };
            let (Some((editor, arg)), Some(new)) = (src, l.split(id, Dir::Row, 0.5)) else {
                return took(false, &format!("duplicate_area {id}"));
            };
            match arg {
                Some(a) => {
                    l.set_editor_arg(new, &editor, &a);
                }
                None => {
                    l.set_editor(new, &editor);
                }
            }
            Ok(())
        },
    },
    Command {
        name: "swap",
        description: "Swap what two areas show",
        navigational: false,
        poll: many_areas,
        run: |ws, p| {
            let (a, b) = (u64_field(p, "a")?, u64_field(p, "b")?);
            let applied = ws.current_layout_mut().swap_editors(a, b);
            took(applied, &format!("swap {a} {b}"))
        },
    },
    Command {
        name: "set_editor",
        description: "Change what an area shows",
        navigational: false,
        poll: always,
        run: |ws, p| {
            let id = u64_field(p, "id")?;
            let applied = ws
                .current_layout_mut()
                .set_editor(id, str_field(p, "editor")?);
            took(applied, &format!("set_editor {id}"))
        },
    },
    Command {
        name: "set_target",
        description: "Point an area at something without changing its editor",
        navigational: false,
        poll: always,
        run: |ws, p| {
            let id = u64_field(p, "id")?;
            // The empty string clears the target, so "show everything again"
            // needs no second command.
            let target = str_field(p, "target").unwrap_or("");
            let applied = ws.current_layout_mut().set_target(id, target);
            took(applied, &format!("set_target {id}"))
        },
    },
    Command {
        name: "open_editor",
        description: "Point an area at a specific thing (editor + argument)",
        navigational: false,
        poll: always,
        run: |ws, p| {
            let id = u64_field(p, "id")?;
            let applied = ws.current_layout_mut().set_editor_arg(
                id,
                str_field(p, "editor")?,
                str_field(p, "arg")?,
            );
            took(applied, &format!("open_editor {id}"))
        },
    },
    Command {
        name: "workspace.switch",
        description: "Show a workspace by index",
        navigational: true,
        poll: many_workspaces,
        run: |ws, p| {
            ws.switch(u64_field(p, "index")? as usize);
            Ok(())
        },
    },
    Command {
        name: "workspace.cycle",
        description: "Show the next or previous workspace",
        navigational: true,
        poll: many_workspaces,
        run: |ws, p| {
            ws.cycle(p.get("delta").and_then(Value::as_i64).unwrap_or(1) as i32);
            Ok(())
        },
    },
    Command {
        name: "workspace.add",
        description: "Add a workspace from a layout",
        navigational: false,
        poll: always,
        run: |ws, p| {
            let name = str_field(p, "name")?;
            let layout = p
                .get("layout")
                .ok_or_else(|| anyhow!("param layout is required"))?;
            let layout = serde_json::from_value(layout.clone())?;
            ws.add(name, layout);
            Ok(())
        },
    },
    Command {
        name: "workspace.rename",
        description: "Rename a workspace",
        navigational: false,
        poll: always,
        run: |ws, p| {
            ws.rename(u64_field(p, "index")? as usize, str_field(p, "name")?);
            Ok(())
        },
    },
    Command {
        name: "workspace.duplicate",
        description: "Duplicate a workspace",
        navigational: false,
        poll: always,
        run: |ws, _| {
            let cur = ws.current();
            let name = format!("{} copy", cur.name);
            let layout = cur.layout.clone();
            ws.add(&name, layout);
            Ok(())
        },
    },
    Command {
        name: "workspace.move",
        description: "Move a workspace tab to another position",
        navigational: false,
        poll: many_workspaces,
        run: |ws, p| {
            let from = u64_field(p, "from")? as usize;
            let to = u64_field(p, "to")? as usize;
            let applied = ws.move_tab(from, to);
            took(applied, &format!("workspace.move {from}->{to}"))
        },
    },
    Command {
        name: "workspace.close",
        description: "Close a workspace",
        navigational: false,
        poll: many_workspaces,
        run: |ws, p| {
            ws.close(u64_field(p, "index")? as usize);
            Ok(())
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::area::Layout;
    use serde_json::json;

    fn ws() -> Workspaces {
        Workspaces::new("main", Layout::single("machine"))
    }

    #[test]
    fn a_mutation_that_finds_nothing_is_an_error() {
        // "split area 9999" answering Ok over an unchanged tree is how a UI
        // shows a success banner over a no-op. Every targeted builtin must
        // refuse a missing id.
        let cmds = Commands::builtin();
        for (name, params) in [
            ("split", json!({ "id": 9999, "dir": "row" })),
            ("join", json!({ "id": 9999 })),
            ("join_into", json!({ "survivor": 9999, "victim": 9998 })),
            ("ratio", json!({ "id": 9999, "ratio": 0.5 })),
            (
                "set_region_width",
                json!({ "id": 9999, "region": "toolbar", "w": 200 }),
            ),
            ("toggle_region", json!({ "id": 9999, "region": "toolbar" })),
            ("duplicate_area", json!({ "id": 9999 })),
            ("swap", json!({ "a": 9999, "b": 9998 })),
            ("set_editor", json!({ "id": 9999, "editor": "runs" })),
            (
                "open_editor",
                json!({ "id": 9999, "editor": "run", "arg": "x" }),
            ),
        ] {
            let mut w = ws();
            let err = cmds.run(&mut w, name, &params);
            assert!(err.is_err(), "{name} on a missing area should be an error");
        }
    }

    #[test]
    fn split_and_set_editor_run_through_the_bus() {
        let cmds = Commands::builtin();
        let mut w = ws();
        cmds.run(
            &mut w,
            "split",
            &json!({ "id": 1, "dir": "row", "frac": 0.6 }),
        )
        .unwrap();
        let leaves = w.current().layout.root.leaves();
        assert_eq!(leaves.len(), 2);
        cmds.run(
            &mut w,
            "set_editor",
            &json!({ "id": leaves[1], "editor": "runs" }),
        )
        .unwrap();
        // The command reshaped the real tree.
        assert_eq!(w.current().layout.root.leaves().len(), 2);
    }

    #[test]
    fn a_bad_param_is_an_error_not_a_silent_noop() {
        let cmds = Commands::builtin();
        let mut w = ws();
        assert!(
            cmds.run(
                &mut w,
                "split",
                &json!({ "id": "not-a-number", "dir": "row" })
            )
            .is_err()
        );
        assert!(
            cmds.run(&mut w, "split", &json!({ "id": 1, "dir": "diagonal" }))
                .is_err()
        );
        assert!(cmds.run(&mut w, "nonexistent", &json!({})).is_err());
    }

    #[test]
    fn navigation_does_not_record_undo_but_edits_do() {
        let cmds = Commands::builtin();
        assert!(!cmds.records_undo("workspace.switch"));
        assert!(!cmds.records_undo("workspace.cycle"));
        assert!(cmds.records_undo("split"));
        assert!(cmds.records_undo("join"));
    }

    #[test]
    fn a_host_command_composes_with_the_builtins() {
        fn open_run(ws: &mut Workspaces, p: &Value) -> Result<()> {
            let id = u64_field(p, "area")?;
            let run = str_field(p, "run")?;
            let l = ws.current_layout_mut();
            if let Some(new) = l.split(id, Dir::Row, 0.5) {
                l.set_editor_arg(new, "run", run);
            }
            Ok(())
        }
        let cmds = Commands::builtin().with(Command {
            name: "open_run",
            description: "Open a run in a new area",
            navigational: false,
            poll: always,
            run: open_run,
        });
        let mut w = ws();
        cmds.run(&mut w, "open_run", &json!({ "area": 1, "run": "abc123" }))
            .unwrap();
        assert_eq!(w.current().layout.root.leaves().len(), 2);
    }
}

#[cfg(test)]
mod poll_tests {
    use super::*;
    use crate::area::{Dir, Layout};

    fn lone() -> Workspaces {
        Workspaces::new("one", Layout::single("runs"))
    }

    fn two_areas() -> Workspaces {
        let mut w = lone();
        w.current_layout_mut().split(1, Dir::Row, 0.5);
        w
    }

    /// The row everybody meets: the last area has nothing to join into. It
    /// has always been offered and has always failed on the click.
    #[test]
    fn what_needs_a_second_area_is_not_offered_with_one() {
        let cmds = Commands::builtin();
        let one = lone();
        for name in ["join", "join_into", "ratio", "swap"] {
            assert!(!cmds.can(&one, name, &Value::Null), "{name} was offered");
        }
        // And splitting always is: there is no workbench where it makes no
        // sense, which is exactly why it needs no poll of its own.
        assert!(cmds.can(&one, "split", &Value::Null));

        let two = two_areas();
        for name in ["join", "join_into", "ratio", "swap", "split"] {
            assert!(cmds.can(&two, name, &Value::Null), "{name} went missing");
        }
    }

    /// The whole workspace family, except the three that make sense alone.
    #[test]
    fn what_needs_a_second_workspace_is_not_offered_with_one() {
        let cmds = Commands::builtin();
        let mut w = lone();
        for name in [
            "workspace.close",
            "workspace.cycle",
            "workspace.switch",
            "workspace.move",
        ] {
            assert!(!cmds.can(&w, name, &Value::Null), "{name} was offered");
        }
        for name in ["workspace.add", "workspace.rename", "workspace.duplicate"] {
            assert!(cmds.can(&w, name, &Value::Null), "{name} went missing");
        }
        w.add("second", Layout::single("runs"));
        for name in [
            "workspace.close",
            "workspace.cycle",
            "workspace.switch",
            "workspace.move",
        ] {
            assert!(cmds.can(&w, name, &Value::Null), "{name} went missing");
        }
    }

    /// A poll the chrome respects and `run` does not is a poll an agent walks
    /// straight through. It is checked in the one place both go through.
    #[test]
    fn run_refuses_what_poll_refuses() {
        let cmds = Commands::builtin();
        let mut w = lone();
        let err = cmds
            .run(&mut w, "join", &serde_json::json!({ "id": 1 }))
            .expect_err("joining the only area is not a thing");
        assert!(
            err.to_string().contains("does not apply"),
            "the refusal should say why: {err}"
        );
        // And the workbench is untouched — a refused command is not a
        // half-applied one.
        assert_eq!(w.current().layout.root.leaves().len(), 1);
    }

    /// `available` is what a palette or a menu is built from, so it has to
    /// shrink with the workbench rather than listing everything always.
    #[test]
    fn available_shrinks_with_the_workbench() {
        let cmds = Commands::builtin();
        let one = lone();
        let mut many = two_areas();
        many.add("second", Layout::single("runs"));
        let count = |w: &Workspaces| cmds.available(w).count();
        assert!(
            count(&one) < count(&many),
            "one area and one workspace should offer less: {} vs {}",
            count(&one),
            count(&many)
        );
        assert!(!cmds.available(&one).any(|c| c.name == "join"));
    }
}
