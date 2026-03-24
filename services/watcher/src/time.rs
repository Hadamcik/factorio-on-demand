use chrono::{DateTime, Local, NaiveDateTime, Utc};

pub fn utc_now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn line_timestamp_to_utc_iso(line: &str) -> Option<String> {
    if line.len() < 19 {
        return None;
    }

    let ts = &line[..19];
    let naive = NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").ok()?;
    let local_dt: DateTime<Local> = naive.and_local_timezone(Local).single()?;
    let utc_dt: DateTime<Utc> = local_dt.with_timezone(&Utc);

    Some(utc_dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_line() {
        assert_eq!(line_timestamp_to_utc_iso("short"), None);
    }

    #[test]
    fn rejects_invalid_timestamp_prefix() {
        assert_eq!(
            line_timestamp_to_utc_iso("not-a-timestamp [JOIN] Bob joined the game"),
            None
        );
    }

    #[test]
    fn parses_valid_timestamp_prefix() {
        let result = line_timestamp_to_utc_iso("2026-03-23 16:32:14 [JOIN] Bob joined the game");
        assert!(result.is_some());
        assert!(result.unwrap().ends_with('Z'));
    }

    #[test]
    fn parses_full_join_line_timestamp_prefix() {
        let result = line_timestamp_to_utc_iso("2026-03-23 16:32:14 [JOIN] Alice joined the game");
        assert!(result.is_some());
        assert!(result.unwrap().ends_with('Z'));
    }
}
