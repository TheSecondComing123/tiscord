use chrono::{DateTime, Datelike, Utc};

/// Format a Discord ISO 8601 timestamp for display.
///
/// - Within last 60 seconds: "just now"
/// - Within last hour: "Xm ago"
/// - Within last 24 hours: "Xh ago"
/// - Same year: "Mar 22, 12:01"
/// - Different year: "Mar 22, 2025 12:01"
/// - Parse failure: original string unchanged
pub fn format_timestamp(iso: &str) -> String {
    let parsed = match DateTime::parse_from_rfc3339(iso) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => return iso.to_string(),
    };

    let now = Utc::now();
    let duration = now.signed_duration_since(parsed);

    // Within last 60 seconds
    if duration.num_seconds() >= 0 && duration.num_seconds() < 60 {
        return "just now".to_string();
    }

    // Within last hour
    let minutes = duration.num_minutes();
    if minutes >= 0 && minutes < 60 {
        return format!("{}m ago", minutes);
    }

    // Within last 24 hours
    let hours = duration.num_hours();
    if hours >= 0 && hours < 24 {
        return format!("{}h ago", hours);
    }

    // Absolute format
    let current_year = now.year();
    let parsed_year = parsed.year();

    if current_year == parsed_year {
        parsed.format("%b %d, %H:%M").to_string()
    } else {
        parsed.format("%b %d, %Y %H:%M").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_just_now() {
        let now = Utc::now();
        let iso = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert_eq!(format_timestamp(&iso), "just now");
    }

    #[test]
    fn test_just_now_30_seconds() {
        let now = Utc::now();
        let thirty_seconds_ago = now - chrono::Duration::seconds(30);
        let iso = thirty_seconds_ago.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert_eq!(format_timestamp(&iso), "just now");
    }

    #[test]
    fn test_minutes_ago() {
        let now = Utc::now();
        let five_minutes_ago = now - chrono::Duration::minutes(5);
        let iso = five_minutes_ago.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert_eq!(format_timestamp(&iso), "5m ago");
    }

    #[test]
    fn test_one_minute_ago() {
        let now = Utc::now();
        let one_minute_ago = now - chrono::Duration::minutes(1);
        let iso = one_minute_ago.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert_eq!(format_timestamp(&iso), "1m ago");
    }

    #[test]
    fn test_59_minutes_ago() {
        let now = Utc::now();
        let fifty_nine_minutes_ago = now - chrono::Duration::minutes(59);
        let iso = fifty_nine_minutes_ago.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert_eq!(format_timestamp(&iso), "59m ago");
    }

    #[test]
    fn test_hours_ago() {
        let now = Utc::now();
        let three_hours_ago = now - chrono::Duration::hours(3);
        let iso = three_hours_ago.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert_eq!(format_timestamp(&iso), "3h ago");
    }

    #[test]
    fn test_one_hour_ago() {
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);
        let iso = one_hour_ago.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert_eq!(format_timestamp(&iso), "1h ago");
    }

    #[test]
    fn test_23_hours_ago() {
        let now = Utc::now();
        let twenty_three_hours_ago = now - chrono::Duration::hours(23);
        let iso = twenty_three_hours_ago.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert_eq!(format_timestamp(&iso), "23h ago");
    }

    #[test]
    fn test_absolute_format_same_year() {
        // Use a date from earlier this year
        let timestamp = "2026-01-15T14:30:00.000+00:00";
        let result = format_timestamp(timestamp);
        assert!(result.contains("Jan 15"));
        assert!(result.contains("14:30"));
        assert!(!result.contains("2026")); // Should not include year for same year
    }

    #[test]
    fn test_absolute_format_different_year() {
        // Use a date from a previous year
        let timestamp = "2025-06-22T09:45:00.000+00:00";
        let result = format_timestamp(timestamp);
        assert!(result.contains("Jun 22"));
        assert!(result.contains("09:45"));
        assert!(result.contains("2025")); // Should include year for different year
    }

    #[test]
    fn test_invalid_input() {
        let invalid = "not a timestamp";
        assert_eq!(format_timestamp(invalid), "not a timestamp");
    }

    #[test]
    fn test_empty_string() {
        let empty = "";
        assert_eq!(format_timestamp(empty), "");
    }

    #[test]
    fn test_malformed_rfc3339() {
        let malformed = "2026-03-22 12:01:00"; // Missing timezone
        assert_eq!(format_timestamp(malformed), malformed);
    }
}
