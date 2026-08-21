use serde::{Deserialize, Serialize};

use super::{
    conversation::Message,
    file::{DirectoryReference, Reference},
    model_provider::ReasoningEffort,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    pub conversation_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub content: String,
    #[serde(default)]
    pub skill_ids: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<Reference>,
    #[serde(default)]
    pub directory_scopes: Vec<DirectoryReference>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallDecideRequest {
    pub run_id: String,
    pub tool_call_id: String,
    pub arguments_digest: String,
    pub decision: ToolCallDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallDecision {
    Allow,
    Reject,
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

impl TryFrom<&str> for RunStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "waiting_approval" => Ok(Self::WaitingApproval),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            value => Err(format!("invalid run status: {value}")),
        }
    }
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
    pub skills: Vec<RunSkill>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSkill {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub run: Run,
    pub assistant_message: Message,
    pub tool_calls: Vec<ToolCallSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Requested,
    WaitingApproval,
    Running,
    Completed,
    Failed,
    Rejected,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRiskLevel {
    ReadOnly,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalPolicy {
    Never,
    Always,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallSnapshot {
    pub id: String,
    pub run_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub arguments_digest: String,
    pub result: Option<serde_json::Value>,
    pub status: ToolCallStatus,
    pub risk_level: ToolRiskLevel,
    pub approval_policy: ToolApprovalPolicy,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub approval_expires_at: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
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
    ToolCallRequested {
        tool_call_id: String,
        name: String,
        arguments: serde_json::Value,
        arguments_digest: String,
        risk_level: ToolRiskLevel,
        approval_policy: ToolApprovalPolicy,
    },
    ToolApprovalRequired {
        tool_call_id: String,
        arguments_digest: String,
        expires_at: String,
    },
    ToolCallStarted {
        tool_call_id: String,
    },
    ToolCallCompleted {
        tool_call_id: String,
        result: serde_json::Value,
    },
    ToolCallFailed {
        tool_call_id: String,
        error_code: String,
        error_message: String,
    },
    ToolCallRejected {
        tool_call_id: String,
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
    use super::{
        Event, EventKind, Phase, RunStatus, StartRequest, ToolCallDecideRequest, ToolCallDecision,
    };

    #[test]
    fn deserializes_start_request() {
        let request: StartRequest = serde_json::from_value(serde_json::json!({
            "conversationId": "conversation-1",
            "providerId": "provider-1",
            "modelId": "model-1",
            "reasoningEffort": "high",
            "content": "你好"
            ,"skillIds": ["time_assistant"]
            ,"attachments": [{
                "referenceId": "file-1",
                "name": "notes.txt",
                "size": 42
            }]
            ,"directoryScopes": []
        }))
        .expect("valid run start request");

        assert_eq!(request.conversation_id, "conversation-1");
        assert_eq!(request.content, "你好");
        assert_eq!(request.skill_ids, ["time_assistant"]);
        assert_eq!(request.attachments[0].reference_id, "file-1");
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

    #[test]
    fn deserializes_tool_call_decision_bound_to_arguments() {
        let request: ToolCallDecideRequest = serde_json::from_value(serde_json::json!({
            "runId": "run-1",
            "toolCallId": "tool-1",
            "argumentsDigest": "abc123",
            "decision": "allow"
        }))
        .expect("valid tool decision");

        assert_eq!(request.run_id, "run-1");
        assert_eq!(request.tool_call_id, "tool-1");
        assert_eq!(request.arguments_digest, "abc123");
        assert_eq!(request.decision, ToolCallDecision::Allow);
    }

    #[test]
    fn parses_stored_run_status() {
        assert_eq!(RunStatus::try_from("running"), Ok(RunStatus::Running));
        assert_eq!(
            RunStatus::try_from("waiting_approval"),
            Ok(RunStatus::WaitingApproval)
        );
        assert!(RunStatus::try_from("unknown").is_err());
    }
}
