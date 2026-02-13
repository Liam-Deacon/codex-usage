use chrono::NaiveTime;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Invalid time format: {0}")]
    InvalidTime(String),
    #[error("Invalid duration format: {0}")]
    InvalidDuration(String),
}

pub fn parse_time(input: &str) -> Result<NaiveTime, ParseError> {
    let input = input.trim().to_lowercase();

    if let Ok(time) = NaiveTime::parse_from_str(&input, "%H:%M") {
        return Ok(time);
    }

    if let Ok(time) = NaiveTime::parse_from_str(&input, "%H:%M:%S") {
        return Ok(time);
    }

    let is_pm = input.ends_with("pm");
    let is_am = input.ends_with("am");

    if is_pm || is_am {
        let core_input = input.trim_end_matches("pm").trim_end_matches("am");
        let (hours_str, minutes_str) = if core_input.contains(':') {
            let parts: Vec<&str> = core_input.split(':').collect();
            (parts[0], parts.get(1).copied())
        } else {
            (core_input, None)
        };

        let hours: u32 = hours_str
            .parse()
            .map_err(|_| ParseError::InvalidTime(input.clone()))?;

        if !(1..=12).contains(&hours) {
            return Err(ParseError::InvalidTime(input.clone()));
        }

        let minutes: u32 = minutes_str
            .unwrap_or("0")
            .parse()
            .map_err(|_| ParseError::InvalidTime(input.clone()))?;

        if minutes > 59 {
            return Err(ParseError::InvalidTime(input.clone()));
        }

        let hours = if is_pm {
            if hours == 12 {
                12
            } else {
                hours + 12
            }
        } else if hours == 12 {
            0
        } else {
            hours
        };

        return NaiveTime::from_hms_opt(hours % 24, minutes, 0)
            .ok_or_else(|| ParseError::InvalidTime(input.clone()));
    }

    Err(ParseError::InvalidTime(input))
}

pub fn parse_duration(input: &str) -> Result<Duration, ParseError> {
    let input = input.trim().to_lowercase();

    if let Ok(seconds) = input.parse::<u64>() {
        return Ok(Duration::from_secs(seconds));
    }

    if input.ends_with("s") {
        let num: f64 = input
            .trim_end_matches('s')
            .parse()
            .map_err(|_| ParseError::InvalidDuration(input.clone()))?;
        return Ok(Duration::from_secs_f64(num));
    }

    if input.ends_with("m") {
        let num: f64 = input
            .trim_end_matches('m')
            .parse()
            .map_err(|_| ParseError::InvalidDuration(input.clone()))?;
        return Ok(Duration::from_secs_f64(num * 60.0));
    }

    if input.ends_with("h") {
        let num: f64 = input
            .trim_end_matches('h')
            .parse()
            .map_err(|_| ParseError::InvalidDuration(input.clone()))?;
        return Ok(Duration::from_secs_f64(num * 3600.0));
    }

    if input.ends_with("d") {
        let num: f64 = input
            .trim_end_matches('d')
            .parse()
            .map_err(|_| ParseError::InvalidDuration(input.clone()))?;
        return Ok(Duration::from_secs_f64(num * 86400.0));
    }

    if let Ok(num) = input.parse::<f64>() {
        return Ok(Duration::from_secs_f64(num));
    }

    Err(ParseError::InvalidDuration(input))
}

#[allow(dead_code)]
pub fn format_duration(duration: &Duration) -> String {
    let total_secs = duration.as_secs();

    if total_secs >= 86400 {
        let days = total_secs / 86400;
        let remainder = total_secs % 86400;
        let hours = remainder / 3600;
        let minutes = (remainder % 3600) / 60;
        if hours > 0 {
            if minutes > 0 {
                format!("{}d{}h{}m", days, hours, minutes)
            } else {
                format!("{}d{}h", days, hours)
            }
        } else if minutes > 0 {
            format!("{}d{}m", days, minutes)
        } else {
            format!("{}d", days)
        }
    } else if total_secs >= 3600 {
        let hours = total_secs / 3600;
        let remainder = total_secs % 3600;
        let minutes = remainder / 60;
        if minutes > 0 {
            format!("{}h{}m", hours, minutes)
        } else {
            format!("{}h", hours)
        }
    } else if total_secs >= 60 {
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;
        if seconds > 0 {
            format!("{}m{}s", minutes, seconds)
        } else {
            format!("{}m", minutes)
        }
    } else {
        format!("{}s", total_secs)
    }
}

pub fn format_time(time: &NaiveTime) -> String {
    time.format("%H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_24h() {
        assert_eq!(
            parse_time("08:00").unwrap(),
            NaiveTime::from_hms_opt(8, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time("14:30").unwrap(),
            NaiveTime::from_hms_opt(14, 30, 0).unwrap()
        );
        assert_eq!(
            parse_time("23:59").unwrap(),
            NaiveTime::from_hms_opt(23, 59, 0).unwrap()
        );
    }

    #[test]
    fn test_parse_time_12h() {
        assert_eq!(
            parse_time("8am").unwrap(),
            NaiveTime::from_hms_opt(8, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time("2pm").unwrap(),
            NaiveTime::from_hms_opt(14, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time("12pm").unwrap(),
            NaiveTime::from_hms_opt(12, 0, 0).unwrap()
        );
        assert_eq!(
            parse_time("12am").unwrap(),
            NaiveTime::from_hms_opt(0, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("60s").unwrap(), Duration::from_secs(60));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("0.5d").unwrap(), Duration::from_secs(43200));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(&Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(&Duration::from_secs(3661)), "1h1m");
        assert_eq!(format_duration(&Duration::from_secs(90061)), "1d1h1m");
    }
}
