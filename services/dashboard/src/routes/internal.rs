use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use rusqlite::Connection;
use serde_json::json;
use shared::api::{SessionEndRequest, SessionEventRequest, SessionStartRequest, SessionStartResponse};
use crate::{
    auth::check_internal_auth,
    state::AppState,
};

pub async fn session_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SessionStartRequest>,
) -> impl IntoResponse {
    let (db_path, token) = {
        let s = state.lock().unwrap();
        (
            s.config.db_path.clone(),
            s.config.internal_api_token.clone(),
        )
    };

    if !check_internal_auth(&headers, &token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if conn
        .execute(
            "INSERT INTO sessions (started_at) VALUES (?)",
            [payload.timestamp],
        )
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let session_id = conn.last_insert_rowid();

    {
        let s = state.lock().unwrap();
        let _ = s.ws_tx.send(
            json!({
            "type": "session_started",
            "session_id": session_id
        })
                .to_string(),
        );
    }

    Json(SessionStartResponse { session_id }).into_response()
}

pub async fn session_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<i64>,
    Json(payload): Json<SessionEventRequest>,
) -> impl IntoResponse {
    let (db_path, token) = {
        let s = state.lock().unwrap();
        (
            s.config.db_path.clone(),
            s.config.internal_api_token.clone(),
        )
    };

    if !check_internal_auth(&headers, &token) {
        return StatusCode::UNAUTHORIZED;
    }

    if payload.event_type != "join" && payload.event_type != "leave" {
        return StatusCode::BAD_REQUEST;
    }

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    match conn.execute(
        "INSERT INTO session_player_events (session_id, timestamp, event_type, player_name)
     VALUES (?, ?, ?, ?)",
        rusqlite::params![
        session_id,
        payload.timestamp,
        payload.event_type,
        payload.player_name
    ],
    ) {
        Ok(_) => {
            let s = state.lock().unwrap();
            let _ = s.ws_tx.send(
                json!({
                "type": "session_event",
                "session_id": session_id
            })
                    .to_string(),
            );

            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub async fn session_end(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<i64>,
    Json(payload): Json<SessionEndRequest>,
) -> impl IntoResponse {
    let (db_path, token) = {
        let s = state.lock().unwrap();
        (
            s.config.db_path.clone(),
            s.config.internal_api_token.clone(),
        )
    };

    if !check_internal_auth(&headers, &token) {
        return StatusCode::UNAUTHORIZED;
    }

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    match conn.execute(
        "UPDATE sessions SET ended_at = ? WHERE id = ?",
        rusqlite::params![payload.timestamp, session_id],
    ) {
        Ok(_) => {
            let s = state.lock().unwrap();
            let _ = s.ws_tx.send(
                json!({
                "type": "session_ended",
                "session_id": session_id
            })
                    .to_string(),
            );

            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        routing::post,
        Router,
    };
    use serde_json::Value;
    use tempfile::tempdir;
    use tower::util::ServiceExt;

    use crate::{
        db::init_db,
        state::{AppState, AppStateInner},
    };

    fn make_state(db_path: String) -> AppState {
        std::sync::Arc::new(std::sync::Mutex::new(AppStateInner::test(db_path)))
    }

    #[tokio::test]
    async fn session_start_requires_auth() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        init_db(db_path.to_str().unwrap()).unwrap();

        let state = make_state(db_path.to_str().unwrap().to_string());

        let app = Router::new()
            .route("/internal/session/start", post(session_start))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/session/start")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"timestamp":"2026-03-23T20:00:00Z"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn session_start_creates_session() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        init_db(db_path.to_str().unwrap()).unwrap();

        let state = make_state(db_path.to_str().unwrap().to_string());

        let app = Router::new()
            .route("/internal/session/start", post(session_start))
            .with_state(state.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/session/start")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"timestamp":"2026-03-23T20:00:00Z"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["session_id"], 1);

        let conn = rusqlite::Connection::open(db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();

        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn session_event_inserts_player_event() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        init_db(db_path.to_str().unwrap()).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO sessions (started_at) VALUES (?)",
            ["2026-03-23T20:00:00Z"],
        )
            .unwrap();

        let state = make_state(db_path.to_str().unwrap().to_string());

        let app = Router::new()
            .route("/internal/session/{id}/event", post(session_event))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/session/1/event")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "timestamp":"2026-03-23T20:01:00Z",
                            "event_type":"join",
                            "player_name":"Alice"
                        }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let conn = rusqlite::Connection::open(db_path).unwrap();
        let row: (String, String, String) = conn
            .query_row(
                "SELECT timestamp, event_type, player_name FROM session_player_events WHERE session_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(row.0, "2026-03-23T20:01:00Z");
        assert_eq!(row.1, "join");
        assert_eq!(row.2, "Alice");
    }

    #[tokio::test]
    async fn session_end_sets_ended_at() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        init_db(db_path.to_str().unwrap()).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO sessions (started_at) VALUES (?)",
            ["2026-03-23T20:00:00Z"],
        )
            .unwrap();

        let state = make_state(db_path.to_str().unwrap().to_string());

        let app = Router::new()
            .route("/internal/session/{id}/end", post(session_end))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/session/1/end")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"timestamp":"2026-03-23T20:05:00Z"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let conn = rusqlite::Connection::open(db_path).unwrap();
        let ended_at: String = conn
            .query_row("SELECT ended_at FROM sessions WHERE id = 1", [], |row| row.get(0))
            .unwrap();

        assert_eq!(ended_at, "2026-03-23T20:05:00Z");
    }
}
