use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamRequest {
    pub provider_id: String,
    pub model_id: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "event",
    content = "data",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StreamEvent {
    OutputDelta {
        content: String,
    },
    Completed {
        finish_reason: Option<String>,
        usage: Option<TokenUsage>,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::{StreamEvent, StreamRequest, TokenUsage};

    #[test]
    fn deserializes_stream_request() {
        let request: StreamRequest = serde_json::from_value(serde_json::json!({
            "providerId": "provider-1",
            "modelId": "deepseek-chat",
            "content": "你好"
        }))
        .expect("valid stream request");

        assert_eq!(request.provider_id, "provider-1");
        assert_eq!(request.model_id, "deepseek-chat");
        assert_eq!(request.content, "你好");
    }

    #[test]
    fn serializes_stream_events() {
        let delta = serde_json::to_value(StreamEvent::OutputDelta {
            content: "你".into(),
        })
        .expect("serializable delta");
        assert_eq!(
            delta,
            serde_json::json!({
                "event": "output_delta",
                "data": { "content": "你" }
            })
        );

        let completed = serde_json::to_value(StreamEvent::Completed {
            finish_reason: Some("stop".into()),
            usage: Some(TokenUsage {
                prompt_tokens: 2,
                completion_tokens: 1,
                total_tokens: 3,
            }),
        })
        .expect("serializable completion");
        assert_eq!(
            completed,
            serde_json::json!({
                "event": "completed",
                "data": {
                    "finishReason": "stop",
                    "usage": {
                        "promptTokens": 2,
                        "completionTokens": 1,
                        "totalTokens": 3
                    }
                }
            })
        );
    }
}
