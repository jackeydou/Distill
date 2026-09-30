//! `distill mcp`: the stdio MCP server the plugin starts, one per agent session.
//!
//! Each tool call opens config, vault and index afresh (a few milliseconds), so a server
//! that lives for a whole session still sees other devices' notes and config changes.
//! Validation failures come back as tool errors with the message the agent needs to retry;
//! only transport problems are protocol errors.

mod app;

use std::path::Path;

use distill_core::Distill;
use distill_core::index::NoteView;
use distill_core::ops::SaveRequest;
use distill_core::sources::{SourceEnv, reopen, verify};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt, schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

const INSTRUCTIONS: &str = "Distill keeps notes of what the user learned in agent sessions and \
notices when they ask the same question again. Read the distill skill for when to call these \
tools and how to fill them in.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecallParams {
    /// The user's question, in their own words.
    pub question: String,
    /// How many topics to return. Default 5.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    /// Keywords. Leave empty to list recent notes.
    #[serde(default)]
    pub query: Option<String>,
    /// Only notes with this tag.
    #[serde(default)]
    pub tag: Option<String>,
    /// Maximum notes to return. Default 20.
    #[serde(default)]
    pub limit: Option<usize>,
}

pub struct DistillServer {
    ui_enabled: bool,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl DistillServer {
    pub fn new(ui_enabled: bool) -> Self {
        let mut tool_router = Self::tool_router();
        if ui_enabled {
            let mut ui_router = Self::ui_router();
            let entry = ui_router
                .map
                .get_mut("distill_open_ui")
                .expect("UI entrypoint exists");
            entry.attr.title = Some(app::title().into());
            entry.attr.icons = Some(vec![app::icon()]);
            tool_router += ui_router;
        }
        Self {
            ui_enabled,
            tool_router,
        }
    }

    #[tool(
        name = "distill_recall",
        description = "Find past notes on questions like this one. Returns matching topics (each with how many times it was asked, its notes and the user's annotations) and every tag in use. Call it before answering a conceptual question, and before distill_save."
    )]
    async fn recall(
        &self,
        Parameters(p): Parameters<RecallParams>,
    ) -> Result<CallToolResult, McpError> {
        respond(|| {
            let d = Distill::open()?;
            let result = d.recall(&p.question, p.limit.unwrap_or(5))?;
            let warnings = if result.topics.is_empty() {
                Vec::new()
            } else {
                start_ui(&d)
            };
            let topics: Vec<Value> = result
                .topics
                .iter()
                .map(|t| {
                    json!({
                        "topic_id": t.topic_id,
                        "label": t.label,
                        "ask_count": t.ask_count,
                        "notes": t.notes.iter().map(|n| note_json(&d, n)).collect::<Vec<_>>(),
                    })
                })
                .collect();
            Ok(json!({
                "topics": topics,
                "tags": result.tags,
                "semantic": result.semantic,
                "warnings": warnings,
            }))
        })
    }

    #[tool(
        name = "distill_save",
        description = "Save a distilled note. Use a topic id from distill_recall when this is the same question asked before, otherwise \"new\". tags must already exist; put a tag in new_tags only when no existing tag fits. Copy source from the distill-source line."
    )]
    async fn save(
        &self,
        Parameters(req): Parameters<SaveRequest>,
    ) -> Result<CallToolResult, McpError> {
        respond(|| {
            let warnings = verify(&req.source, &SourceEnv::from_process())?;
            for w in &warnings {
                eprintln!("distill: {w}");
            }
            let mut warnings = warnings;
            let mut d = Distill::open()?;
            let saved = d.save(req)?;
            warnings.extend(start_ui(&d));
            let mut out = serde_json::to_value(&saved).map_err(json_error)?;
            out["file"] = json!(saved.path);
            out["warnings"] = json!(warnings);
            Ok(out)
        })
    }

    #[tool(
        name = "distill_search",
        description = "Search notes by keyword and/or tag. With no query, lists recent notes."
    )]
    async fn search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        respond(|| {
            let d = Distill::open()?;
            let hits = d.index.search(
                p.query.as_deref().unwrap_or_default(),
                p.tag.as_deref(),
                p.limit.unwrap_or(20),
            )?;
            let hits = hits
                .iter()
                .map(|h| {
                    let mut v = serde_json::to_value(h).map_err(json_error)?;
                    v["file"] = json!(abs(&d, &h.path));
                    v["url"] = json!(d.config.note_url(&h.id));
                    Ok(v)
                })
                .collect::<distill_core::Result<Vec<Value>>>()?;
            Ok(json!({ "notes": hits }))
        })
    }

    #[tool(
        name = "distill_stats",
        description = "Counts for analysis: notes, topics, topics asked more than once, most-asked topics (and whether the user annotated them), tags, notes per ISO week and per project."
    )]
    async fn stats(&self) -> Result<CallToolResult, McpError> {
        respond(|| {
            let stats = Distill::open()?.index.stats()?;
            serde_json::to_value(stats).map_err(json_error)
        })
    }
}

impl Default for DistillServer {
    fn default() -> Self {
        Self::new(false)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DistillServer {
    fn get_info(&self) -> ServerConfig {
        let capabilities = if self.ui_enabled {
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build()
        } else {
            ServerCapabilities::builder().enable_tools().build()
        };
        ServerConfig::new(capabilities)
            .with_server_info(Implementation::new("distill", env!("CARGO_PKG_VERSION"))
                .with_title(app::title()).with_icons(vec![app::icon()]))
            .with_instructions(if std::env::var_os("DISTILL_DEV").is_some() {
                "Distill dev uses an isolated local vault. Read the distill-dev skill and use only this server for dev context."
            } else { INSTRUCTIONS })
    }
    async fn list_resources(
        &self,
        _: Option<rmcp::model::PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListResourcesResult, McpError> {
        Ok(rmcp::model::ListResourcesResult::with_all_items(
            if self.ui_enabled {
                vec![app::resource()]
            } else {
                vec![]
            },
        ))
    }

    async fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParams,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ReadResourceResponse, McpError> {
        app::read_resource(&request.uri, self.ui_enabled).map(Into::into)
    }
}

pub async fn serve(ui_enabled: bool) -> anyhow::Result<()> {
    let service = DistillServer::new(ui_enabled)
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}

/// Starts the web UI so the links in a tool result open. A failure does not fail the tool
/// call; it comes back as a warning the agent can pass on.
fn start_ui(d: &Distill) -> Vec<String> {
    if !d.config.ui.autostart {
        return Vec::new();
    }
    match crate::ui::ensure_running(&d.config, &d.dirs) {
        Ok(_) => Vec::new(),
        Err(e) => {
            let warning = format!("note links may not open: {e:#}");
            eprintln!("distill: {warning}");
            vec![warning]
        }
    }
}

fn note_json(d: &Distill, n: &NoteView) -> Value {
    json!({
        "id": n.id,
        "url": d.config.note_url(&n.id),
        "title": n.title,
        "question": n.question,
        "conclusion": n.conclusion,
        "tags": n.tags,
        "created": n.created,
        "annotations": n.annotations,
        "file": abs(d, &n.path),
        "reopen": reopen(&n.source),
    })
}

fn abs(d: &Distill, rel: &str) -> String {
    d.vault.root().join(Path::new(rel)).display().to_string()
}

fn json_error(e: serde_json::Error) -> distill_core::Error {
    distill_core::Error::InvalidNote {
        reason: format!("could not encode the result: {e}"),
    }
}

/// Success becomes JSON content; a Distill error becomes a tool error carrying the whole
/// error chain, which the agent reads to fix its call.
fn respond(f: impl FnOnce() -> distill_core::Result<Value>) -> Result<CallToolResult, McpError> {
    match f() {
        Ok(value) => Ok(CallToolResult::success(vec![ContentBlock::json(value)?])),
        Err(err) => {
            let mut text = err.to_string();
            let mut source = std::error::Error::source(&err);
            while let Some(cause) = source {
                text.push_str(&format!(": {cause}"));
                source = cause.source();
            }
            Ok(CallToolResult::error(vec![ContentBlock::text(text)]))
        }
    }
}
