//! The workbench as an MCP server.
//!
//! Immersion's thesis, in the shape an agent actually wants: not a REST call it
//! must be taught, but tools it discovers. Every layout command is a typed MCP
//! tool — `split { id, dir, frac }`, `open_run { area, run }`, `undo` — with a
//! JSON schema derived from its argument struct, so a coding agent connected to
//! `/mcp` sees them in `tools/list` and calls them the way it calls any tool.
//!
//! The tools reach the same [`crate::daemon::dispatch_checked`] a keypress
//! does, so an MCP call and a click land on one write path with one undo
//! history. The server is mounted on the daemon's own axum router (see
//! [`service`]) and shares its process and state; there is no second copy of
//! the workbench behind the protocol.

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities,
        ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::{Value, json};

// `Dir` and `Region` are the library's — a second definition here would be one
// more thing to keep in step, which is exactly what the generated wire types
// removed elsewhere.
use immersion::Dir;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SplitArgs {
    /// The area (leaf) id to split. Read ids from `get_state`.
    pub id: u64,
    /// Split direction: `row` puts the new area beside, `col` below.
    pub dir: Dir,
    /// The first child's fraction of the space, 0..1. Defaults to 0.5.
    pub frac: Option<f64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SelectArgs {
    /// What sort of thing is being selected: `file`, `folder`, `run`,
    /// `chart` or `data`. Areas showing that sort follow it.
    pub kind: String,
    /// The thing itself — a path for `file`, a run id for `run`, a chart
    /// pointer for `chart`.
    pub value: String,
    /// `replace` (the default) selects only this; `extend` adds it to the
    /// selection and makes it active; `toggle` removes it if it was already
    /// selected. Many things can be selected; the last one is active, and
    /// that is what unpinned areas follow.
    pub mode: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PinArgs {
    /// The area id.
    pub id: u64,
    /// True freezes it on what it is showing; false lets it follow again.
    pub pinned: bool,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DepthArgs {
    /// How many undo steps to take. Past the end of the stack unwinds as far
    /// as there is history rather than failing.
    pub depth: usize,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct IdArgs {
    /// The area id.
    pub id: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct JoinIntoArgs {
    /// The area that stays and takes the space.
    pub survivor: u64,
    /// The area that is closed.
    pub victim: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RatioArgs {
    /// The split (or either of its children) whose seam moves.
    pub id: u64,
    /// The first child's fraction of the space, 0..1.
    pub ratio: f64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetEditorArgs {
    /// The area to repoint.
    pub id: u64,
    /// The editor id to show (e.g. `runs`, `fleet`, `settings`).
    pub editor: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct OpenEditorArgs {
    /// The area to repoint.
    pub id: u64,
    /// The editor id to show.
    pub editor: String,
    /// The editor's argument (e.g. a run id for the `run` editor).
    pub arg: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct OpenRunArgs {
    /// The area to split; the run opens in the new half beside it.
    pub area: u64,
    /// The run id to open.
    pub run: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct IndexArgs {
    /// The workspace tab index (0-based).
    pub index: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CycleArgs {
    /// +1 for the next workspace, -1 for the previous. Defaults to +1.
    pub delta: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RenameArgs {
    /// The workspace tab index (0-based).
    pub index: u64,
    /// The new name.
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetTargetArgs {
    /// The area to retarget.
    pub id: u64,
    /// What it should look at — a JSON pointer into the workbench documents
    /// (`/settings/favorites`), a path for a file browser, or a run id. The
    /// empty string clears the target.
    pub target: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct MoveArgs {
    /// The tab's current position (0-based).
    pub from: u64,
    /// Where it should end up.
    pub to: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SwapArgs {
    /// One area id.
    pub a: u64,
    /// The other; the two exchange what they show.
    pub b: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RegionArgs {
    /// The area whose region is toggled.
    pub id: u64,
    /// `toolbar`, `sidebar`, `header`, or `header_flip`.
    pub region: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RegionWidthArgs {
    /// The area whose region is resized.
    pub id: u64,
    /// `toolbar` or `sidebar`.
    pub region: String,
    /// The new width in pixels.
    pub w: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WorkspaceAddArgs {
    /// The tab name.
    pub name: String,
    /// The starting layout tree. Omit for a single-area default; the shape is
    /// the `layout` value `get_state` returns.
    #[serde(default)]
    pub layout: serde_json::Value,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetSettingArgs {
    /// JSON pointer into the settings document (e.g. `/theme`, `/ui_scale`,
    /// `/keymap/undo`). `get_state` shows the whole document.
    pub pointer: String,
    /// The value to write at that pointer.
    pub value: serde_json::Value,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FavoriteAddArgs {
    /// The label shown in the Quick Favourites menu.
    pub label: String,
    /// The command the favourite runs when picked.
    pub action: String,
    /// Parameters for that command, if any.
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RunArgs {
    /// The run id, as listed by `get_state`.
    pub id: String,
}

/// The MCP handler. Holds no workbench state of its own — every tool reaches
/// the daemon's shared state through `crate::daemon`, so the handler the
/// session factory mints per connection is a thin, cloneable router.
#[derive(Clone)]
pub struct Workbench {
    // Written by `new()`, read by the `#[tool_handler]`-generated routing; the
    // dead-code pass does not see the macro's use of it.
    #[allow(dead_code)]
    tool_router: ToolRouter<Workbench>,
}

impl Default for Workbench {
    fn default() -> Self {
        Self::new()
    }
}

/// Run a command through the one write path and answer with the new workbench,
/// or the command's error as tool-visible content (so the agent reads why a
/// call was rejected rather than getting an opaque protocol failure).
fn run(name: &str, params: serde_json::Value) -> Result<CallToolResult, McpError> {
    match crate::daemon::dispatch_from("agent", name, params) {
        Ok(ws) => Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&ws).unwrap_or_default(),
        )])),
        Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
            e.to_string(),
        )])),
    }
}

/// MCP tool names are snake_case; command names use dots (`workspace.add`).
/// One spelling rule, applied in one place — the parity test checks with it,
/// and the Info log shows commands with it, so what a person copies off a log
/// row is what an agent actually calls.
pub(crate) fn tool_name(command: &str) -> String {
    command.replace('.', "_")
}

/// Every tool this server offers, as the model an agent receives — name,
/// description and the JSON Schema of its parameters. The router's own
/// accessor is generated private, and the reference is built outside this
/// module, so this is the door.
pub(crate) fn tools() -> Vec<rmcp::model::Tool> {
    Workbench::tool_router().list_all()
}

#[tool_router]
impl Workbench {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Point every unpinned area of a kind at one thing — the file browser's click, as a command. Selecting a file moves both a code viewer and a diff viewer, since both point at a path. Pinned areas are left alone."
    )]
    async fn select(
        &self,
        Parameters(a): Parameters<SelectArgs>,
    ) -> Result<CallToolResult, McpError> {
        run(
            "select",
            json!({ "kind": a.kind, "value": a.value, "mode": a.mode.unwrap_or_else(|| "replace".into()) }),
        )
    }

    #[tool(
        description = "Freeze an area on what it is showing so the selection stops moving it, or let it follow again. Blender's pin."
    )]
    async fn set_pinned(
        &self,
        Parameters(a): Parameters<PinArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("set_pinned", json!({ "id": a.id, "pinned": a.pinned }))
    }

    #[tool(description = "Split an area in two")]
    async fn split(
        &self,
        Parameters(a): Parameters<SplitArgs>,
    ) -> Result<CallToolResult, McpError> {
        run(
            "split",
            json!({ "id": a.id, "dir": a.dir, "frac": a.frac.unwrap_or(0.5) }),
        )
    }

    #[tool(description = "Close an area; its sibling takes the space")]
    async fn join(&self, Parameters(a): Parameters<IdArgs>) -> Result<CallToolResult, McpError> {
        run("join", json!({ "id": a.id }))
    }

    #[tool(description = "Merge one area into a sibling")]
    async fn join_into(
        &self,
        Parameters(a): Parameters<JoinIntoArgs>,
    ) -> Result<CallToolResult, McpError> {
        run(
            "join_into",
            json!({ "survivor": a.survivor, "victim": a.victim }),
        )
    }

    #[tool(description = "Move the seam between two areas")]
    async fn ratio(
        &self,
        Parameters(a): Parameters<RatioArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("ratio", json!({ "id": a.id, "ratio": a.ratio }))
    }

    #[tool(description = "Change what an area shows")]
    async fn set_editor(
        &self,
        Parameters(a): Parameters<SetEditorArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("set_editor", json!({ "id": a.id, "editor": a.editor }))
    }

    #[tool(description = "Point an area at a specific thing (editor + argument)")]
    async fn open_editor(
        &self,
        Parameters(a): Parameters<OpenEditorArgs>,
    ) -> Result<CallToolResult, McpError> {
        run(
            "open_editor",
            json!({ "id": a.id, "editor": a.editor, "arg": a.arg }),
        )
    }

    #[tool(description = "Open a run in a new area beside the list")]
    async fn open_run(
        &self,
        Parameters(a): Parameters<OpenRunArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("open_run", json!({ "area": a.area, "run": a.run }))
    }

    #[tool(description = "Show a workspace by index")]
    async fn workspace_switch(
        &self,
        Parameters(a): Parameters<IndexArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("workspace.switch", json!({ "index": a.index }))
    }

    #[tool(description = "Show the next or previous workspace")]
    async fn workspace_cycle(
        &self,
        Parameters(a): Parameters<CycleArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("workspace.cycle", json!({ "delta": a.delta.unwrap_or(1) }))
    }

    #[tool(description = "Rename a workspace")]
    async fn workspace_rename(
        &self,
        Parameters(a): Parameters<RenameArgs>,
    ) -> Result<CallToolResult, McpError> {
        run(
            "workspace.rename",
            json!({ "index": a.index, "name": a.name }),
        )
    }

    #[tool(description = "Close a workspace")]
    async fn workspace_close(
        &self,
        Parameters(a): Parameters<IndexArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("workspace.close", json!({ "index": a.index }))
    }

    #[tool(
        description = "Point an area at something without changing its editor: a JSON pointer, a path, or a run id. Empty clears it."
    )]
    async fn set_target(
        &self,
        Parameters(a): Parameters<SetTargetArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("set_target", json!({ "id": a.id, "target": a.target }))
    }

    #[tool(description = "Split an area and show the same editor in the new half")]
    async fn duplicate_area(
        &self,
        Parameters(a): Parameters<IdArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("duplicate_area", json!({ "id": a.id }))
    }

    #[tool(description = "Swap what two areas show")]
    async fn swap(&self, Parameters(a): Parameters<SwapArgs>) -> Result<CallToolResult, McpError> {
        run("swap", json!({ "a": a.a, "b": a.b }))
    }

    #[tool(description = "Show or hide an area's toolbar, sidebar, or header")]
    async fn toggle_region(
        &self,
        Parameters(a): Parameters<RegionArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("toggle_region", json!({ "id": a.id, "region": a.region }))
    }

    #[tool(description = "Resize an area's toolbar or sidebar")]
    async fn set_region_width(
        &self,
        Parameters(a): Parameters<RegionWidthArgs>,
    ) -> Result<CallToolResult, McpError> {
        run(
            "set_region_width",
            json!({ "id": a.id, "region": a.region, "w": a.w }),
        )
    }

    #[tool(description = "Add a workspace tab")]
    async fn workspace_add(
        &self,
        Parameters(a): Parameters<WorkspaceAddArgs>,
    ) -> Result<CallToolResult, McpError> {
        run(
            "workspace.add",
            json!({ "name": a.name, "layout": a.layout }),
        )
    }

    #[tool(description = "Move a workspace tab to another position")]
    async fn workspace_move(
        &self,
        Parameters(a): Parameters<MoveArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("workspace.move", json!({ "from": a.from, "to": a.to }))
    }

    #[tool(description = "Duplicate a workspace tab")]
    async fn workspace_duplicate(
        &self,
        Parameters(a): Parameters<IndexArgs>,
    ) -> Result<CallToolResult, McpError> {
        run("workspace.duplicate", json!({ "index": a.index }))
    }

    #[tool(
        description = "Write one value into the settings document by JSON pointer — theme, ui_scale, keymap overrides, favourites. The same operation the Settings editor performs."
    )]
    async fn set_setting(
        &self,
        Parameters(a): Parameters<SetSettingArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&crate::daemon::set_setting("agent", &a.pointer, a.value))
                .unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Add an entry to the Quick Favourites menu (the Q menu). Deduped by label; the list is capped at 12."
    )]
    async fn favorite_add(
        &self,
        Parameters(a): Parameters<FavoriteAddArgs>,
    ) -> Result<CallToolResult, McpError> {
        let entry = serde_json::json!({ "label": a.label, "action": a.action, "params": a.params });
        let (_, added) = crate::daemon::favorite_add("agent", entry);
        Ok(CallToolResult::success(vec![ContentBlock::text(
            if added {
                "added"
            } else {
                "already present (deduped by label)"
            }
            .to_string(),
        )]))
    }

    #[tool(
        description = "Re-run the most recent layout-changing command (Blender's Repeat Last). Navigation and failed commands are skipped."
    )]
    async fn repeat_last(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&crate::daemon::repeat_last("agent")).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Revert the last layout change")]
    async fn undo(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&crate::daemon::undo("agent")).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Step back several layout changes at once, to a point in the undo history. depth is how many steps to take; every one lands on the redo stack. Read the names with get_state's log."
    )]
    async fn undo_to(
        &self,
        Parameters(a): Parameters<DepthArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&crate::daemon::undo_to("agent", a.depth)).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Reapply an undone change")]
    async fn redo(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&crate::daemon::redo("agent")).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Read the live workbench: the workspace tree (area ids, editors, ratios), settings, current box metrics, the fleet, and a run summary. Runs carry a step COUNT only — call get_run for one run's step detail."
    )]
    async fn get_state(&self) -> Result<CallToolResult, McpError> {
        let snap = crate::daemon::snapshot();
        // A run's step logs are inlined in the snapshot and dwarf everything
        // else — a single 300-step run made get_state ~10 MB, too big for an
        // agent to read. Summarize each run to its step count here; the agent
        // drills into one run with get_run. The UI's metric timeseries stay out
        // too — an agent wants the current box numbers, not an hour of samples.
        let runs: Vec<Value> = snap
            .runs
            .iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "workflow": r.workflow,
                    "status": r.status,
                    "note": r.note,
                    "error": r.error,
                    "updated_at": r.updated_at,
                    "steps": r.steps.len(),
                })
            })
            .collect();
        let state = json!({
            "workspaces": crate::daemon::workspaces(),
            "settings": crate::daemon::settings(),
            "herdr": snap.herdr,
            "machine": snap.machine,
            "fleet": snap.fleet,
            "timers": snap.timers,
            "runs": runs,
        });
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&state).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Read one run in full: every step with its result and error. Use after get_state to drill into a run by id."
    )]
    async fn get_run(
        &self,
        Parameters(a): Parameters<RunArgs>,
    ) -> Result<CallToolResult, McpError> {
        match crate::daemon::snapshot()
            .runs
            .into_iter()
            .find(|r| r.id == a.id)
        {
            Some(run) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string(&run).unwrap_or_default(),
            )])),
            None => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "no run with id {}",
                a.id
            ))])),
        }
    }
}

#[tool_handler]
impl ServerHandler for Workbench {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "The powderman workbench. Every tool is a layout command on one write \
                 path with one undo history — the same operations the person at the \
                 browser drives. Read `get_state` for area ids before acting."
                    .to_string(),
            )
    }
}

/// The tower service to mount on the daemon's axum router (at `/mcp`). A fresh
/// [`Workbench`] per session; all sessions share the one daemon state behind
/// `crate::daemon`.
pub fn service() -> StreamableHttpService<Workbench, LocalSessionManager> {
    StreamableHttpService::new(
        || Ok(Workbench::new()),
        LocalSessionManager::default().into(),
        host_config(),
    )
}

/// The transport config, with the DNS-rebinding host allowlist made
/// configurable. The rmcp default allows loopback only — right for an MCP
/// server on localhost, but powderman is reached through a proxy (its own
/// hostname is not localhost), where every request is then Forbidden. So
/// `POWDERMAN_MCP_ALLOWED_HOSTS` (comma-separated) adds hosts to the loopback
/// set; a host with no port matches any port, so `powderworks-dev.exe.xyz`
/// covers `:7778`. The value `*` disables the check entirely (allow any Host)
/// — reasonable only when a front proxy already authenticates, which the
/// exe.dev proxy does.
fn host_config() -> StreamableHttpServerConfig {
    // Stateless JSON mode. The rmcp default is stateful: it issues an
    // mcp-session-id and answers over a held-open SSE stream, and every
    // follow-up request must carry that session id. A reverse proxy breaks both
    // — it strips the non-standard response header (so the client never learns
    // the session) and buffers the stream — and the next request comes back
    // "400 Bad Request: Session ID is required". Stateless mode makes each
    // request a self-contained POST answered with one JSON body: no session to
    // track, no stream to hold, nothing for a proxy to mangle.
    let config = StreamableHttpServerConfig::default()
        .with_json_response(true)
        .with_legacy_session_mode(false);
    match std::env::var("POWDERMAN_MCP_ALLOWED_HOSTS") {
        Ok(v) if v.trim() == "*" => config.disable_allowed_hosts(),
        Ok(v) if !v.trim().is_empty() => {
            let mut hosts = vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "::1".to_string(),
            ];
            hosts.extend(
                v.split(',')
                    .map(str::trim)
                    .filter(|h| !h.is_empty())
                    .map(str::to_string),
            );
            config.with_allowed_hosts(hosts)
        }
        _ => config,
    }
}

#[cfg(test)]
mod parity {
    use super::*;

    /// Commands and host actions an agent is deliberately not given, each with
    /// the reason it is absent. Anything not listed here must have a tool —
    /// adding an entry is a decision someone has to write down, which is the
    /// point.
    const NOT_FOR_AGENTS: &[(&str, &str)] = &[(
        "load_layout",
        "replaces the whole workbench from an uploaded file; an agent that \
         wants a layout builds it with workspace_add",
    )];

    /// The parity invariant, counted rather than remembered: everything a
    /// person can do to server truth, an agent can do too. Six commands were
    /// missing tools when this test was written — duplicate_area, swap,
    /// toggle_region, set_region_width, workspace.add, workspace.duplicate —
    /// which is exactly the drift the test exists to stop.
    #[test]
    fn every_command_and_host_action_has_an_mcp_tool() {
        let router = Workbench::tool_router();
        let mut missing = Vec::new();

        let commands = crate::workflows::commands();
        let host = crate::ui::host_actions();
        for name in commands.iter().map(|c| c.name).chain(host) {
            if NOT_FOR_AGENTS.iter().any(|(n, _)| *n == name) {
                continue;
            }
            if !router.has_route(&super::tool_name(name)) {
                missing.push(name.to_string());
            }
        }
        assert!(
            missing.is_empty(),
            "no MCP tool for: {}\n\
             Add a tool, or add the name to NOT_FOR_AGENTS with the reason.",
            missing.join(", ")
        );
    }

    /// The other direction: an exemption that no longer names anything real is
    /// stale documentation, and a tool for a command that stopped existing is
    /// a dead route an agent can still call.
    #[test]
    fn the_exemption_list_stays_current() {
        let commands = crate::workflows::commands();
        for (name, _why) in NOT_FOR_AGENTS {
            let known = commands.get(name).is_some()
                || !matches!(crate::ui::route(name), crate::ui::Route::Bus);
            assert!(known, "NOT_FOR_AGENTS names {name}, which no longer exists");
        }
    }

    /// Client-view actions are per-client by design — an agent has no
    /// viewport. A tool for one would be a lie about what it does.
    #[test]
    fn client_view_actions_have_no_tools() {
        let router = Workbench::tool_router();
        for a in crate::ui::client_view_actions() {
            assert!(
                !router.has_route(&super::tool_name(a)),
                "{a} is client-view state but has an MCP tool"
            );
        }
    }
}
