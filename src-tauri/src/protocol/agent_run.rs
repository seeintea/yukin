use serde::{Deserialize, Serialize};

use super::{conversation::Message, model_provider::ReasoningEffort};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    pub conversation_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResponse {
    pub run_id: String,
    pub user_message_id: String,
    pub assistant_message_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub conversation_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub status: RunStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub run: Run,
    pub assistant_message: Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Thinking,
    Responding,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub schema_version: u8,
    pub event_id: String,
    pub conversation_id: String,
    pub run_id: String,
    pub sequence: u64,
    pub timestamp: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "event",
    content = "data",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum EventKind {
    RunStarted {
        user_message_id: String,
        assistant_message_id: String,
    },
    PhaseChanged {
        phase: Phase,
    },
    OutputTextDelta {
        content: String,
    },
    UsageUpdated {
        prompt_tokens: u64,
        completion_tokens: u64,
        total_tokens: u64,
    },
    RunCompleted {},
    RunFailed {
        error_code: String,
        error_message: String,
    },
    RunCancelled {},
}

#[cfg(test)]
mod tests {
    use super::{Event, EventKind, Phase, StartRequest};

    #[test]
    fn deserializes_start_request() {
        let request: StartRequest = serde_json::from_value(serde_json::json!({
            "conversationId": "conversation-1",
            "providerId": "provider-1",
            "modelId": "model-1",
            "reasoningEffort": "high",
            "content": "你好"
        }))
        .expect("valid run start request");

        assert_eq!(request.conversation_id, "conversation-1");
        assert_eq!(request.content, "你好");
    }

    #[test]
    fn serializes_versioned_event_envelope() {
        let event = Event {
            schema_version: 1,
            event_id: "event-1".into(),
            conversation_id: "conversation-1".into(),
            run_id: "run-1".into(),
            sequence: 2,
            timestamp: "2026-08-19T10:00:00.000Z".into(),
            kind: EventKind::PhaseChanged {
                phase: Phase::Thinking,
            },
        };

        let value = serde_json::to_value(event).expect("serializable run event");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["runId"], "run-1");
        assert_eq!(value["sequence"], 2);
        assert_eq!(value["event"], "phase_changed");
        assert_eq!(value["data"]["phase"], "thinking");
    }
}
