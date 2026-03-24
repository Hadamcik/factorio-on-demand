use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rusqlite::Connection;

use crate::state::AppState;

#[derive(serde::Deserialize)]
pub struct SessionsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SessionSummary {
    pub id: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub total_events: i64,
    pub unique_players: i64,
}

pub async fn list_sessions(
    State(state): State<AppState>,
    Query(q): Query<SessionsQuery>,
) -> impl IntoResponse {
    let db_path = {
        let s = state.lock().unwrap();
        s.config.db_path.clone()
    };

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<SessionSummary>::new())).into_response(),
    };

    let limit = q.limit.unwrap_or(10).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut stmt = match conn.prepare(
        r#"
        SELECT
            s.id,
            s.started_at,
            s.ended_at,
            (SELECT COUNT(*) FROM session_player_events e WHERE e.session_id = s.id) AS total_events,
            (SELECT COUNT(DISTINCT player_name) FROM session_player_events e WHERE e.session_id = s.id) AS unique_players
        FROM sessions s
        ORDER BY s.started_at DESC
        LIMIT ? OFFSET ?
        "#,
    ) {
        Ok(stmt) => stmt,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<SessionSummary>::new())).into_response(),
    };

    let rows = match stmt.query_map([limit, offset], |row| {
        Ok(SessionSummary {
            id: row.get(0)?,
            started_at: row.get(1)?,
            ended_at: row.get(2)?,
            total_events: row.get(3)?,
            unique_players: row.get(4)?,
        })
    }) {
        Ok(rows) => rows,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<SessionSummary>::new())).into_response(),
    };

    Json(rows.filter_map(Result::ok).collect::<Vec<_>>()).into_response()
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SessionEventOut {
    pub timestamp: String,
    pub event_type: String,
    pub player_name: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SessionDetail {
    pub id: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub total_events: i64,
    pub unique_players: i64,
    pub max_concurrent_players: i64,
    pub events: Vec<SessionEventOut>,
}

pub async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<i64>,
) -> impl IntoResponse {
    let db_path = {
        let s = state.lock().unwrap();
        s.config.db_path.clone()
    };

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let session_row: Result<(i64, String, Option<String>), _> = conn.query_row(
        "SELECT id, started_at, ended_at FROM sessions WHERE id = ?",
        [session_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );

    let (id, started_at, ended_at) = match session_row {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let total_events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM session_player_events WHERE session_id = ?",
            [session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let unique_players: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT player_name) FROM session_player_events WHERE session_id = ?",
            [session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let max_concurrent_players: i64 = conn
        .query_row(
            r#"
            WITH deltas AS (
                SELECT
                    CASE
                        WHEN event_type = 'join' THEN 1
                        ELSE -1
                    END AS delta,
                    timestamp,
                    id
                FROM session_player_events
                WHERE session_id = ?
            ),
            running AS (
                SELECT
                    SUM(delta) OVER (
                        ORDER BY timestamp, id
                        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
                    ) AS concurrent
                FROM deltas
            )
            SELECT COALESCE(MAX(concurrent), 0) FROM running
            "#,
            [session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut stmt = match conn.prepare(
        "SELECT timestamp, event_type, player_name
         FROM session_player_events
         WHERE session_id = ?
         ORDER BY timestamp, id",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let events = match stmt.query_map([session_id], |row| {
        Ok(SessionEventOut {
            timestamp: row.get(0)?,
            event_type: row.get(1)?,
            player_name: row.get(2)?,
        })
    }) {
        Ok(rows) => rows.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    Json(SessionDetail {
        id,
        started_at,
        ended_at,
        total_events,
        unique_players,
        max_concurrent_players,
        events,
    })
        .into_response()
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
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
    async fn list_sessions_returns_derived_counts() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        init_db(db_path.to_str().unwrap()).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();

        conn.execute(
            "INSERT INTO sessions (id, started_at, ended_at) VALUES (1, ?, ?)",
            rusqlite::params!["2026-03-23T20:00:00Z", "2026-03-23T20:05:00Z"],
        )
            .unwrap();

        conn.execute(
            "INSERT INTO session_player_events (session_id, timestamp, event_type, player_name)
             VALUES
             (1, '2026-03-23T20:01:00Z', 'join', 'Alice'),
             (1, '2026-03-23T20:02:00Z', 'join', 'Bob'),
             (1, '2026-03-23T20:03:00Z', 'leave', 'Alice'),
             (1, '2026-03-23T20:04:00Z', 'leave', 'Bob')",
            [],
        )
            .unwrap();

        let state = make_state(db_path.to_str().unwrap().to_string());

        let app = Router::new()
            .route("/api/sessions", get(list_sessions))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/sessions?limit=10&offset=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let sessions: Vec<SessionSummary> = serde_json::from_slice(&body).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, 1);
        assert_eq!(sessions[0].total_events, 4);
        assert_eq!(sessions[0].unique_players, 2);
    }

    #[tokio::test]
    async fn get_session_returns_events_and_max_concurrent() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        init_db(db_path.to_str().unwrap()).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();

        conn.execute(
            "INSERT INTO sessions (id, started_at, ended_at) VALUES (1, ?, ?)",
            rusqlite::params!["2026-03-23T20:00:00Z", "2026-03-23T20:05:00Z"],
        )
            .unwrap();

        conn.execute(
            "INSERT INTO session_player_events (session_id, timestamp, event_type, player_name)
             VALUES
             (1, '2026-03-23T20:01:00Z', 'join', 'Alice'),
             (1, '2026-03-23T20:02:00Z', 'join', 'Bob'),
             (1, '2026-03-23T20:03:00Z', 'leave', 'Alice'),
             (1, '2026-03-23T20:04:00Z', 'leave', 'Bob')",
            [],
        )
            .unwrap();

        let state = make_state(db_path.to_str().unwrap().to_string());

        let app = Router::new()
            .route("/api/sessions/{id}", get(get_session))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/sessions/1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let detail: SessionDetail = serde_json::from_slice(&body).unwrap();

        assert_eq!(detail.id, 1);
        assert_eq!(detail.total_events, 4);
        assert_eq!(detail.unique_players, 2);
        assert_eq!(detail.max_concurrent_players, 2);
        assert_eq!(detail.events.len(), 4);
        assert_eq!(detail.events[0].event_type, "join");
        assert_eq!(detail.events[0].player_name, "Alice");
        assert_eq!(detail.events[1].player_name, "Bob");
    }

    #[tokio::test]
    async fn get_session_returns_not_found_for_missing_session() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        init_db(db_path.to_str().unwrap()).unwrap();

        let state = make_state(db_path.to_str().unwrap().to_string());

        let app = Router::new()
            .route("/api/sessions/{id}", get(get_session))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/sessions/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
