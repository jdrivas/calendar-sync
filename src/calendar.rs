use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
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
        // Timed event - use date_time with UTC DateTime
        let start_dt = event.start_datetime();
        let end_dt = event.end_datetime();

        // Convert NaiveDateTime to DateTime<Utc>
        // Note: This assumes the input times are in UTC. For proper timezone handling,
        // you'd want to use chrono-tz and convert from local time.
        let start_utc = Utc.from_utc_datetime(&start_dt);
        let end_utc = Utc.from_utc_datetime(&end_dt);

        google_event.start = Some(EventDateTime {
            date: None,
            date_time: Some(start_utc),
            time_zone: Some("America/Los_Angeles".to_string()), // TODO: make configurable
        });

        google_event.end = Some(EventDateTime {
            date: None,
            date_time: Some(end_utc),
            time_zone: Some("America/Los_Angeles".to_string()),
        });
    }

    google_event
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
