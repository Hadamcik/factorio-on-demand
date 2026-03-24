use std::time::{Duration, Instant};

use crate::{
    api::{DashboardApi, DashboardClient},
    config::Config,
    log_reader::{wait_for_log_file, LogReader},
    logging::log,
    parser::{Parser, PlayerEvent},
    state_machine::WatcherState,
    suspend::suspend_machine,
    time::line_timestamp_to_utc_iso,
};

enum LoopAction {
    Continue,
    Suspend,
}

struct WatcherEngine<C: DashboardClient> {
    client: C,
    state: WatcherState,
    session_id: i64,
    first_join_deadline: Instant,
    empty_since: Option<Instant>,
    empty_server_timeout_seconds: u64,
}

impl<C: DashboardClient> WatcherEngine<C> {
    fn new(
        client: C,
        session_id: i64,
        first_join_timeout_seconds: u64,
        empty_server_timeout_seconds: u64,
        now: Instant,
    ) -> Self {
        Self {
            client,
            state: WatcherState::new(),
            session_id,
            first_join_deadline: now + Duration::from_secs(first_join_timeout_seconds),
            empty_since: None,
            empty_server_timeout_seconds,
        }
    }

    async fn handle_player_event(
        &mut self,
        event: PlayerEvent,
        iso_ts: String,
    ) -> Result<(), String> {
        match event {
            PlayerEvent::Join {
                timestamp_prefix: _,
                player_name,
            } => {
                self.state.on_join(&player_name);
                self.empty_since = None;

                let mut players: Vec<_> = self.state.active_players.iter().cloned().collect();
                players.sort();

                log(format!("Player joined: {player_name}; active={players:?}"));

                self.client
                    .send_event(self.session_id, iso_ts, "join", &player_name)
                    .await?;
            }

            PlayerEvent::Leave {
                timestamp_prefix: _,
                player_name,
            } => {
                self.state.on_leave(&player_name);

                let mut players: Vec<_> = self.state.active_players.iter().cloned().collect();
                players.sort();

                log(format!("Player left: {player_name}; active={players:?}"));

                self.client
                    .send_event(self.session_id, iso_ts, "leave", &player_name)
                    .await?;

                if self.state.empty_timeout_running {
                    self.empty_since = Some(Instant::now());
                    log("Server is now empty; started empty timeout");
                }
            }
        }

        Ok(())
    }

    async fn check_timeouts(&mut self, now: Instant) -> Result<LoopAction, String> {
        if !self.state.first_join_seen && now >= self.first_join_deadline {
            log("No player joined within startup timeout");
            self.client.end_session(self.session_id, None).await?;
            return Ok(LoopAction::Suspend);
        }

        if self.state.first_join_seen && self.state.active_players.is_empty() {
            if let Some(empty_since_at) = self.empty_since {
                if now.duration_since(empty_since_at)
                    >= Duration::from_secs(self.empty_server_timeout_seconds)
                {
                    log("Server stayed empty long enough after players left");
                    self.client.end_session(self.session_id, None).await?;
                    return Ok(LoopAction::Suspend);
                }
            }
        }

        Ok(LoopAction::Continue)
    }
}

pub async fn run(config: Config) -> Result<(), String> {
    let api = DashboardApi::new(config.clone())?;
    let parser = Parser::new()?;

    log(format!("Waiting for log file: {}", config.log_path.display()));
    wait_for_log_file(&config.log_path).await;
    log("Log file detected");

    log("Creating remote session");
    let session_id = api.start_session().await?;
    log(format!("Session started: {session_id}"));

    let mut engine = WatcherEngine::new(
        api,
        session_id,
        config.first_join_timeout_seconds,
        config.empty_server_timeout_seconds,
        Instant::now(),
    );

    let mut log_reader = LogReader::open(config.log_path.clone()).await?;

    loop {
        match engine.check_timeouts(Instant::now()).await? {
            LoopAction::Continue => {}
            LoopAction::Suspend => {
                if let Err(e) = suspend_machine().await {
                    log(format!("Failed to suspend machine: {e}"));
                    return Err(e);
                }
                return Ok(());
            }
        }

        let Some(line) = log_reader.read_next_line().await? else {
            continue;
        };

        log(format!("Read: {line}"));

        let Some(iso_ts) = line_timestamp_to_utc_iso(&line) else {
            continue;
        };

        let Some(event) = parser.parse_line(&line) else {
            continue;
        };

        if let Err(e) = engine.handle_player_event(event, iso_ts).await {
            log(format!("Failed to send dashboard event: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::parser::PlayerEvent;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        StartSession,
        SendEvent {
            session_id: i64,
            event_type: String,
            player_name: String,
        },
        EndSession {
            session_id: i64,
        },
    }

    #[derive(Clone, Default)]
    struct FakeDashboardClient {
        calls: Arc<Mutex<Vec<Call>>>,
        next_session_id: i64,
    }

    impl FakeDashboardClient {
        fn new(next_session_id: i64) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                next_session_id,
            }
        }

        fn calls(&self) -> Vec<Call> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl DashboardClient for FakeDashboardClient {
        async fn start_session(&self) -> Result<i64, String> {
            self.calls.lock().unwrap().push(Call::StartSession);
            Ok(self.next_session_id)
        }

        async fn send_event(
            &self,
            session_id: i64,
            _timestamp: String,
            event_type: &str,
            player_name: &str,
        ) -> Result<(), String> {
            self.calls.lock().unwrap().push(Call::SendEvent {
                session_id,
                event_type: event_type.to_string(),
                player_name: player_name.to_string(),
            });
            Ok(())
        }

        async fn end_session(
            &self,
            session_id: i64,
            _timestamp: Option<String>,
        ) -> Result<(), String> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::EndSession { session_id });
            Ok(())
        }
    }

    #[tokio::test]
    async fn join_event_sends_dashboard_join_call() {
        let client = FakeDashboardClient::new(42);

        let mut engine = WatcherEngine::new(client.clone(), 42, 600, 120, Instant::now());

        engine
            .handle_player_event(
                PlayerEvent::Join {
                    timestamp_prefix: "2026-03-23 20:00:00".to_string(),
                    player_name: "Alice".to_string(),
                },
                "2026-03-23T19:00:00Z".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(
            client.calls(),
            vec![Call::SendEvent {
                session_id: 42,
                event_type: "join".to_string(),
                player_name: "Alice".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn leave_event_sends_dashboard_leave_call() {
        let client = FakeDashboardClient::new(42);

        let mut engine = WatcherEngine::new(client.clone(), 42, 600, 120, Instant::now());

        engine
            .handle_player_event(
                PlayerEvent::Join {
                    timestamp_prefix: "2026-03-23 20:00:00".to_string(),
                    player_name: "Alice".to_string(),
                },
                "2026-03-23T19:00:00Z".to_string(),
            )
            .await
            .unwrap();

        engine
            .handle_player_event(
                PlayerEvent::Leave {
                    timestamp_prefix: "2026-03-23 20:01:00".to_string(),
                    player_name: "Alice".to_string(),
                },
                "2026-03-23T19:01:00Z".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(
            client.calls(),
            vec![
                Call::SendEvent {
                    session_id: 42,
                    event_type: "join".to_string(),
                    player_name: "Alice".to_string(),
                },
                Call::SendEvent {
                    session_id: 42,
                    event_type: "leave".to_string(),
                    player_name: "Alice".to_string(),
                }
            ]
        );
    }

    #[tokio::test]
    async fn first_join_timeout_ends_session() {
        let client = FakeDashboardClient::new(42);

        let now = Instant::now();
        let mut engine = WatcherEngine::new(client.clone(), 42, 600, 120, now);

        let action = engine
            .check_timeouts(now + Duration::from_secs(601))
            .await
            .unwrap();

        assert!(matches!(action, LoopAction::Suspend));
        assert_eq!(client.calls(), vec![Call::EndSession { session_id: 42 }]);
    }

    #[tokio::test]
    async fn empty_server_timeout_ends_session() {
        let client = FakeDashboardClient::new(42);

        let now = Instant::now();
        let mut engine = WatcherEngine::new(client.clone(), 42, 600, 120, now);

        engine
            .handle_player_event(
                PlayerEvent::Join {
                    timestamp_prefix: "2026-03-23 20:00:00".to_string(),
                    player_name: "Alice".to_string(),
                },
                "2026-03-23T19:00:00Z".to_string(),
            )
            .await
            .unwrap();

        engine
            .handle_player_event(
                PlayerEvent::Leave {
                    timestamp_prefix: "2026-03-23 20:01:00".to_string(),
                    player_name: "Alice".to_string(),
                },
                "2026-03-23T19:01:00Z".to_string(),
            )
            .await
            .unwrap();

        let empty_since = engine.empty_since.unwrap();
        let action = engine
            .check_timeouts(empty_since + Duration::from_secs(121))
            .await
            .unwrap();

        assert!(matches!(action, LoopAction::Suspend));
        assert_eq!(
            client.calls(),
            vec![
                Call::SendEvent {
                    session_id: 42,
                    event_type: "join".to_string(),
                    player_name: "Alice".to_string(),
                },
                Call::SendEvent {
                    session_id: 42,
                    event_type: "leave".to_string(),
                    player_name: "Alice".to_string(),
                },
                Call::EndSession { session_id: 42 },
            ]
        );
    }
}
