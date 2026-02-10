use std::sync::Arc;

use roci::tools::Tool;
use serde_json::Value;

use crate::homie_config::{ToolProviderConfig, ToolsConfig};

use super::{apply_patch, exec, fs, openclaw_browser, process, web, ToolContext};

pub trait ToolProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn is_dynamic(&self) -> bool {
        false
    }
    fn tools(&self, ctx: ToolContext) -> Vec<Arc<dyn Tool>>;
}

#[derive(Debug, Clone)]
pub struct ListedTool {
    pub provider_id: String,
    pub provider_dynamic: bool,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Clone)]
struct ProvidedTool {
    provider_id: String,
    provider_dynamic: bool,
    tool: Arc<dyn Tool>,
}

pub struct CoreToolProvider;

impl ToolProvider for CoreToolProvider {
    fn id(&self) -> &'static str {
        "core"
    }

    fn tools(&self, ctx: ToolContext) -> Vec<Arc<dyn Tool>> {
        let mut tools = vec![
            fs::read_tool(ctx.clone()),
            fs::ls_tool(ctx.clone()),
            fs::find_tool(ctx.clone()),
            fs::grep_tool(ctx.clone()),
            apply_patch::apply_patch_tool(ctx.clone()),
            exec::exec_tool(ctx.clone()),
            process::process_tool(ctx.clone()),
        ];
        if let Some(tool) = web::web_fetch_tool(ctx.clone()) {
            tools.push(tool);
        }
        if let Some(tool) = web::web_search_tool(ctx.clone()) {
            tools.push(tool);
        }
        tools
    }
}

pub struct ToolRegistry {
    providers: Vec<Arc<dyn ToolProvider>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            providers: vec![
                Arc::new(CoreToolProvider),
                Arc::new(openclaw_browser::OpenClawBrowserProvider),
            ],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_provider(mut self, provider: Arc<dyn ToolProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn build_tools(
        &self,
        ctx: ToolContext,
        config: &ToolsConfig,
    ) -> Result<Vec<Arc<dyn Tool>>, String> {
        let tools = self.collect_tools(ctx, config)?;
        Ok(tools.into_iter().map(|entry| entry.tool).collect())
    }

    pub fn list_tools(
        &self,
        ctx: ToolContext,
        config: &ToolsConfig,
    ) -> Result<Vec<ListedTool>, String> {
        let tools = self.collect_tools(ctx, config)?;
        Ok(tools
            .into_iter()
            .map(|entry| ListedTool {
                provider_id: entry.provider_id,
                provider_dynamic: entry.provider_dynamic,
                name: entry.tool.name().to_string(),
                description: entry.tool.description().to_string(),
                input_schema: entry.tool.parameters().schema.clone(),
            })
            .collect())
    }

    fn validate_provider_overrides(&self, config: &ToolsConfig) -> Result<(), String> {
        let known: std::collections::HashSet<&str> = self
            .providers
            .iter()
            .map(|provider| provider.id())
            .collect();
        for (provider_id, override_cfg) in &config.providers {
            if known.contains(provider_id.as_str()) {
                continue;
            }
            let controls_enabled = override_cfg.enabled.unwrap_or(false)
                || !override_cfg.channels.is_empty()
                || !override_cfg.allow_tools.is_empty()
                || !override_cfg.deny_tools.is_empty();
            if controls_enabled {
                return Err(format!(
                    "unknown tool provider `{provider_id}` in tools.providers config"
                ));
            }
        }
        Ok(())
    }

    fn validate_tool_overrides(
        &self,
        provider_id: &str,
        override_cfg: Option<&ToolProviderConfig>,
        tools: &[Arc<dyn Tool>],
    ) -> Result<(), String> {
        let Some(override_cfg) = override_cfg else {
            return Ok(());
        };
        let available: std::collections::HashSet<&str> =
            tools.iter().map(|tool| tool.name()).collect();
        for tool_name in &override_cfg.allow_tools {
            if !available.contains(tool_name.as_str()) {
                return Err(format!(
                    "unknown tool `{tool_name}` in tools.providers.{provider_id}.allow_tools"
                ));
            }
        }
        for tool_name in &override_cfg.deny_tools {
            if !available.contains(tool_name.as_str()) {
                return Err(format!(
                    "unknown tool `{tool_name}` in tools.providers.{provider_id}.deny_tools"
                ));
            }
        }
        Ok(())
    }

    fn collect_tools(
        &self,
        ctx: ToolContext,
        config: &ToolsConfig,
    ) -> Result<Vec<ProvidedTool>, String> {
        self.validate_provider_overrides(config)?;
        let mut tools = Vec::new();
        let mut seen_names: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for provider in &self.providers {
            let override_cfg = config.providers.get(provider.id());
            if !provider_enabled(provider.as_ref(), override_cfg, &ctx.channel) {
                continue;
            }
            let provider_tools = provider.tools(ctx.clone());
            self.validate_tool_overrides(provider.id(), override_cfg, &provider_tools)?;
            for tool in provider_tools {
                if !tool_allowed(tool.name(), override_cfg) {
                    continue;
                }
                if let Some(existing) =
                    seen_names.insert(tool.name().to_string(), provider.id().to_string())
                {
                    return Err(format!(
                        "tool name conflict `{}` between providers `{existing}` and `{}`",
                        tool.name(),
                        provider.id()
                    ));
                }
                tools.push(ProvidedTool {
                    provider_id: provider.id().to_string(),
                    provider_dynamic: provider.is_dynamic(),
                    tool,
                });
            }
        }
        Ok(tools)
    }
}

fn provider_enabled(
    provider: &dyn ToolProvider,
    override_cfg: Option<&ToolProviderConfig>,
    channel: &str,
) -> bool {
    if !provider_allowed_for_channel(override_cfg, channel) {
        return false;
    }
    match override_cfg.and_then(|cfg| cfg.enabled) {
        Some(enabled) => enabled,
        None => !provider.is_dynamic(),
    }
}

fn provider_allowed_for_channel(override_cfg: Option<&ToolProviderConfig>, channel: &str) -> bool {
    let Some(override_cfg) = override_cfg else {
        return true;
    };
    if override_cfg.channels.is_empty() {
        return true;
    }
    override_cfg
        .channels
        .iter()
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .any(|entry| entry.eq_ignore_ascii_case(channel))
}

fn tool_allowed(tool_name: &str, override_cfg: Option<&ToolProviderConfig>) -> bool {
    let Some(override_cfg) = override_cfg else {
        return true;
    };
    let in_allow_list = if override_cfg.allow_tools.is_empty() {
        true
    } else {
        override_cfg
            .allow_tools
            .iter()
            .any(|name| name == tool_name)
    };
    in_allow_list && !override_cfg.deny_tools.iter().any(|name| name == tool_name)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use roci::tools::{AgentTool, AgentToolParameters, Tool};
    use serde_json::json;

    use crate::homie_config::ToolsConfig;

    use super::{ListedTool, ToolContext, ToolProvider, ToolRegistry};

    struct StaticProvider {
        id: &'static str,
        dynamic: bool,
        names: Vec<&'static str>,
    }

    impl ToolProvider for StaticProvider {
        fn id(&self) -> &'static str {
            self.id
        }

        fn is_dynamic(&self) -> bool {
            self.dynamic
        }

        fn tools(&self, _ctx: ToolContext) -> Vec<Arc<dyn Tool>> {
            self.names
                .iter()
                .map(|name| {
                    Arc::new(AgentTool::new(
                        *name,
                        "test",
                        AgentToolParameters::empty(),
                        |_args, _ctx| async move { Ok(json!({"ok": true})) },
                    )) as Arc<dyn Tool>
                })
                .collect()
        }
    }

    fn dummy_ctx() -> ToolContext {
        dummy_ctx_for_channel("web")
    }

    fn dummy_ctx_for_channel(channel: &str) -> ToolContext {
        let homie_config = Arc::new(crate::HomieConfig::default());
        let processes = Arc::new(super::super::ProcessRegistry::new());
        ToolContext::with_processes_and_channel(processes, homie_config, channel)
    }

    #[test]
    fn dynamic_provider_disabled_by_default() {
        let registry = ToolRegistry::new().with_provider(Arc::new(StaticProvider {
            id: "dynamic_x",
            dynamic: true,
            names: vec!["dyn_tool"],
        }));
        let config = ToolsConfig::default();
        let tools = registry.build_tools(dummy_ctx(), &config).expect("build");
        assert!(!tools.iter().any(|tool| tool.name() == "dyn_tool"));
    }

    #[test]
    fn enabled_unknown_provider_override_fails() {
        let mut config = ToolsConfig::default();
        config.providers.insert(
            "missing_provider".into(),
            crate::homie_config::ToolProviderConfig {
                enabled: Some(true),
                channels: Vec::new(),
                allow_tools: Vec::new(),
                deny_tools: Vec::new(),
            },
        );
        let error = match ToolRegistry::new().build_tools(dummy_ctx(), &config) {
            Ok(_) => panic!("unknown provider should fail"),
            Err(error) => error,
        };
        assert!(error.contains("unknown tool provider"));
    }

    #[test]
    fn tool_name_conflict_fails() {
        let registry = ToolRegistry::new()
            .with_provider(Arc::new(StaticProvider {
                id: "dynamic_a",
                dynamic: true,
                names: vec!["dup_tool"],
            }))
            .with_provider(Arc::new(StaticProvider {
                id: "dynamic_b",
                dynamic: true,
                names: vec!["dup_tool"],
            }));
        let mut config = ToolsConfig::default();
        config.providers.insert(
            "dynamic_a".into(),
            crate::homie_config::ToolProviderConfig {
                enabled: Some(true),
                channels: Vec::new(),
                allow_tools: Vec::new(),
                deny_tools: Vec::new(),
            },
        );
        config.providers.insert(
            "dynamic_b".into(),
            crate::homie_config::ToolProviderConfig {
                enabled: Some(true),
                channels: Vec::new(),
                allow_tools: Vec::new(),
                deny_tools: Vec::new(),
            },
        );
        let error = match registry.build_tools(dummy_ctx(), &config) {
            Ok(_) => panic!("conflict should fail"),
            Err(error) => error,
        };
        assert!(error.contains("tool name conflict"));
    }

    #[test]
    fn openclaw_provider_disabled_by_default() {
        let config = ToolsConfig::default();
        let listed = ToolRegistry::new()
            .list_tools(dummy_ctx(), &config)
            .expect("list");
        assert!(!listed
            .iter()
            .any(|tool| tool.provider_id == "openclaw_browser"));
    }

    #[test]
    fn openclaw_provider_enabled_when_overridden() {
        let mut config = ToolsConfig::default();
        config.providers.insert(
            "openclaw_browser".into(),
            crate::homie_config::ToolProviderConfig {
                enabled: Some(true),
                channels: Vec::new(),
                allow_tools: Vec::new(),
                deny_tools: Vec::new(),
            },
        );
        let listed = ToolRegistry::new()
            .list_tools(dummy_ctx(), &config)
            .expect("list");
        let openclaw = listed
            .into_iter()
            .find(|tool: &ListedTool| tool.name == "openclaw_browser")
            .expect("openclaw browser tool");
        assert_eq!(openclaw.provider_id, "openclaw_browser");
        assert!(openclaw.provider_dynamic);
        assert!(openclaw.input_schema.is_object());
    }

    #[test]
    fn provider_channels_exclude_non_matching_context() {
        let registry = ToolRegistry::new().with_provider(Arc::new(StaticProvider {
            id: "channel_gate",
            dynamic: false,
            names: vec!["channel_tool"],
        }));
        let mut config = ToolsConfig::default();
        config.providers.insert(
            "channel_gate".into(),
            crate::homie_config::ToolProviderConfig {
                enabled: Some(true),
                channels: vec!["discord".to_string()],
                allow_tools: Vec::new(),
                deny_tools: Vec::new(),
            },
        );
        let tools = registry
            .build_tools(dummy_ctx_for_channel("web"), &config)
            .expect("build");
        assert!(!tools.iter().any(|tool| tool.name() == "channel_tool"));
    }

    #[test]
    fn provider_channels_include_matching_context() {
        let registry = ToolRegistry::new().with_provider(Arc::new(StaticProvider {
            id: "channel_gate",
            dynamic: false,
            names: vec!["channel_tool"],
        }));
        let mut config = ToolsConfig::default();
        config.providers.insert(
            "channel_gate".into(),
            crate::homie_config::ToolProviderConfig {
                enabled: Some(true),
                channels: vec!["discord".to_string()],
                allow_tools: Vec::new(),
                deny_tools: Vec::new(),
            },
        );
        let tools = registry
            .build_tools(dummy_ctx_for_channel("discord"), &config)
            .expect("build");
        assert!(tools.iter().any(|tool| tool.name() == "channel_tool"));
    }
}
