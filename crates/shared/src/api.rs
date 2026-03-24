use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SessionStartRequest {
    pub timestamp: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SessionStartResponse {
    pub session_id: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SessionEventRequest {
    pub timestamp: String,
    pub event_type: String,
    pub player_name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SessionEndRequest {
    pub timestamp: String,
}
