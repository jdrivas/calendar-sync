use anyhow::{Context, Result};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use serde::Deserialize;
use std::collections::HashMap;

use crate::event::CalendarEvent;

const CODA_API_BASE: &str = "https://coda.io/apis/v1";
const DEFAULT_EVENT_DURATION_MINUTES: i64 = 150; // 2.5 hours

#[derive(Debug, Deserialize)]
struct CodaRowsResponse {
    items: Vec<CodaRow>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodaRow {
    values: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CodaTablesResponse {
    items: Vec<CodaTable>,
}

#[derive(Debug, Deserialize)]
pub struct CodaTable {
    pub id: String,
    pub name: String,
    #[serde(rename = "tableType")]
    pub table_type: String,
}

pub struct CodaClient {
    client: reqwest::Client,
    api_token: String,
}

impl CodaClient {
    pub fn new(api_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_token,
        }
    }

    pub async fn list_tables(&self, doc_id: &str) -> Result<Vec<CodaTable>> {
        let url = format!("{}/docs/{}/tables", CODA_API_BASE, doc_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to fetch tables from Coda")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Coda API error ({}): {}", status, body);
        }

        let tables_response: CodaTablesResponse = response
            .json()
            .await
            .context("Failed to parse Coda tables response")?;

        Ok(tables_response.items)
    }

    pub async fn fetch_events(
        &self,
        doc_id: &str,
        table_id: &str,
    ) -> Result<Vec<CalendarEvent>> {
        let mut all_events = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut url = format!(
                "{}/docs/{}/tables/{}/rows?useColumnNames=true",
                CODA_API_BASE, doc_id, table_id
            );

            if let Some(token) = &page_token {
                url.push_str(&format!("&pageToken={}", token));
            }

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.api_token))
                .send()
                .await
                .context("Failed to fetch rows from Coda")?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("Coda API error ({}): {}", status, body);
            }

            let rows_response: CodaRowsResponse = response
                .json()
                .await
                .context("Failed to parse Coda response")?;

            for row in rows_response.items {
                match parse_coda_row(&row.values) {
                    Ok(event) => all_events.push(event),
                    Err(e) => {
                        tracing::warn!("Skipping row due to parse error: {}", e);
                    }
                }
            }

            page_token = rows_response.next_page_token;
            if page_token.is_none() {
                break;
            }
        }

        Ok(all_events)
    }
}

fn parse_coda_row(values: &HashMap<String, serde_json::Value>) -> Result<CalendarEvent> {
    // Extract Display -> title
    let title = get_string_value(values, "Display")
        .context("Missing 'Display' column")?;

    // Extract performanceDate -> start_date, start_time
    let performance_date_str = get_string_value(values, "performanceDate")
        .context("Missing 'performanceDate' column")?;
    
    let (start_date, start_time) = parse_coda_datetime(&performance_date_str)
        .with_context(|| format!("Invalid performanceDate: '{}'", performance_date_str))?;

    // Calculate end time (start + 2.5 hours)
    let end_time = start_time.map(|t| {
        let start_dt = NaiveDateTime::new(start_date, t);
        let end_dt = start_dt + Duration::minutes(DEFAULT_EVENT_DURATION_MINUTES);
        end_dt.time()
    });

    // Extract Organization
    let organization = get_string_value(values, "Organization").ok();

    // Extract Purchased (check if value is "Yes" or truthy)
    let purchased = get_string_value(values, "Purchased")
        .map(|v| v.to_lowercase() == "yes" || v.to_lowercase() == "true")
        .unwrap_or(false);

    // Extract venue -> location
    let location = get_string_value(values, "venue").ok();

    // Build description: kenticoURL\nartists\nworks
    let description = build_description(values);

    Ok(CalendarEvent {
        title,
        description,
        location,
        organization,
        purchased,
        start_date,
        start_time,
        end_date: start_date,
        end_time,
    })
}

fn get_string_value(values: &HashMap<String, serde_json::Value>, key: &str) -> Result<String> {
    values
        .get(key)
        .and_then(|v| {
            match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Null => None,
                other => Some(other.to_string().trim_matches('"').to_string()),
            }
        })
        .filter(|s| !s.is_empty())
        .context(format!("Missing or empty value for '{}'", key))
}

fn build_description(values: &HashMap<String, serde_json::Value>) -> Option<String> {
    let mut parts = Vec::new();

    if let Ok(url) = get_string_value(values, "kenticoUrl") {
        parts.push(url);
    }

    if let Ok(artists) = get_string_value(values, "artists") {
        parts.push(artists);
    }

    if let Ok(works) = get_string_value(values, "works") {
        parts.push(works);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn parse_coda_datetime(s: &str) -> Result<(NaiveDate, Option<NaiveTime>)> {
    // Coda datetime formats can vary. Try common formats:
    // ISO 8601 with timezone: "2024-07-17T19:30:00.000-07:00"
    // ISO 8601: "2024-02-15T19:30:00"
    // Date only: "2024-02-15"
    // US format: "2/15/2024 7:30 PM"
    
    let s = s.trim();

    // Try ISO 8601 with timezone offset (e.g., "2024-07-17T19:30:00.000-07:00")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok((dt.date_naive(), Some(dt.time())));
    }

    // Try ISO 8601 with timezone but without fractional seconds
    if let Ok(dt) = chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%:z") {
        return Ok((dt.date_naive(), Some(dt.time())));
    }

    // Try ISO datetime with T separator (no timezone)
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok((dt.date(), Some(dt.time())));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Ok((dt.date(), Some(dt.time())));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        return Ok((dt.date(), Some(dt.time())));
    }

    // Try date with space and time
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok((dt.date(), Some(dt.time())));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        return Ok((dt.date(), Some(dt.time())));
    }

    // Try US format with AM/PM
    let s_upper = s.to_uppercase();
    if let Ok(dt) = NaiveDateTime::parse_from_str(&s_upper, "%m/%d/%Y %I:%M %p") {
        return Ok((dt.date(), Some(dt.time())));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(&s_upper, "%m/%d/%Y %I:%M:%S %p") {
        return Ok((dt.date(), Some(dt.time())));
    }

    // Try date only formats
    let date_formats = ["%Y-%m-%d", "%m/%d/%Y", "%d/%m/%Y"];
    for fmt in date_formats {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            return Ok((date, None));
        }
    }

    anyhow::bail!("Could not parse datetime: '{}'", s)
}

pub fn get_api_token() -> Result<String> {
    std::env::var("CODA_API_TOKEN")
        .context("CODA_API_TOKEN environment variable not set. Get your token from https://coda.io/account")
}
