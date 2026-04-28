#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::super::tools::{
        default_brave_search_endpoint, default_firecrawl_base_url, default_searxng_api_key_header,
        default_web_fetch_user_agent, default_web_search_provider,
    };
    use super::super::{HomieConfig, ToolProviderConfig};

    #[test]
    fn web_config_empty_override_strings_use_safe_defaults() {
        let raw = r#"
        [tools.web.fetch]
        user_agent = "   "

        [tools.web.fetch.firecrawl]
        base_url = ""

        [tools.web.search]
        provider = "  "

        [tools.web.search.brave]
        endpoint = ""

        [tools.web.search.searxng]
        api_key_header = "    "
        "#;
        let config: HomieConfig = toml::from_str(raw).expect("parse config");
        assert_eq!(
            config.tools.web.fetch.user_agent,
            default_web_fetch_user_agent()
        );
        assert_eq!(
            config.tools.web.fetch.firecrawl.base_url,
            default_firecrawl_base_url()
        );
        assert_eq!(
            config.tools.web.search.provider,
            default_web_search_provider()
        );
        assert_eq!(
            config.tools.web.search.brave.endpoint,
            default_brave_search_endpoint()
        );
        assert_eq!(
            config.tools.web.search.searxng.api_key_header,
            default_searxng_api_key_header()
        );
    }

    #[test]
    fn web_config_empty_numeric_bool_and_headers_strings_parse() {
        let raw = r#"
        [tools.web.fetch]
        enabled = ""
        max_chars = ""
        timeout_seconds = ""
        cache_ttl_minutes = ""
        max_redirects = ""
        readability = ""

        [tools.web.fetch.firecrawl]
        enabled = ""
        only_main_content = ""
        max_age_ms = ""
        timeout_seconds = ""

        [tools.web.search]
        enabled = ""
        timeout_seconds = ""
        cache_ttl_minutes = ""
        max_results = ""

        [tools.web.search.searxng]
        headers = ""
        "#;
        let parsed = toml::from_str::<HomieConfig>(raw);
        assert!(parsed.is_ok(), "empty overrides should parse: {parsed:?}");
    }

    #[test]
    fn tools_provider_overrides_parse() {
        let raw = r#"
        [tools.providers.core]
        enabled = true
        channels = ["web", "discord"]
        allow_tools = ["read", "ls"]
        deny_tools = ["exec"]

        [tools.providers.channel_discord]
        enabled = false
        "#;
        let config: HomieConfig = toml::from_str(raw).expect("parse config");
        let core = config
            .tools
            .providers
            .get("core")
            .expect("core provider override present");
        assert_eq!(core.enabled, Some(true));
        assert_eq!(
            core.channels,
            vec!["web".to_string(), "discord".to_string()]
        );
        assert_eq!(core.allow_tools, vec!["read".to_string(), "ls".to_string()]);
        assert_eq!(core.deny_tools, vec!["exec".to_string()]);
        let discord = config
            .tools
            .providers
            .get("channel_discord")
            .expect("channel_discord provider override present");
        assert_eq!(discord.enabled, Some(false));
    }

    #[test]
    fn tool_provider_default_is_empty() {
        let cfg = ToolProviderConfig::default();
        assert_eq!(cfg.enabled, None);
        assert!(cfg.channels.is_empty());
        assert!(cfg.allow_tools.is_empty());
        assert!(cfg.deny_tools.is_empty());
    }
}
