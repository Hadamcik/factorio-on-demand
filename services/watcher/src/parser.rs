use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerEvent {
    Join { timestamp_prefix: String, player_name: String },
    Leave { timestamp_prefix: String, player_name: String },
}

pub struct Parser {
    join_re: Regex,
    leave_re: Regex,
}

impl Parser {
    pub fn new() -> Result<Self, String> {
        let join_re = Regex::new(
            r"^(?P<ts>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[JOIN\] (?P<player>.+) joined the game$",
        )
            .map_err(|e| format!("invalid join regex: {e}"))?;

        let leave_re = Regex::new(
            r"^(?P<ts>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[LEAVE\] (?P<player>.+) left the game$",
        )
            .map_err(|e| format!("invalid leave regex: {e}"))?;

        Ok(Self { join_re, leave_re })
    }

    pub fn parse_line(&self, line: &str) -> Option<PlayerEvent> {
        if let Some(captures) = self.join_re.captures(line) {
            return Some(PlayerEvent::Join {
                timestamp_prefix: captures.name("ts")?.as_str().to_string(),
                player_name: captures.name("player")?.as_str().to_string(),
            });
        }

        if let Some(captures) = self.leave_re.captures(line) {
            return Some(PlayerEvent::Leave {
                timestamp_prefix: captures.name("ts")?.as_str().to_string(),
                player_name: captures.name("player")?.as_str().to_string(),
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_join_line() {
        let parser = Parser::new().unwrap();
        let line = "2026-03-23 16:32:14 [JOIN] EntropyEater joined the game";

        let event = parser.parse_line(line);

        assert_eq!(
            event,
            Some(PlayerEvent::Join {
                timestamp_prefix: "2026-03-23 16:32:14".to_string(),
                player_name: "EntropyEater".to_string(),
            })
        );
    }

    #[test]
    fn parses_leave_line() {
        let parser = Parser::new().unwrap();
        let line = "2026-03-23 16:32:15 [LEAVE] EntropyEater left the game";

        let event = parser.parse_line(line);

        assert_eq!(
            event,
            Some(PlayerEvent::Leave {
                timestamp_prefix: "2026-03-23 16:32:15".to_string(),
                player_name: "EntropyEater".to_string(),
            })
        );
    }

    #[test]
    fn ignores_unrelated_line() {
        let parser = Parser::new().unwrap();
        let line = "=== Log opened 2026-03-23 16:31:57 ===";

        assert_eq!(parser.parse_line(line), None);
    }
}
