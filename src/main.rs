mod calendar;
mod cli;
mod csv_parser;
mod event;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::cli::{Cli, Commands};

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
        Commands::Import { file, calendar_id, dry_run } => {
            tracing::info!("Importing events from: {}", file.display());
            
            let events = csv_parser::parse_csv(&file)?;
            tracing::info!("Parsed {} events", events.len());

            if dry_run {
                tracing::info!("Dry run mode - not creating events");
                for event in &events {
                    println!("{}", event);
                }
                return Ok(());
            }

            let hub = calendar::create_calendar_hub().await?;
            calendar::create_events(&hub, &calendar_id, &events).await?;
            
            tracing::info!("Successfully created {} events", events.len());
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
