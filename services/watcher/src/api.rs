use async_trait::async_trait;
use reqwest::Client;
use shared::api::{
    SessionEndRequest, SessionEventRequest, SessionStartRequest, SessionStartResponse,
};

use crate::{config::Config, time::utc_now_iso};

#[async_trait]
pub trait DashboardClient: Send + Sync {
    async fn start_session(&self) -> Result<i64, String>;
    async fn send_event(
        &self,
        session_id: i64,
        timestamp: String,
        event_type: &str,
        player_name: &str,
    ) -> Result<(), String>;
    async fn end_session(
        &self,
        session_id: i64,
        timestamp: Option<String>,
    ) -> Result<(), String>;
}

#[derive(Clone)]
pub struct DashboardApi {
    client: Client,
    config: Config,
}

impl DashboardApi {
    pub fn new(config: Config) -> Result<Self, String> {
        let client = Client::builder()
            .use_rustls_tls()
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        Ok(Self { client, config })
    }

    async fn post_json<TReq, TResp>(&self, path: &str, payload: &TReq) -> Result<TResp, String>
    where
        TReq: serde::Serialize + ?Sized,
        TResp: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", self.config.dashboard_url, path);

        let response = self
            .client
            .post(url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.internal_api_token),
            )
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read body>".to_string());
            return Err(format!("HTTP {}: {}", status, body));
        }

        response
            .json::<TResp>()
            .await
            .map_err(|e| format!("failed to decode response: {e}"))
    }

    async fn post_no_content<TReq>(&self, path: &str, payload: &TReq) -> Result<(), String>
    where
        TReq: serde::Serialize + ?Sized,
    {
        let url = format!("{}{}", self.config.dashboard_url, path);

        let response = self
            .client
            .post(url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.internal_api_token),
            )
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read body>".to_string());
            return Err(format!("HTTP {}: {}", status, body));
        }

        Ok(())
    }
}

#[async_trait]
impl DashboardClient for DashboardApi {
    async fn start_session(&self) -> Result<i64, String> {
        let response: SessionStartResponse = self
            .post_json(
                "/internal/session/start",
                &SessionStartRequest {
                    timestamp: utc_now_iso(),
                },
            )
            .await?;

        Ok(response.session_id)
    }

    async fn send_event(
        &self,
        session_id: i64,
        timestamp: String,
        event_type: &str,
        player_name: &str,
    ) -> Result<(), String> {
        self.post_no_content(
            &format!("/internal/session/{session_id}/event"),
            &SessionEventRequest {
                timestamp,
                event_type: event_type.to_string(),
                player_name: player_name.to_string(),
            },
        )
            .await
    }

    async fn end_session(
        &self,
        session_id: i64,
        timestamp: Option<String>,
    ) -> Result<(), String> {
        self.post_no_content(
            &format!("/internal/session/{session_id}/end"),
            &SessionEndRequest {
                timestamp: timestamp.unwrap_or_else(utc_now_iso),
            },
        )
            .await
    }
}
