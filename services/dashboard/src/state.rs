use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;
use crate::config::AppConfig;

#[derive(Clone, Debug)]
pub struct AppStateInner {
    pub last_wake_started_at: Option<Instant>,
    pub last_wake_message: Option<String>,
    pub config: AppConfig,
    pub ws_tx: broadcast::Sender<String>,
}

impl AppStateInner {
    pub fn new(config: AppConfig, ws_tx: broadcast::Sender<String>) -> Self {
        Self {
            last_wake_started_at: None,
            last_wake_message: None,
            config,
            ws_tx,
        }
    }
    #[cfg(test)]
    pub fn test(db_path: String) -> Self {
        let (ws_tx, _ws_rx) = broadcast::channel(16);

        Self::new(
            AppConfig::test(db_path),
            ws_tx,
        )
    }
}

pub type AppState = Arc<Mutex<AppStateInner>>;

