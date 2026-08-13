use serde::{Deserialize, Serialize};

use super::common::RecordTimestamps;

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
pub struct ModelProvider {
    pub id: String,
    pub provider_name: String,
    pub api_format: ApiFormat,
    pub base_url: String,
    pub provider_alias: String,
    pub api_key_alias: String,
    #[serde(flatten)]
    pub timestamps: RecordTimestamps,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModelProviderInput {
    pub provider_name: String,
    pub api_format: ApiFormat,
    pub base_url: String,
    pub provider_alias: String,
    pub api_key_alias: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetModelProviderInput {
    pub id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListModelProvidersInput {
    #[serde(default)]
    pub include_deleted: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateModelProviderInput {
    pub id: String,
    pub provider_name: Option<String>,
    pub api_format: Option<ApiFormat>,
    pub base_url: Option<String>,
    pub provider_alias: Option<String>,
    pub api_key_alias: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteModelProviderInput {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::{ApiFormat, CreateModelProviderInput, ModelProvider};

    #[test]
    fn deserializes_camel_case_create_input() {
        let input: CreateModelProviderInput = serde_json::from_value(serde_json::json!({
            "providerName": "OpenAI",
            "apiFormat": "openai",
            "baseUrl": "https://api.openai.com/v1",
            "providerAlias": "default",
            "apiKeyAlias": "provider/default"
        }))
        .expect("valid model provider input");

        assert_eq!(input.provider_name, "OpenAI");
        assert_eq!(input.api_format, ApiFormat::OpenAi);
        assert_eq!(input.api_key_alias, "provider/default");
    }

    #[test]
    fn serializes_timestamps_as_flat_camel_case_fields() {
        let provider = ModelProvider {
            id: "01900000-0000-7000-8000-000000000001".into(),
            provider_name: "Anthropic".into(),
            api_format: ApiFormat::Anthropic,
            base_url: "https://api.anthropic.com".into(),
            provider_alias: "claude".into(),
            api_key_alias: "provider/claude".into(),
            timestamps: super::RecordTimestamps {
                created_at: "2026-08-13T13:23:22.487Z".into(),
                updated_at: "2026-08-13T13:23:22.487Z".into(),
            },
        };

        let value = serde_json::to_value(provider).expect("serializable model provider");

        assert_eq!(value["apiFormat"], "anthropic");
        assert_eq!(value["createdAt"], "2026-08-13T13:23:22.487Z");
        assert!(value.get("timestamps").is_none());
        assert!(value.get("deletedAt").is_none());
    }
}
