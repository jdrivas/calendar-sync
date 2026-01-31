use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveTime};
use csv::Reader;
use serde::Deserialize;
use std::path::Path;

use crate::event::CalendarEvent;

#[derive(Debug, Deserialize)]
struct CsvRecord {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    location: Option<String>,
    start_date: String,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    end_date: Option<String>,
    #[serde(default)]
    end_time: Option<String>,
}

pub fn parse_csv(path: &Path) -> Result<Vec<CalendarEvent>> {
    let mut reader = Reader::from_path(path)
        .with_context(|| format!("Failed to open CSV file: {}", path.display()))?;

    let mut events = Vec::new();

    for (index, result) in reader.deserialize().enumerate() {
        let record: CsvRecord = result
            .with_context(|| format!("Failed to parse row {}", index + 1))?;

        let event = parse_record(record, index + 1)?;
        events.push(event);
    }

    Ok(events)
}

fn parse_record(record: CsvRecord, row_num: usize) -> Result<CalendarEvent> {
    let start_date = parse_date(&record.start_date)
        .with_context(|| format!("Invalid start_date in row {}: '{}'", row_num, record.start_date))?;

    let end_date = match &record.end_date {
        Some(d) if !d.is_empty() => parse_date(d)
            .with_context(|| format!("Invalid end_date in row {}: '{}'", row_num, d))?,
        _ => start_date,
    };

    let start_time = match &record.start_time {
        Some(t) if !t.is_empty() => Some(parse_time(t)
            .with_context(|| format!("Invalid start_time in row {}: '{}'", row_num, t))?),
        _ => None,
    };

    let end_time = match &record.end_time {
        Some(t) if !t.is_empty() => Some(parse_time(t)
            .with_context(|| format!("Invalid end_time in row {}: '{}'", row_num, t))?),
        _ => None,
    };

    Ok(CalendarEvent {
        title: record.title,
        description: record.description.filter(|s| !s.is_empty()),
        location: record.location.filter(|s| !s.is_empty()),
        organization: None, // CSV doesn't have organization column
        purchased: false,   // CSV doesn't have purchased column
        start_date,
        start_time,
        end_date,
        end_time,
    })
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    // Try common date formats
    let formats = [
        "%Y-%m-%d",
        "%m/%d/%Y",
        "%d/%m/%Y",
        "%Y/%m/%d",
        "%m-%d-%Y",
    ];

    for fmt in formats {
        if let Ok(date) = NaiveDate::parse_from_str(s.trim(), fmt) {
            return Ok(date);
        }
    }

    anyhow::bail!("Could not parse date: '{}'", s)
}

fn parse_time(s: &str) -> Result<NaiveTime> {
    // Try common time formats
    let formats = [
        "%H:%M:%S",
        "%H:%M",
        "%I:%M:%S %p",
        "%I:%M %p",
        "%I:%M%p",
    ];

    let s = s.trim().to_uppercase();

    for fmt in formats {
        if let Ok(time) = NaiveTime::parse_from_str(&s, fmt) {
            return Ok(time);
        }
    }

    anyhow::bail!("Could not parse time: '{}'", s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_formats() {
        assert!(parse_date("2024-01-15").is_ok());
        assert!(parse_date("01/15/2024").is_ok());
        assert!(parse_date("2024/01/15").is_ok());
    }

    #[test]
    fn test_parse_time_formats() {
        assert!(parse_time("14:30").is_ok());
        assert!(parse_time("14:30:00").is_ok());
        assert!(parse_time("2:30 PM").is_ok());
    }
}
