use anyhow::{Context, Result};
use chrono::{NaiveDate, TimeZone, Utc};
use chrono_tz::America::Los_Angeles;
use google_calendar3::api::Event;
use google_calendar3::api::EventDateTime;
use google_calendar3::CalendarHub;
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use std::path::PathBuf;

use crate::event::CalendarEvent;

type Hub = CalendarHub<HttpsConnector<HttpConnector>>;

const CREDENTIALS_FILE: &str = "credentials.json";
const TOKEN_CACHE_FILE: &str = "token_cache.json";

pub async fn create_calendar_hub() -> Result<Hub> {
    let credentials_path = get_credentials_path()?;
    
    let secret = yup_oauth2::read_application_secret(&credentials_path)
        .await
        .with_context(|| {
            format!(
                "Failed to read credentials from {}. \
                Download OAuth 2.0 credentials from Google Cloud Console and save as '{}'",
                credentials_path.display(),
                CREDENTIALS_FILE
            )
        })?;

    let token_cache_path = get_token_cache_path()?;
    
    let auth = yup_oauth2::InstalledFlowAuthenticator::builder(
        secret,
        yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk(&token_cache_path)
    .build()
    .await
    .context("Failed to create authenticator")?;

    let client = hyper::Client::builder().build(
        hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build(),
    );

    Ok(CalendarHub::new(client, auth))
}

pub async fn list_calendars(hub: &Hub) -> Result<()> {
    let (_, calendar_list) = hub
        .calendar_list()
        .list()
        .doit()
        .await
        .context("Failed to list calendars")?;

    println!("\nAvailable Calendars:");
    println!("{:-<60}", "");
    
    if let Some(items) = calendar_list.items {
        for calendar in items {
            let id = calendar.id.unwrap_or_default();
            let summary = calendar.summary.unwrap_or_else(|| "(No name)".to_string());
            let primary = if calendar.primary.unwrap_or(false) { " [PRIMARY]" } else { "" };
            println!("  {} {}", summary, primary);
            println!("    ID: {}", id);
        }
    }

    Ok(())
}

pub async fn create_events(hub: &Hub, calendar_id: &str, events: &[CalendarEvent]) -> Result<()> {
    for event in events {
        let google_event = convert_to_google_event(event);
        
        hub.events()
            .insert(google_event, calendar_id)
            .doit()
            .await
            .with_context(|| format!("Failed to create event: {}", event.title))?;

        tracing::info!("Created event: {}", event.title);
    }

    Ok(())
}

fn convert_to_google_event(event: &CalendarEvent) -> Event {
    let mut google_event = Event::default();
    
    google_event.summary = Some(event.title.clone());
    google_event.description = event.description.clone();
    google_event.location = event.location.clone();

    if event.is_all_day() {
        // All-day event - use date only
        google_event.start = Some(EventDateTime {
            date: Some(event.start_date),
            date_time: None,
            time_zone: None,
        });
        
        // Google Calendar expects end date to be exclusive (day after)
        let end_date = event.end_date.succ_opt().unwrap_or(event.end_date);
        google_event.end = Some(EventDateTime {
            date: Some(end_date),
            date_time: None,
            time_zone: None,
        });
    } else {
        // Timed event - interpret naive datetime as Pacific time, convert to UTC
        let start_dt = event.start_datetime();
        let end_dt = event.end_datetime();

        // Interpret the naive datetime as Pacific time and convert to UTC
        let start_pacific = Los_Angeles.from_local_datetime(&start_dt)
            .single()
            .unwrap_or_else(|| Los_Angeles.from_local_datetime(&start_dt).latest().unwrap());
        let end_pacific = Los_Angeles.from_local_datetime(&end_dt)
            .single()
            .unwrap_or_else(|| Los_Angeles.from_local_datetime(&end_dt).latest().unwrap());

        let start_utc = start_pacific.with_timezone(&Utc);
        let end_utc = end_pacific.with_timezone(&Utc);

        google_event.start = Some(EventDateTime {
            date: None,
            date_time: Some(start_utc),
            time_zone: Some("America/Los_Angeles".to_string()),
        });

        google_event.end = Some(EventDateTime {
            date: None,
            date_time: Some(end_utc),
            time_zone: Some("America/Los_Angeles".to_string()),
        });
    }

    google_event
}

/// Represents a Google Calendar event that was found
#[derive(Debug, Clone)]
pub struct FoundCalendarEvent {
    pub id: String,
    pub title: String,
    pub date: NaiveDate,
    pub location: Option<String>,
}

/// Find Google Calendar events that match the given CalendarEvents (by title and date)
pub async fn find_matching_events(
    hub: &Hub,
    calendar_id: &str,
    events: &[CalendarEvent],
) -> Result<Vec<(CalendarEvent, FoundCalendarEvent)>> {
    if events.is_empty() {
        return Ok(vec![]);
    }

    // Get date range from events
    let min_date = events.iter().map(|e| e.start_date).min().unwrap();
    let max_date = events.iter().map(|e| e.start_date).max().unwrap();

    // Fetch all calendar events in the date range
    let time_min = Utc.from_utc_datetime(&min_date.and_hms_opt(0, 0, 0).unwrap());
    let time_max = Utc.from_utc_datetime(&max_date.succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap());

    let mut all_gcal_events = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut request = hub
            .events()
            .list(calendar_id)
            .time_min(time_min)
            .time_max(time_max)
            .single_events(true)
            .max_results(2500);

        if let Some(token) = &page_token {
            request = request.page_token(token);
        }

        let (_, event_list) = request
            .doit()
            .await
            .context("Failed to list calendar events")?;

        if let Some(items) = event_list.items {
            all_gcal_events.extend(items);
        }

        page_token = event_list.next_page_token;
        if page_token.is_none() {
            break;
        }
    }

    tracing::info!("Found {} events in Google Calendar within date range", all_gcal_events.len());

    // Match Coda events to Google Calendar events by title and date
    let mut matches = Vec::new();

    for coda_event in events {
        for gcal_event in &all_gcal_events {
            let gcal_title = gcal_event.summary.as_deref().unwrap_or("");
            let gcal_date = extract_event_date(gcal_event);

            if let Some(date) = gcal_date {
                // Match by title (case-insensitive) and date
                if gcal_title.to_lowercase() == coda_event.title.to_lowercase() 
                    && date == coda_event.start_date 
                {
                    if let Some(id) = &gcal_event.id {
                        matches.push((
                            coda_event.clone(),
                            FoundCalendarEvent {
                                id: id.clone(),
                                title: gcal_title.to_string(),
                                date,
                                location: gcal_event.location.clone(),
                            },
                        ));
                    }
                }
            }
        }
    }

    Ok(matches)
}

/// Extract the date from a Google Calendar event
fn extract_event_date(event: &Event) -> Option<NaiveDate> {
    if let Some(start) = &event.start {
        // Try date first (all-day events)
        if let Some(date) = start.date {
            return Some(date);
        }
        // Try date_time (timed events)
        if let Some(dt) = &start.date_time {
            return Some(dt.date_naive());
        }
    }
    None
}

/// Delete events from Google Calendar
pub async fn delete_events(
    hub: &Hub,
    calendar_id: &str,
    event_ids: &[String],
) -> Result<usize> {
    let mut deleted = 0;
    for event_id in event_ids {
        hub.events()
            .delete(calendar_id, event_id)
            .doit()
            .await
            .with_context(|| format!("Failed to delete event: {}", event_id))?;
        deleted += 1;
        tracing::info!("Deleted event: {}", event_id);
    }
    Ok(deleted)
}

fn get_credentials_path() -> Result<PathBuf> {
    // Check for env var first, then fall back to current directory
    if let Ok(path) = std::env::var("GOOGLE_CREDENTIALS_PATH") {
        return Ok(PathBuf::from(path));
    }
    
    let path = std::env::current_dir()?.join(CREDENTIALS_FILE);
    Ok(path)
}

fn get_token_cache_path() -> Result<PathBuf> {
    // Check for env var first, then fall back to current directory
    if let Ok(path) = std::env::var("GOOGLE_TOKEN_CACHE_PATH") {
        return Ok(PathBuf::from(path));
    }

    let path = std::env::current_dir()?.join(TOKEN_CACHE_FILE);
    Ok(path)
}
