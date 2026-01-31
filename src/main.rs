mod calendar;
mod cli;
mod coda;
mod csv_parser;
mod event;

use std::collections::HashMap;

use anyhow::Result;
use chrono::NaiveDate;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::cli::{Cli, Commands};
use crate::event::CalendarEvent;

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

fn filter_events(
    events: Vec<CalendarEvent>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    purchased_only: bool,
) -> Vec<CalendarEvent> {
    let mut filtered: Vec<CalendarEvent> = events
        .into_iter()
        .filter(|e| {
            // Filter by start date
            if let Some(sd) = start_date {
                if e.start_date < sd {
                    return false;
                }
            }
            // Filter by end date
            if let Some(ed) = end_date {
                if e.start_date > ed {
                    return false;
                }
            }
            // Filter by purchased
            if purchased_only && !e.purchased {
                return false;
            }
            true
        })
        .collect();
    
    // Sort by date and time
    filtered.sort_by(|a, b| {
        a.start_date.cmp(&b.start_date)
            .then_with(|| a.start_time.cmp(&b.start_time))
    });
    
    filtered
}

fn print_events(events: &[CalendarEvent]) {
    println!("\n{:<40} {:<12} {:<8} {:<12} {:<8} {:<25}", 
        "summary", "start.date", "start", "end.date", "end", "location");
    println!("{}", "-".repeat(105));
    for event in events {
        // Line 1: summary, dates, times, location
        println!("{:<40} {:<12} {:<8} {:<12} {:<8} {:<25}",
            truncate(&event.title, 38),
            event.start_date.format("%Y-%m-%d"),
            event.start_time.map(|t| t.format("%H:%M").to_string()).unwrap_or_else(|| "all-day".to_string()),
            event.end_date.format("%Y-%m-%d"),
            event.end_time.map(|t| t.format("%H:%M").to_string()).unwrap_or_else(|| "all-day".to_string()),
            event.location.as_deref().map(|l| truncate(l, 23)).unwrap_or_default(),
        );
        // Line 2: description (indented)
        if let Some(desc) = &event.description {
            let desc_preview = truncate(desc.replace('\n', " | ").as_str(), 100);
            println!("  description: {}", desc_preview);
        }
    }
}

fn print_stats(events: &[CalendarEvent]) {
    println!("\n{}", "=".repeat(60));
    println!("STATISTICS");
    println!("{}", "=".repeat(60));
    
    // Total events and purchased
    let total_purchased = events.iter().filter(|e| e.purchased).count();
    println!("\nTotal Events: {} ({} purchased)", events.len(), total_purchased);
    
    // Events by venue (total, purchased)
    let mut by_venue: HashMap<String, (usize, usize)> = HashMap::new();
    for event in events {
        let venue = event.location.clone().unwrap_or_else(|| "(No venue)".to_string());
        let entry = by_venue.entry(venue).or_insert((0, 0));
        entry.0 += 1;
        if event.purchased {
            entry.1 += 1;
        }
    }
    
    println!("\nEvents by Venue:");
    println!("{:<6} {:<6} {}", "Total", "Purch", "Venue");
    println!("{:-<50}", "");
    let mut venue_counts: Vec<_> = by_venue.into_iter().collect();
    venue_counts.sort_by(|a, b| b.1.0.cmp(&a.1.0)); // Sort by total count descending
    for (venue, (total, purchased)) in venue_counts {
        println!("  {:>4} {:>6}  {}", total, purchased, venue);
    }
    
    // Events by organization (total, purchased)
    let mut by_org: HashMap<String, (usize, usize)> = HashMap::new();
    for event in events {
        let org = event.organization.clone().unwrap_or_else(|| "(No organization)".to_string());
        let entry = by_org.entry(org).or_insert((0, 0));
        entry.0 += 1;
        if event.purchased {
            entry.1 += 1;
        }
    }
    
    println!("\nEvents by Organization:");
    println!("{:<6} {:<6} {}", "Total", "Purch", "Organization");
    println!("{:-<50}", "");
    let mut org_counts: Vec<_> = by_org.into_iter().collect();
    org_counts.sort_by(|a, b| b.1.0.cmp(&a.1.0)); // Sort by total count descending
    for (org, (total, purchased)) in org_counts {
        println!("  {:>4} {:>6}  {}", total, purchased, org);
    }
    
    println!();
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load .env file if present
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Commands::Import { file, calendar_id, dry_run, stats, start_date, end_date, purchased, delete } => {
            tracing::info!("Importing events from: {}", file.display());
            
            let all_events = csv_parser::parse_csv(&file)?;
            tracing::info!("Parsed {} events", all_events.len());

            let events = filter_events(all_events, start_date, end_date, purchased);
            if start_date.is_some() || end_date.is_some() || purchased {
                tracing::info!("After filtering: {} events", events.len());
            }

            if delete {
                let hub = calendar::create_calendar_hub().await?;
                let matches = calendar::find_matching_events(&hub, &calendar_id, &events).await?;
                
                if dry_run {
                    println!("\n{} events would be DELETED:", matches.len());
                    println!("{}", "=".repeat(80));
                    println!("{:<40} {:<12} {:<30}", "TITLE", "DATE", "GCAL LOCATION");
                    println!("{}", "-".repeat(80));
                    for (_, gcal) in &matches {
                        println!("{:<40} {:<12} {:<30}",
                            truncate(&gcal.title, 38),
                            gcal.date.format("%Y-%m-%d"),
                            gcal.location.as_deref().map(|l| truncate(l, 28)).unwrap_or_default(),
                        );
                    }
                    if stats {
                        print_stats(&events);
                    }
                    println!();
                    return Ok(());
                }

                let event_ids: Vec<String> = matches.iter().map(|(_, g)| g.id.clone()).collect();
                let deleted = calendar::delete_events(&hub, &calendar_id, &event_ids).await?;
                tracing::info!("Successfully deleted {} events", deleted);
                return Ok(());
            }

            if dry_run {
                tracing::info!("Dry run mode - not creating events");
                print_events(&events);
                if stats {
                    print_stats(&events);
                }
                println!();
                return Ok(());
            }

            if stats {
                print_stats(&events);
            }

            let hub = calendar::create_calendar_hub().await?;
            calendar::create_events(&hub, &calendar_id, &events).await?;
            
            tracing::info!("Successfully created {} events", events.len());
        }
        Commands::CodaImport { doc_id, table_id, calendar_id, dry_run, stats, start_date, end_date, purchased, delete } => {
            tracing::info!("Importing events from Coda doc: {}, table: {}", doc_id, table_id);
            
            let api_token = coda::get_api_token()?;
            let client = coda::CodaClient::new(api_token);
            let all_events = client.fetch_events(&doc_id, &table_id).await?;
            tracing::info!("Fetched {} events from Coda", all_events.len());

            let events = filter_events(all_events, start_date, end_date, purchased);
            if start_date.is_some() || end_date.is_some() || purchased {
                tracing::info!("After filtering: {} events", events.len());
            }

            if delete {
                let hub = calendar::create_calendar_hub().await?;
                let matches = calendar::find_matching_events(&hub, &calendar_id, &events).await?;
                
                if dry_run {
                    println!("\n{} events would be DELETED:", matches.len());
                    println!("{}", "=".repeat(80));
                    println!("{:<40} {:<12} {:<30}", "TITLE", "DATE", "GCAL LOCATION");
                    println!("{}", "-".repeat(80));
                    for (_, gcal) in &matches {
                        println!("{:<40} {:<12} {:<30}",
                            truncate(&gcal.title, 38),
                            gcal.date.format("%Y-%m-%d"),
                            gcal.location.as_deref().map(|l| truncate(l, 28)).unwrap_or_default(),
                        );
                    }
                    if stats {
                        print_stats(&events);
                    }
                    println!();
                    return Ok(());
                }

                let event_ids: Vec<String> = matches.iter().map(|(_, g)| g.id.clone()).collect();
                let deleted = calendar::delete_events(&hub, &calendar_id, &event_ids).await?;
                tracing::info!("Successfully deleted {} events", deleted);
                return Ok(());
            }

            if dry_run {
                tracing::info!("Dry run mode - not creating events");
                print_events(&events);
                if stats {
                    print_stats(&events);
                }
                println!();
                return Ok(());
            }

            if stats {
                print_stats(&events);
            }

            let hub = calendar::create_calendar_hub().await?;
            calendar::create_events(&hub, &calendar_id, &events).await?;
            
            tracing::info!("Successfully created {} events", events.len());
        }
        Commands::ListCodaTables { doc_id } => {
            tracing::info!("Listing tables in Coda doc: {}", doc_id);
            
            let api_token = coda::get_api_token()?;
            let client = coda::CodaClient::new(api_token);
            let tables = client.list_tables(&doc_id).await?;

            println!("\nTables in Coda document:");
            println!("{:-<60}", "");
            for table in tables {
                println!("  {} ({})", table.name, table.table_type);
                println!("    ID: {}", table.id);
            }
            println!();
        }
        Commands::ListCalendars => {
            let hub = calendar::create_calendar_hub().await?;
            calendar::list_calendars(&hub).await?;
        }
        Commands::Auth => {
            tracing::info!("Authenticating with Google Calendar...");
            let _hub = calendar::create_calendar_hub().await?;
            tracing::info!("Authentication successful!");
        }
    }

    Ok(())
}
