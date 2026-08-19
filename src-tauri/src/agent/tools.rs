use chrono::{FixedOffset, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{RuntimeError, ToolDefinition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RiskLevel {
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApprovalPolicy {
    Never,
}

pub(crate) struct ToolRegistry;

impl ToolRegistry {
    pub(crate) fn built_in() -> Self {
        Self
    }

    pub(crate) fn definitions(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "current_time".into(),
            description: "Get the current date and time for a UTC offset. Use this when the user asks for the current time or date.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "utcOffset": {
                        "type": "string",
                        "description": "UTC offset in ±HH:MM format, for example +08:00. Defaults to +00:00."
                    }
                },
                "additionalProperties": false
            }),
        }]
    }

    pub(crate) fn metadata(&self, name: &str) -> Result<(RiskLevel, ApprovalPolicy), RuntimeError> {
        match name {
            "current_time" => Ok((RiskLevel::ReadOnly, ApprovalPolicy::Never)),
            _ => Err(RuntimeError::ToolNotFound(name.into())),
        }
    }

    pub(crate) async fn execute(
        &self,
        name: &str,
        arguments: &Value,
    ) -> Result<Value, RuntimeError> {
        self.metadata(name)?;
        match name {
            "current_time" => current_time(arguments),
            _ => Err(RuntimeError::ToolNotFound(name.into())),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CurrentTimeArguments {
    #[serde(default = "default_utc_offset")]
    utc_offset: String,
}

fn default_utc_offset() -> String {
    "+00:00".into()
}

fn current_time(arguments: &Value) -> Result<Value, RuntimeError> {
    let arguments: CurrentTimeArguments =
        serde_json::from_value(arguments.clone()).map_err(|error| {
            RuntimeError::InvalidToolArguments {
                name: "current_time".into(),
                message: error.to_string(),
            }
        })?;
    let offset = parse_utc_offset(&arguments.utc_offset)?;
    let local = Utc::now().with_timezone(&offset);

    Ok(json!({
        "dateTime": local.to_rfc3339_opts(SecondsFormat::Secs, true),
        "utcOffset": arguments.utc_offset,
        "unixTimestamp": local.timestamp()
    }))
}

fn parse_utc_offset(value: &str) -> Result<FixedOffset, RuntimeError> {
    let bytes = value.as_bytes();
    let valid_shape = bytes.len() == 6
        && matches!(bytes[0], b'+' | b'-')
        && bytes[3] == b':'
        && bytes[1..3].iter().all(u8::is_ascii_digit)
        && bytes[4..6].iter().all(u8::is_ascii_digit);
    if !valid_shape {
        return Err(invalid_offset(value));
    }
    let hours = value[1..3]
        .parse::<i32>()
        .map_err(|_| invalid_offset(value))?;
    let minutes = value[4..6]
        .parse::<i32>()
        .map_err(|_| invalid_offset(value))?;
    if hours > 23 || minutes > 59 {
        return Err(invalid_offset(value));
    }
    let seconds = (hours * 60 + minutes) * 60 * if bytes[0] == b'-' { -1 } else { 1 };
    FixedOffset::east_opt(seconds).ok_or_else(|| invalid_offset(value))
}

fn invalid_offset(value: &str) -> RuntimeError {
    RuntimeError::InvalidToolArguments {
        name: "current_time".into(),
        message: format!("utcOffset must use ±HH:MM format, received {value}"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::ToolRegistry;
    use crate::agent::RuntimeError;

    #[test]
    fn rejects_unknown_and_invalid_tool_arguments() {
        let runtime = tauri::async_runtime::block_on(
            ToolRegistry::built_in()
                .execute("current_time", &json!({ "utcOffset": "Asia/Shanghai" })),
        );
        assert!(matches!(
            runtime,
            Err(RuntimeError::InvalidToolArguments { .. })
        ));

        let unknown =
            tauri::async_runtime::block_on(ToolRegistry::built_in().execute("missing", &json!({})));
        assert_eq!(unknown, Err(RuntimeError::ToolNotFound("missing".into())));
    }

    #[test]
    fn executes_current_time_for_valid_offset() {
        let output = tauri::async_runtime::block_on(
            ToolRegistry::built_in().execute("current_time", &json!({ "utcOffset": "+08:00" })),
        )
        .expect("current time output");

        assert_eq!(output["utcOffset"], "+08:00");
        assert!(output["dateTime"]
            .as_str()
            .is_some_and(|value| value.ends_with("+08:00")));
    }
}
