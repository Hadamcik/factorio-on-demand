use std::sync::{Arc, Mutex};

use axum::{routing::{get, post}, Router};
use tokio::sync::broadcast;

use crate::{
    config::AppConfig,
    db
    ,
    routes,
    state::{AppState, AppStateInner},
    status::broadcaster::spawn_status_broadcaster,
};
use crate::routes::static_files::{app_js, index, style_css};
use crate::routes::status::status;
use crate::routes::wake::wake;

pub async fn build_app(config: AppConfig) -> (Router, std::net::SocketAddr) {
    db::init_db(&config.db_path).expect("db init failed");

    let (ws_tx, _ws_rx) = broadcast::channel::<String>(256);

    let bind_addr = config.bind_addr;
    let state: AppState = Arc::new(Mutex::new(AppStateInner::new(
        config,
        ws_tx.clone(),
    )));

    spawn_status_broadcaster(state.clone());

    let app = Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/style.css", get(style_css))
        .route("/ws", get(routes::ws::ws_handler))
        .route("/api/wake", post(wake))
        .route("/api/status", get(status))
        .route("/api/sessions", get(routes::sessions::list_sessions))
        .route("/api/sessions/{id}", get(routes::sessions::get_session))
        .route("/internal/session/start", post(routes::internal::session_start))
        .route("/internal/session/{id}/event", post(routes::internal::session_event))
        .route("/internal/session/{id}/end", post(routes::internal::session_end))
        .with_state(state);

    (app, bind_addr)
}
