use serde::{Deserialize, Serialize};

use super::common::RecordMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    #[serde(rename = "openai")]
    OpenAi,
    Anthropic,
}

impl ApiFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
        }
    }
}

impl TryFrom<String> for ApiFormat {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "openai" => Ok(Self::OpenAi),
            "anthropic" => Ok(Self::Anthropic),
            _ => Err(format!("unsupported API format: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPreset {
    pub model_id: String,
    pub display_name: String,
    pub supports_thinking: bool,
    pub reasoning_efforts: Vec<ReasoningEffort>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl ReasoningEffort {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

impl TryFrom<String> for ReasoningEffort {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "none" => Ok(Self::None),
            "minimal" => Ok(Self::Minimal),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::XHigh),
            "max" => Ok(Self::Max),
            _ => Err(format!("unsupported reasoning effort: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionPreset {
    pub api_format: ApiFormat,
    pub base_url: String,
    pub models: Vec<ModelPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderPreset {
    pub provider_key: String,
    pub display_name: String,
    pub connections: Vec<ConnectionPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProvider {
    pub id: String,
    pub provider_key: String,
    pub api_format: ApiFormat,
    pub base_url: String,
    pub provider_alias: String,
    #[serde(flatten)]
    pub metadata: RecordMetadata,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    pub provider_key: String,
    pub api_format: ApiFormat,
    pub base_url: String,
    pub provider_alias: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequest {
    pub id: String,
    pub api_format: Option<ApiFormat>,
    pub base_url: Option<String>,
    pub provider_alias: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceCredentialRequest {
    pub id: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionRequest {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResponse {
    pub model_id: String,
    pub latency_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::{
        ApiFormat, CreateRequest, ModelPreset, ModelProvider, ReasoningEffort,
        TestConnectionRequest,
    };

    #[test]
    fn deserializes_camel_case_create_request() {
        let request: CreateRequest = serde_json::from_value(serde_json::json!({
            "providerKey": "openai",
            "apiFormat": "openai",
            "baseUrl": "https://api.openai.com/v1",
            "providerAlias": "default",
            "apiKey": "sk-......"
        }))
        .expect("valid model provider request");

        assert_eq!(request.provider_key, "openai");
        assert_eq!(request.api_format, ApiFormat::OpenAi);
        assert_eq!(request.api_key, "sk-......");
    }

    #[test]
    fn serializes_metadata_as_flat_camel_case_fields() {
        let provider = ModelProvider {
            id: "01900000-0000-7000-8000-000000000001".into(),
            provider_key: "anthropic".into(),
            api_format: ApiFormat::Anthropic,
            base_url: "https://api.anthropic.com".into(),
            provider_alias: "claude".into(),
            metadata: super::RecordMetadata {
                created_at: "2026-08-13T13:23:22.487Z".into(),
                updated_at: "2026-08-13T13:23:22.487Z".into(),
            },
        };

        let value = serde_json::to_value(provider).expect("serializable model provider");

        assert_eq!(value["apiFormat"], "anthropic");
        assert_eq!(value["createdAt"], "2026-08-13T13:23:22.487Z");
        assert!(value.get("apiKeyAlias").is_none());
        assert!(value.get("metadata").is_none());
        assert!(value.get("deletedAt").is_none());
    }

    #[test]
    fn serializes_model_capabilities() {
        let model = ModelPreset {
            model_id: "deepseek-v4-pro".into(),
            display_name: "V4 Pro".into(),
            supports_thinking: true,
            reasoning_efforts: vec![ReasoningEffort::High, ReasoningEffort::Max],
        };

        let value = serde_json::to_value(model).expect("serializable model preset");

        assert_eq!(value["supportsThinking"], true);
        assert_eq!(
            value["reasoningEfforts"],
            serde_json::json!(["high", "max"])
        );
    }

    #[test]
    fn deserializes_connection_test_request() {
        let request: TestConnectionRequest = serde_json::from_value(serde_json::json!({
            "providerId": "provider-1",
            "modelId": "deepseek-v4-flash"
        }))
        .expect("valid connection test request");

        assert_eq!(request.provider_id, "provider-1");
        assert_eq!(request.model_id, "deepseek-v4-flash");
    }
}
