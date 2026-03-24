use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use wake_on_lan::MagicPacket;

use crate::{state::AppState};

#[derive(Serialize)]
pub struct WakeResponse {
    pub ok: bool,
    pub message: String,
}

pub async fn wake(
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    let target_mac = {
        let s = state.lock().unwrap();
        s.config.target_mac
    };
    let magic_packet = MagicPacket::new(&target_mac);

    let response = match magic_packet.send() {
        Ok(_) => WakeResponse {
            ok: true,
            message: "Wake request sent successfully.".to_string(),
        },
        Err(e) => WakeResponse {
            ok: false,
            message: format!("Failed to send wake request: {e}"),
        },
    };

    let mut s = state.lock().unwrap();
    s.last_wake_message = Some(response.message.clone());

    if response.ok {
        s.last_wake_started_at = Some(std::time::Instant::now());
    }

    (StatusCode::OK, Json(response))
}
