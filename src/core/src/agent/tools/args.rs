use std::collections::HashMap;

use roci::error::RociError;
use roci::tools::ToolArguments;
use serde_json::{Map, Number, Value};

pub(super) struct ParsedToolArgs {
    map: Map<String, Value>,
    literal: Option<String>,
}

impl ParsedToolArgs {
    pub(super) fn new(args: &ToolArguments) -> Result<Self, RociError> {
        match args.raw() {
            Value::Object(map) => Ok(Self {
                map: map.clone(),
                literal: None,
            }),
            Value::Null => Ok(Self {
                map: Map::new(),
                literal: None,
            }),
            Value::String(raw) => {
                if raw.trim().is_empty() {
                    return Ok(Self {
                        map: Map::new(),
                        literal: None,
                    });
                }

                if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                    return match parsed {
                        Value::Object(map) => Ok(Self { map, literal: None }),
                        Value::String(value) => Ok(Self {
                            map: Map::new(),
                            literal: Some(value),
                        }),
                        Value::Number(number) => Ok(Self {
                            map: Map::new(),
                            literal: Some(number.to_string()),
                        }),
                        Value::Bool(value) => Ok(Self {
                            map: Map::new(),
                            literal: Some(value.to_string()),
                        }),
                        _ => Err(RociError::InvalidArgument(
                            "arguments must be a JSON object or string".into(),
                        )),
                    };
                }

                Ok(Self {
                    map: Map::new(),
                    literal: Some(raw.clone()),
                })
            }
            Value::Number(number) => Ok(Self {
                map: Map::new(),
                literal: Some(number.to_string()),
            }),
            Value::Bool(value) => Ok(Self {
                map: Map::new(),
                literal: Some(value.to_string()),
            }),
            _ => Err(RociError::InvalidArgument(
                "arguments must be a JSON object or string".into(),
            )),
        }
    }

    pub(super) fn literal(&self) -> Option<&str> {
        self.literal.as_deref()
    }

    pub(super) fn get_string(&self, key: &str) -> Result<Option<String>, RociError> {
        let Some(value) = self.map.get(key) else {
            return Ok(None);
        };

        match value {
            Value::Null => Ok(None),
            Value::String(text) => Ok(Some(text.clone())),
            Value::Number(number) => Ok(Some(number.to_string())),
            Value::Bool(value) => Ok(Some(value.to_string())),
            _ => Err(RociError::InvalidArgument(format!(
                "{key} must be a string"
            ))),
        }
    }

    pub(super) fn get_string_any(&self, keys: &[&str]) -> Result<Option<String>, RociError> {
        for key in keys {
            if let Some(value) = self.get_string(key)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    pub(super) fn get_u64(&self, key: &str) -> Result<Option<u64>, RociError> {
        let Some(value) = self.map.get(key) else {
            return Ok(None);
        };

        let Some(parsed) = parse_u64(value) else {
            return Err(RociError::InvalidArgument(format!(
                "{key} must be a non-negative integer"
            )));
        };
        Ok(Some(parsed))
    }

    pub(super) fn get_u64_any(&self, keys: &[&str]) -> Result<Option<u64>, RociError> {
        for key in keys {
            if let Some(value) = self.get_u64(key)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    pub(super) fn get_usize(&self, key: &str) -> Result<Option<usize>, RociError> {
        let Some(value) = self.map.get(key) else {
            return Ok(None);
        };

        let Some(parsed_u64) = parse_u64(value) else {
            return Err(RociError::InvalidArgument(format!(
                "{key} must be a non-negative integer"
            )));
        };
        let parsed = usize::try_from(parsed_u64).map_err(|_| {
            RociError::InvalidArgument(format!("{key} is too large for this platform"))
        })?;
        Ok(Some(parsed))
    }

    pub(super) fn get_usize_any(&self, keys: &[&str]) -> Result<Option<usize>, RociError> {
        for key in keys {
            if let Some(value) = self.get_usize(key)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    pub(super) fn get_bool(&self, key: &str) -> Result<Option<bool>, RociError> {
        let Some(value) = self.map.get(key) else {
            return Ok(None);
        };

        let Some(parsed) = parse_bool(value) else {
            return Err(RociError::InvalidArgument(format!(
                "{key} must be a boolean"
            )));
        };
        Ok(Some(parsed))
    }

    pub(super) fn get_bool_any(&self, keys: &[&str]) -> Result<Option<bool>, RociError> {
        for key in keys {
            if let Some(value) = self.get_bool(key)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    pub(super) fn get_env_map(
        &self,
        key: &str,
    ) -> Result<Option<HashMap<String, String>>, RociError> {
        let Some(value) = self.map.get(key) else {
            return Ok(None);
        };

        let map = match value {
            Value::Object(map) => map,
            _ => {
                return Err(RociError::InvalidArgument(format!(
                    "{key} must be an object"
                )));
            }
        };

        let mut result = HashMap::new();
        for (name, value) in map {
            match value {
                Value::Null => {}
                Value::String(text) => {
                    result.insert(name.clone(), text.clone());
                }
                Value::Number(number) => {
                    result.insert(name.clone(), number.to_string());
                }
                Value::Bool(flag) => {
                    result.insert(name.clone(), flag.to_string());
                }
                _ => {
                    return Err(RociError::InvalidArgument(format!(
                        "env.{name} must be a string"
                    )));
                }
            }
        }

        Ok(Some(result))
    }

    pub(super) fn get_env_map_any(
        &self,
        keys: &[&str],
    ) -> Result<Option<HashMap<String, String>>, RociError> {
        for key in keys {
            if let Some(value) = self.get_env_map(key)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
}

fn parse_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number_as_u64(number),
        Value::String(text) => parse_u64_string(text),
        _ => None,
    }
}

fn number_as_u64(number: &Number) -> Option<u64> {
    number
        .as_u64()
        .or_else(|| number.as_i64().and_then(|value| u64::try_from(value).ok()))
}

fn parse_u64_string(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = trimmed.parse::<u64>() {
        return Some(value);
    }

    let value = trimmed.parse::<f64>().ok()?;
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
        return None;
    }

    let integer = value as u128;
    u64::try_from(integer).ok()
}

fn parse_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => match number.as_i64() {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use roci::tools::ToolArguments;
    use serde_json::json;

    use super::ParsedToolArgs;

    #[test]
    fn parsed_tool_args_accepts_plain_string_literal() {
        let args = ToolArguments::new(json!("echo ok"));
        let parsed = ParsedToolArgs::new(&args).expect("parse args");
        assert_eq!(parsed.literal(), Some("echo ok"));
    }

    #[test]
    fn parsed_tool_args_accepts_json_object_in_string_payload() {
        let args = ToolArguments::new(json!("{\"limit\":\"42\"}"));
        let parsed = ParsedToolArgs::new(&args).expect("parse args");
        assert_eq!(parsed.get_usize("limit").expect("limit"), Some(42));
    }

    #[test]
    fn parsed_tool_args_parses_bool_and_numeric_drift() {
        let args = ToolArguments::new(json!({
            "background": "true",
            "timeout": "12.0"
        }));
        let parsed = ParsedToolArgs::new(&args).expect("parse args");
        assert_eq!(parsed.get_bool("background").expect("bool"), Some(true));
        assert_eq!(parsed.get_u64("timeout").expect("timeout"), Some(12));
    }

    #[test]
    fn parsed_tool_args_rejects_invalid_integer() {
        let args = ToolArguments::new(json!({ "limit": "abc" }));
        let parsed = ParsedToolArgs::new(&args).expect("parse args");
        let err = parsed
            .get_usize("limit")
            .expect_err("invalid limit should error");
        assert_eq!(
            err.to_string(),
            "Invalid argument: limit must be a non-negative integer"
        );
    }

    #[test]
    fn parsed_tool_args_reads_aliases() {
        let args = ToolArguments::new(json!({
            "cmd": "echo hi",
            "tailBytes": "25",
            "detached": "true",
            "environment": {"RETRY":"2"}
        }));
        let parsed = ParsedToolArgs::new(&args).expect("parse args");
        assert_eq!(
            parsed.get_string_any(&["command", "cmd"]).expect("cmd"),
            Some("echo hi".to_string())
        );
        assert_eq!(
            parsed.get_usize_any(&["tail", "tailBytes"]).expect("tail"),
            Some(25)
        );
        assert_eq!(
            parsed
                .get_bool_any(&["background", "detached"])
                .expect("bool"),
            Some(true)
        );
        assert_eq!(
            parsed
                .get_env_map_any(&["env", "environment"])
                .expect("env")
                .expect("map")
                .get("RETRY"),
            Some(&"2".to_string())
        );
    }
}
