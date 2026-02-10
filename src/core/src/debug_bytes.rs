use std::sync::OnceLock;

use uuid::Uuid;

fn debug_terminal_bytes_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let v = std::env::var("HOMIE_DEBUG_TERMINAL_BYTES").unwrap_or_default();
        matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES")
    })
}

fn debug_terminal_session_filter() -> Option<Uuid> {
    static FILTER: OnceLock<Option<Uuid>> = OnceLock::new();
    FILTER
        .get_or_init(|| {
            std::env::var("HOMIE_DEBUG_TERMINAL_SESSION")
                .ok()
                .and_then(|v| v.parse::<Uuid>().ok())
        })
        .clone()
}

pub fn terminal_debug_enabled_for(session_id: Uuid) -> bool {
    if !debug_terminal_bytes_enabled() {
        return false;
    }
    match debug_terminal_session_filter() {
        Some(filter) => filter == session_id,
        None => true,
    }
}

pub fn fmt_bytes(data: &[u8], max: usize) -> String {
    let show = if max > 0 && data.len() > max {
        &data[..max]
    } else {
        data
    };

    let mut hex = String::new();
    for (i, b) in show.iter().enumerate() {
        if i > 0 {
            hex.push(' ');
        }
        hex.push_str(&format!("{:02x}", b));
    }

    let mut ascii = String::new();
    for &b in show {
        match b {
            b'\x1b' => ascii.push_str("<esc>"),
            b'\r' => ascii.push_str("<cr>"),
            b'\n' => ascii.push_str("<lf>"),
            b'\t' => ascii.push_str("<tab>"),
            0x00 => ascii.push_str("<nul>"),
            0x20..=0x7e => ascii.push(b as char),
            _ => ascii.push('.'),
        }
    }

    format!("len={} hex=[{}] ascii=\"{}\"", data.len(), hex, ascii)
}

pub fn contains_subseq(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}
