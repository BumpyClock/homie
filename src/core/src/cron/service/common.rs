use serde::de::DeserializeOwned;
use serde_json::Value;

use homie_protocol::{error_codes, Response};

pub(super) fn parse_required_params<T: DeserializeOwned>(
    req_id: uuid::Uuid,
    params: Option<Value>,
) -> Result<T, Response> {
    match params {
        Some(v) => serde_json::from_value(v).map_err(|e| {
            Response::error(
                req_id,
                error_codes::INVALID_PARAMS,
                format!("invalid params: {e}"),
            )
        }),
        None => Err(Response::error(
            req_id,
            error_codes::INVALID_PARAMS,
            "missing params",
        )),
    }
}

pub(super) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
