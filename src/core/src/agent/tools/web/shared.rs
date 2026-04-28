use url::Url;

pub(super) fn truncate_str(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (count, ch) in value.chars().enumerate() {
        if count >= max_chars {
            break;
        }
        out.push(ch);
    }
    out
}

pub(super) fn resolve_site_name(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|s| s.to_string()))
}
