//! Codex discovers the global/thread entrypoints from tool metadata, then reads this App.

use axum::http::Method;
use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Icon, MetaObject, ReadResourceResult, Resource, ResourceContents,
};
use rmcp::{ErrorData as McpError, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use super::DistillServer;

pub const URI: &str = "ui://distill/library.html";
const MIME: &str = "text/html;profile=mcp-app";

#[derive(rust_embed::Embed)]
#[folder = "$CARGO_MANIFEST_DIR/../../apps/web/dist-mcp/"]
#[allow_missing = true]
struct Assets;

pub fn title() -> &'static str {
    if std::env::var_os("DISTILL_DEV").is_some() {
        "Distill dev"
    } else {
        "Distill"
    }
}

pub fn icon() -> Icon {
    let svg = include_str!("../../../../plugins/distill/codex/assets/distill-sidebar.svg");
    Icon::new(format!(
        "data:image/svg+xml;base64,{}",
        STANDARD.encode(svg)
    ))
    .with_mime_type("image/svg+xml")
    .with_sizes(vec!["any".into()])
}

pub fn resource() -> Resource {
    Resource::new(URI, "distill-library")
        .with_title(title())
        .with_mime_type(MIME)
        .with_description("Distill notes, Review and annotations")
        .with_icons(vec![icon()])
}

pub fn read_resource(uri: &str, enabled: bool) -> Result<ReadResourceResult, McpError> {
    if !enabled || uri != URI {
        return Err(McpError::resource_not_found(
            format!("unknown UI resource {uri}"),
            None,
        ));
    }
    let file = Assets::get("mcp.html").ok_or_else(|| {
        McpError::internal_error(
            "Distill MCP App is missing. Run mise run web:build and rebuild distill.",
            None,
        )
    })?;
    let html = String::from_utf8(file.data.into_owned())
        .map_err(|e| McpError::internal_error(format!("invalid MCP App HTML: {e}"), None))?;
    Ok(ReadResourceResult::new(vec![ResourceContents::text(html, URI)
        .with_mime_type(MIME).with_meta(metadata(json!({
            "ui": {"csp": {"connectDomains": [], "resourceDomains": []}},
            "openai/ui": {"availableDisplayModes": ["fullscreen"], "preferredDisplayMode": "fullscreen"}
        })))]))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ReadParams {
    pub path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum WriteMethod {
    Post,
    Put,
    Delete,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct WriteParams {
    pub method: WriteMethod,
    pub path: String,
    pub body: Option<Value>,
}

#[tool_router(router = ui_router, vis = "pub(super)")]
impl DistillServer {
    #[tool(name = "distill_open_ui", description = "Open the Distill notes library and Review dashboard.",
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = metadata(json!({"ui": {"resourceUri": URI}, "openai/ui": {"entrypoints": [{"type": "global"}, {"type": "thread"}]}})))]
    async fn open_ui(&self) -> Result<CallToolResult, McpError> {
        response(crate::ui::mcp_request(Method::GET, "/session", None).await)
    }

    #[tool(name = "distill_ui_read", description = "Read Distill UI API data. Paths include /session, /timeline, /notes, /topics, /tags, /stats and /problems.",
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = metadata(json!({"ui": {"visibility": ["app"]}})))]
    async fn ui_read(
        &self,
        Parameters(p): Parameters<ReadParams>,
    ) -> Result<CallToolResult, McpError> {
        response(crate::ui::mcp_request(Method::GET, &p.path, None).await)
    }

    #[tool(name = "distill_ui_write", description = "Edit annotations, rename or merge topics, dismiss duplicates and resolve conflicts through the Distill UI API.",
        annotations(read_only_hint = false, destructive_hint = true, open_world_hint = false),
        meta = metadata(json!({"ui": {"visibility": ["app"]}})))]
    async fn ui_write(
        &self,
        Parameters(p): Parameters<WriteParams>,
    ) -> Result<CallToolResult, McpError> {
        let method = match p.method {
            WriteMethod::Post => Method::POST,
            WriteMethod::Put => Method::PUT,
            WriteMethod::Delete => Method::DELETE,
        };
        response(crate::ui::mcp_request(method, &p.path, p.body).await)
    }
}

fn response(result: anyhow::Result<crate::ui::McpResponse>) -> Result<CallToolResult, McpError> {
    match result {
        Ok(value) => Ok(CallToolResult::structured(
            serde_json::to_value(value)
                .map_err(|e| McpError::internal_error(format!("encoding UI result: {e}"), None))?,
        )),
        Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
            "{error:#}"
        ))])),
    }
}

fn metadata(value: Value) -> MetaObject {
    MetaObject(value.as_object().expect("metadata object").clone())
}
