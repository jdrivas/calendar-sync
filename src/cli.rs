use chrono::NaiveDate;
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

        /// Show statistics (total events, by organization, by venue)
        #[arg(short, long)]
        stats: bool,

        /// Only include events on or after this date (YYYY-MM-DD)
        #[arg(long, value_parser = parse_date)]
        start_date: Option<NaiveDate>,

        /// Only include events on or before this date (YYYY-MM-DD)
        #[arg(long, value_parser = parse_date)]
        end_date: Option<NaiveDate>,

        /// Only include events where Purchased == Yes
        #[arg(short, long)]
        purchased: bool,

        /// Delete matching events from Google Calendar instead of creating them
        #[arg(long)]
        delete: bool,
    },

    /// Import events from a Coda.io table to Google Calendar (use --dry-run to preview)
    CodaImport {
        /// Coda document ID (from the doc URL)
        #[arg(short, long)]
        doc_id: String,

        /// Coda table ID or name
        #[arg(short, long)]
        table_id: String,

        /// Google Calendar ID to add events to (use 'primary' for main calendar)
        #[arg(short, long, default_value = "primary")]
        calendar_id: String,

        /// Preview events without creating them in Google Calendar
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Show statistics (total events, by organization, by venue)
        #[arg(short, long)]
        stats: bool,

        /// Only include events on or after this date (YYYY-MM-DD)
        #[arg(long, value_parser = parse_date)]
        start_date: Option<NaiveDate>,

        /// Only include events on or before this date (YYYY-MM-DD)
        #[arg(long, value_parser = parse_date)]
        end_date: Option<NaiveDate>,

        /// Only include events where Purchased == Yes
        #[arg(short, long)]
        purchased: bool,

        /// Delete matching events from Google Calendar instead of creating them
        #[arg(long)]
        delete: bool,
    },

    /// List tables in a Coda document (helps find table IDs)
    ListCodaTables {
        /// Coda document ID (from the doc URL)
        #[arg(short, long)]
        doc_id: String,
    },

    /// List available calendars
    ListCalendars,

    /// Authenticate with Google Calendar (stores credentials for future use)
    Auth,
}

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| format!("Invalid date format '{}'. Use YYYY-MM-DD", s))
}
