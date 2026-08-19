use serde::{Deserialize, Serialize};

use super::common::RecordMetadata;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub metadata: RecordMetadata,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub run_id: Option<String>,
    pub role: MessageRole,
    pub content: String,
    pub status: MessageStatus,
    pub sequence: i64,
    pub metadata: RecordMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    Tool,
}

impl TryFrom<&str> for MessageRole {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "tool" => Ok(Self::Tool),
            value => Err(format!("invalid message role: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Streaming,
    Completed,
    Failed,
    Cancelled,
}

impl TryFrom<&str> for MessageStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "streaming" => Ok(Self::Streaming),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            value => Err(format!("invalid message status: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRequest {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRequest {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::{MessageRole, MessageStatus, RenameRequest};

    #[test]
    fn deserializes_conversation_rename_request() {
        let request: RenameRequest = serde_json::from_value(serde_json::json!({
            "id": "conversation-1",
            "title": "新的标题"
        }))
        .expect("valid rename request");

        assert_eq!(request.id, "conversation-1");
        assert_eq!(request.title, "新的标题");
    }

    #[test]
    fn parses_stored_message_enums() {
        assert_eq!(
            MessageRole::try_from("assistant"),
            Ok(MessageRole::Assistant)
        );
        assert_eq!(
            MessageStatus::try_from("cancelled"),
            Ok(MessageStatus::Cancelled)
        );
        assert!(MessageRole::try_from("unknown").is_err());
        assert!(MessageStatus::try_from("unknown").is_err());
    }
}
