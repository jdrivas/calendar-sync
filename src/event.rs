use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub end_date: NaiveDate,
    pub end_time: Option<NaiveTime>,
}

impl CalendarEvent {
    /// Returns true if this is an all-day event (no specific times)
    pub fn is_all_day(&self) -> bool {
        self.start_time.is_none() && self.end_time.is_none()
    }

    /// Get start as datetime, defaulting to midnight if no time specified
    pub fn start_datetime(&self) -> NaiveDateTime {
        self.start_date.and_time(self.start_time.unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap()))
    }

    /// Get end as datetime, defaulting to end of day if no time specified
    pub fn end_datetime(&self) -> NaiveDateTime {
        self.end_date.and_time(self.end_time.unwrap_or(NaiveTime::from_hms_opt(23, 59, 59).unwrap()))
    }
}

impl fmt::Display for CalendarEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_all_day() {
            write!(f, "[ALL DAY] {} - {}: {}", self.start_date, self.end_date, self.title)
        } else {
            write!(
                f,
                "{} {} - {} {}: {}",
                self.start_date,
                self.start_time.map(|t| t.to_string()).unwrap_or_default(),
                self.end_date,
                self.end_time.map(|t| t.to_string()).unwrap_or_default(),
                self.title
            )
        }
    }
}
