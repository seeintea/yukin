use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordTimestamps {
    pub created_at: String,
    pub updated_at: String,
}
