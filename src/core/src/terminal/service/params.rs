use serde_json::Value;
use uuid::Uuid;

use super::shell::detect_default_shell;

pub(super) fn parse_start_params(params: &Option<Value>) -> (String, u16, u16) {
    let default_shell = detect_default_shell();
    let p = params.as_ref();
    let shell = p
        .and_then(|v| v.get("shell"))
        .and_then(|v| v.as_str())
        .unwrap_or(&default_shell)
        .to_string();
    let cols = p
        .and_then(|v| v.get("cols"))
        .and_then(|v| v.as_u64())
        .unwrap_or(80) as u16;
    let rows = p
        .and_then(|v| v.get("rows"))
        .and_then(|v| v.as_u64())
        .unwrap_or(24) as u16;
    (shell, cols, rows)
}

pub(super) fn parse_session_id(params: &Option<Value>) -> Option<Uuid> {
    params
        .as_ref()?
        .get("session_id")?
        .as_str()?
        .parse::<Uuid>()
        .ok()
}

pub(super) fn parse_attach_params(params: &Option<Value>) -> Option<(Uuid, bool, usize)> {
    let session_id = parse_session_id(params)?;
    let replay = params
        .as_ref()
        .and_then(|v| v.get("replay"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_bytes = params
        .as_ref()
        .and_then(|v| v.get("max_bytes").or_else(|| v.get("maxBytes")))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    Some((session_id, replay, max_bytes))
}

pub(super) fn parse_resize_params(params: &Option<Value>) -> Option<(Uuid, u16, u16)> {
    let p = params.as_ref()?;
    let session_id = p.get("session_id")?.as_str()?.parse::<Uuid>().ok()?;
    let cols = p.get("cols")?.as_u64()? as u16;
    let rows = p.get("rows")?.as_u64()? as u16;
    Some((session_id, cols, rows))
}

pub(super) fn parse_input_params(params: &Option<Value>) -> Option<(Uuid, String)> {
    let p = params.as_ref()?;
    let session_id = p.get("session_id")?.as_str()?.parse::<Uuid>().ok()?;
    let data = p.get("data")?.as_str()?.to_string();
    Some((session_id, data))
}
