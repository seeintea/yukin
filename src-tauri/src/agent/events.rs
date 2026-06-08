#![allow(dead_code)]

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    Text { content: String },
    ToolCall { name: String },
    Done,
}
