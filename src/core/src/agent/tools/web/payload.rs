use std::time::Instant;

const DEFAULT_ERROR_MAX_CHARS: usize = 4000;

#[derive(Debug, Clone, Copy)]
pub(super) enum ExtractMode {
    Markdown,
    Text,
}

pub(super) struct FetchPayloadArgs<'a> {
    pub(super) url: &'a str,
    pub(super) final_url: &'a str,
    pub(super) status: u16,
    pub(super) content_type: &'a str,
    pub(super) title: Option<&'a str>,
    pub(super) extract_mode: ExtractMode,
    pub(super) extractor: &'a str,
    pub(super) backend: &'a str,
    pub(super) text: &'a str,
    pub(super) max_chars: usize,
    pub(super) start: Instant,
    pub(super) warning: Option<&'a str>,
}

pub(super) fn build_fetch_payload(args: FetchPayloadArgs<'_>) -> serde_json::Value {
    let (text, truncated) = truncate_text(args.text, args.max_chars);
    let length = text.chars().count();
    let mode = match args.extract_mode {
        ExtractMode::Markdown => "markdown",
        ExtractMode::Text => "text",
    };
    let mut obj = serde_json::json!({
        "url": args.url,
        "finalUrl": args.final_url,
        "status": args.status,
        "contentType": args.content_type,
        "extractMode": mode,
        "extractor": args.extractor,
        "backend": args.backend,
        "truncated": truncated,
        "length": length,
        "fetchedAt": chrono::Utc::now().to_rfc3339(),
        "tookMs": args.start.elapsed().as_millis() as u64,
        "text": text,
    });
    if let Some(title) = args.title {
        if let Some(map) = obj.as_object_mut() {
            map.insert("title".into(), serde_json::Value::String(title.to_string()));
        }
    }
    if let Some(warning) = args.warning {
        if let Some(map) = obj.as_object_mut() {
            map.insert(
                "warning".into(),
                serde_json::Value::String(warning.to_string()),
            );
        }
    }
    obj
}

pub(super) fn format_fetch_error_detail(detail: &str, content_type: Option<&str>) -> String {
    let trimmed = detail.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut text = trimmed.to_string();
    if content_type
        .map(|v| v.to_lowercase().contains("text/html"))
        .unwrap_or(false)
        || looks_like_html(trimmed)
    {
        let markdown = htmd::convert(trimmed).unwrap_or_else(|_| trimmed.to_string());
        text = markdown_to_text(&markdown);
    }
    super::shared::truncate_str(&text, DEFAULT_ERROR_MAX_CHARS)
}

fn truncate_text(text: &str, max_chars: usize) -> (String, bool) {
    if max_chars == 0 {
        return (String::new(), text.is_empty());
    }
    let mut out = String::new();
    for (count, ch) in text.chars().enumerate() {
        if count >= max_chars {
            break;
        }
        out.push(ch);
    }
    let truncated = text.chars().count() > max_chars;
    (out, truncated)
}

fn looks_like_html(value: &str) -> bool {
    let trimmed = value.trim_start().to_lowercase();
    trimmed.starts_with("<!doctype html") || trimmed.starts_with("<html")
}

pub(super) fn markdown_to_text(markdown: &str) -> String {
    use pulldown_cmark::{Event, Parser};
    let mut out = String::new();
    for event in Parser::new(markdown) {
        match event {
            Event::Text(text) | Event::Code(text) => out.push_str(&text),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            _ => {}
        }
    }
    out
}
