use std::{env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub log_path: PathBuf,
    pub dashboard_url: String,
    pub internal_api_token: String,
    pub first_join_timeout_seconds: u64,
    pub empty_server_timeout_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let _ = dotenvy::from_path("services/watcher/.env");

        let log_path = env::var("LOG_PATH")
            .unwrap_or_else(|_| "/home/timelord-master/factorio/runtime/console.log".to_string());

        let dashboard_url = env::var("DASHBOARD_URL")
            .map_err(|_| "DASHBOARD_URL must be set".to_string())?;

        let internal_api_token = env::var("INTERNAL_API_TOKEN")
            .map_err(|_| "INTERNAL_API_TOKEN must be set".to_string())?;

        let first_join_timeout_seconds = env::var("FIRST_JOIN_TIMEOUT_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10 * 60);

        let empty_server_timeout_seconds = env::var("EMPTY_SERVER_TIMEOUT_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(2 * 60);

        Ok(Self {
            log_path: PathBuf::from(log_path),
            dashboard_url: dashboard_url.trim_end_matches('/').to_string(),
            internal_api_token,
            first_join_timeout_seconds,
            empty_server_timeout_seconds,
        })
    }
}
