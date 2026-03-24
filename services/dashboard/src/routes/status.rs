use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;

use crate::{
    state::AppState,
    status::probe::{probe_machine_online, probe_service_online},
};

#[derive(Serialize)]
pub struct StatusResponse {
    pub machine_online: bool,
    pub factorio_online: bool,
    pub last_wake_message: Option<String>,
    pub seconds_since_wake: Option<u64>,
    pub waiting_for_machine_online: bool,
}

pub async fn status(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let (machine_check_addr, factorio_check_addr, timeout_ms) = {
        let s = state.lock().unwrap();
        (
            s.config.machine_check_addr(),
            s.config.factorio_check_addr(),
            s.config.tcp_timeout_ms,
        )
    };

    let (machine_online, factorio_online) = tokio::join!(
        probe_machine_online(&machine_check_addr, timeout_ms),
        probe_service_online(&factorio_check_addr, timeout_ms),
    );

    let mut s = state.lock().unwrap();

    let seconds_since_wake = s.last_wake_started_at.map(|t| t.elapsed().as_secs());
    let waiting_for_machine_online = s.last_wake_started_at.is_some() && !machine_online;

    if machine_online {
        s.last_wake_started_at = None;
        s.last_wake_message = None;
    }

    let response = StatusResponse {
        machine_online,
        factorio_online,
        last_wake_message: s.last_wake_message.clone(),
        seconds_since_wake,
        waiting_for_machine_online,
    };

    (StatusCode::OK, Json(response))
}
