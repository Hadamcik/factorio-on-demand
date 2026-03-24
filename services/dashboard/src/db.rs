use rusqlite::Connection;

pub fn init_db(path: &str) -> rusqlite::Result<()> {
    let conn = Connection::open(path)?;

    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at TEXT NOT NULL,
            ended_at TEXT
        );

        CREATE TABLE IF NOT EXISTS session_player_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL,
            timestamp TEXT NOT NULL,
            event_type TEXT NOT NULL CHECK (event_type IN ('join', 'leave')),
            player_name TEXT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_started_at
        ON sessions(started_at DESC);

        CREATE INDEX IF NOT EXISTS idx_events_session_time
        ON session_player_events(session_id, timestamp, id);
        "#,
    )?;

    Ok(())
}
