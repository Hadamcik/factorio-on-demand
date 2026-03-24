use serde::Serialize;
use tokio::{time::Duration};
use crate::state::AppState;
use crate::status::probe::{probe_machine_online, probe_service_online};

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct StatusWsMessage {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub machine_online: bool,
    pub factorio_online: bool,
}

pub fn spawn_status_broadcaster(state: AppState) {
    tokio::spawn(async move {
        let mut last_sent: Option<StatusWsMessage> = None;

        let (ws_tx, machine_check_addr, factorio_check_addr, timeout_ms) = {
            let s = state.lock().unwrap();
            (
                s.ws_tx.clone(),
                s.config.machine_check_addr(),
                s.config.factorio_check_addr(),
                s.config.tcp_timeout_ms
            )
        };

        loop {
            let (machine_online, factorio_online) = tokio::join!(
                probe_machine_online(&machine_check_addr, timeout_ms),
                probe_service_online(&factorio_check_addr, timeout_ms),
            );

            let snapshot = StatusWsMessage {
                message_type: "status",
                machine_online,
                factorio_online,
            };

            if last_sent.as_ref() != Some(&snapshot) {
                if let Ok(payload) = serde_json::to_string(&snapshot) {
                    let _ = ws_tx.send(payload);
                }
                last_sent = Some(snapshot);
            }

            tokio::time::sleep(Duration::from_millis(900)).await;
        }
    });
}
