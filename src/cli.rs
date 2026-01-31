use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "calendar-sync")]
#[command(author, version, about = "Sync calendar events from CSV/Google Sheets to Google Calendar")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Import events from a CSV file to Google Calendar (use --dry-run to preview)
    Import {
        /// Path to the CSV file containing events
        #[arg(short, long)]
        file: PathBuf,

        /// Google Calendar ID to add events to (use 'primary' for main calendar)
        #[arg(short, long, default_value = "primary")]
        calendar_id: String,

        /// Preview events without creating them in Google Calendar
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// List available calendars
    ListCalendars,

    /// Authenticate with Google Calendar (stores credentials for future use)
    Auth,
}
