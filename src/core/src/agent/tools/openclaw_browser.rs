use std::sync::Arc;

use roci::tools::tool::ToolExecutionContext;
use roci::tools::{AgentTool, AgentToolParameters, Tool, ToolArguments};

use super::registry::ToolProvider;
use super::ToolContext;

pub const PROVIDER_ID: &str = "openclaw_browser";
const TOOL_NAME: &str = "openclaw_browser";

pub struct OpenClawBrowserProvider;

impl ToolProvider for OpenClawBrowserProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn is_dynamic(&self) -> bool {
        true
    }

    fn tools(&self, ctx: ToolContext) -> Vec<Arc<dyn Tool>> {
        vec![openclaw_browser_tool(ctx)]
    }
}

pub fn openclaw_browser_tool(ctx: ToolContext) -> Arc<dyn Tool> {
    let params = AgentToolParameters::object()
        .string_enum(
            "action",
            "Browser action (navigate, click, type, or extract).",
            &["navigate", "click", "type", "extract"],
            true,
        )
        .string("url", "Target URL for navigate action.", false)
        .string("selector", "CSS selector for click/type/extract.", false)
        .string("text", "Text input for type action.", false)
        .number("timeout_ms", "Action timeout in milliseconds.", false)
        .build();

    Arc::new(AgentTool::new(
        TOOL_NAME,
        "OpenClaw browser automation scaffold.",
        params,
        move |_args: ToolArguments, _ctx: ToolExecutionContext| {
            let ctx = ctx.clone();
            async move { Ok(stub_result(&ctx)) }
        },
    ))
}

fn stub_result(ctx: &ToolContext) -> serde_json::Value {
    let endpoint = ctx.openclaw_browser.endpoint.trim();
    if endpoint.is_empty() {
        return serde_json::json!({
            "ok": false,
            "provider": PROVIDER_ID,
            "tool": TOOL_NAME,
            "error": {
                "code": "not_configured",
                "message": "OpenClaw browser provider is not configured.",
                "hint": "Set tools.openclaw_browser.endpoint and enable tools.providers.openclaw_browser.",
            }
        });
    }

    serde_json::json!({
        "ok": false,
        "provider": PROVIDER_ID,
        "tool": TOOL_NAME,
        "error": {
            "code": "not_implemented",
            "message": "OpenClaw browser provider scaffold is enabled but execution is not implemented yet.",
            "endpoint": endpoint,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use roci::tools::tool::ToolExecutionContext;
    use roci::tools::ToolArguments;
    use serde_json::json;

    use crate::homie_config::OpenClawBrowserToolsConfig;

    use super::{openclaw_browser_tool, stub_result, ToolContext};

    #[test]
    fn stub_result_reports_not_configured_when_endpoint_missing() {
        let mut config = crate::HomieConfig::default();
        config.tools.openclaw_browser = OpenClawBrowserToolsConfig::default();
        let ctx = ToolContext::new(Arc::new(config));
        let payload = stub_result(&ctx);
        assert_eq!(payload["error"]["code"], "not_configured");
    }

    #[tokio::test]
    async fn tool_reports_not_implemented_when_endpoint_set() {
        let mut config = crate::HomieConfig::default();
        config.tools.openclaw_browser.endpoint = "http://127.0.0.1:7331".to_string();
        let ctx = ToolContext::new(Arc::new(config));
        let tool = openclaw_browser_tool(ctx);
        let payload = tool
            .execute(
                &ToolArguments::new(json!({"action":"navigate"})),
                &ToolExecutionContext::default(),
            )
            .await
            .expect("tool response");
        assert_eq!(payload["error"]["code"], "not_implemented");
    }
}
