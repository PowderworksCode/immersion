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

/// A split runs along a row or a column — the same two the command bus takes,
/// as an enum so the tool schema offers the agent the choice rather than a
/// free-text string it can get wrong.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    Row,
    Col,
}

impl Dir {
    fn as_str(&self) -> &'static str {
        match self {
            Dir::Row => "row",
            Dir::Col => "col",
        }
    }
}

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
    match crate::daemon::dispatch_checked(name, params) {
        Ok(ws) => Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&ws).unwrap_or_default(),
        )])),
        Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
            e.to_string(),
        )])),
    }
}

#[tool_router]
impl Workbench {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Split an area in two")]
    async fn split(
        &self,
        Parameters(a): Parameters<SplitArgs>,
    ) -> Result<CallToolResult, McpError> {
        run(
            "split",
            json!({ "id": a.id, "dir": a.dir.as_str(), "frac": a.frac.unwrap_or(0.5) }),
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

    #[tool(description = "Revert the last layout change")]
    async fn undo(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&crate::daemon::undo()).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Reapply an undone change")]
    async fn redo(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string(&crate::daemon::redo()).unwrap_or_default(),
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
