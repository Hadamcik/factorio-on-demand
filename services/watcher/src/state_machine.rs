use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct WatcherState {
    pub first_join_seen: bool,
    pub active_players: HashSet<String>,
    pub empty_timeout_running: bool,
}

impl WatcherState {
    pub fn new() -> Self {
        Self {
            first_join_seen: false,
            active_players: HashSet::new(),
            empty_timeout_running: false,
        }
    }

    pub fn on_join(&mut self, player: &str) {
        self.active_players.insert(player.to_string());
        self.first_join_seen = true;
        self.empty_timeout_running = false;
    }

    pub fn on_leave(&mut self, player: &str) {
        self.active_players.remove(player);

        if self.first_join_seen && self.active_players.is_empty() {
            self.empty_timeout_running = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_join_marks_session_active() {
        let mut state = WatcherState::new();

        state.on_join("Alice");

        assert!(state.first_join_seen);
        assert!(state.active_players.contains("Alice"));
        assert!(!state.empty_timeout_running);
    }

    #[test]
    fn empty_timeout_starts_when_last_player_leaves() {
        let mut state = WatcherState::new();

        state.on_join("Alice");
        state.on_leave("Alice");

        assert!(state.first_join_seen);
        assert!(state.active_players.is_empty());
        assert!(state.empty_timeout_running);
    }

    #[test]
    fn empty_timeout_clears_when_someone_joins_again() {
        let mut state = WatcherState::new();

        state.on_join("Alice");
        state.on_leave("Alice");
        assert!(state.empty_timeout_running);

        state.on_join("Bob");

        assert!(!state.empty_timeout_running);
        assert!(state.active_players.contains("Bob"));
    }

    #[test]
    fn leaving_unknown_player_does_not_panic() {
        let mut state = WatcherState::new();

        state.on_leave("Ghost");

        assert!(state.active_players.is_empty());
        assert!(!state.first_join_seen);
        assert!(!state.empty_timeout_running);
    }

    #[test]
    fn empty_timeout_starts_only_after_last_player_leaves() {
        let mut state = WatcherState::new();

        state.on_join("Alice");
        state.on_join("Bob");
        state.on_leave("Alice");

        assert!(state.first_join_seen);
        assert!(state.active_players.contains("Bob"));
        assert!(!state.empty_timeout_running);

        state.on_leave("Bob");

        assert!(state.active_players.is_empty());
        assert!(state.empty_timeout_running);
    }

    #[test]
    fn duplicate_join_does_not_break_state() {
        let mut state = WatcherState::new();

        state.on_join("Alice");
        state.on_join("Alice");

        assert!(state.first_join_seen);
        assert_eq!(state.active_players.len(), 1);
        assert!(state.active_players.contains("Alice"));
        assert!(!state.empty_timeout_running);
    }

    #[test]
    fn duplicate_leave_after_empty_is_harmless() {
        let mut state = WatcherState::new();

        state.on_join("Alice");
        state.on_leave("Alice");
        state.on_leave("Alice");

        assert!(state.first_join_seen);
        assert!(state.active_players.is_empty());
        assert!(state.empty_timeout_running);
    }
}
