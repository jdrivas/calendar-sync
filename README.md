# Calendar Sync

A Rust CLI tool to sync calendar events from CSV files (and eventually Google Sheets/Coda.io) to Google Calendar.

## Features

- Import events from CSV files to Google Calendar
- Support for all-day and timed events
- Dry-run mode to preview imports
- List available calendars
- OAuth2 authentication with token caching

## Setup

### 1. Google Cloud Console Setup

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project or select an existing one
3. Enable the **Google Calendar API**:
   - Navigate to "APIs & Services" > "Library"
   - Search for "Google Calendar API" and enable it
4. Create OAuth 2.0 credentials:
   - Go to "APIs & Services" > "Credentials"
   - Click "Create Credentials" > "OAuth client ID"
   - Select "Desktop app" as the application type
   - Download the JSON file and save it as `credentials.json` in the project root

### 2. Build the CLI

```bash
cargo build --release
```

The binary will be at `target/release/calendar_sync`.

### 3. Authenticate

```bash
./target/release/calendar_sync auth
```

This will open a browser for OAuth authentication. After authorizing, tokens are cached locally.

## Usage

### List Available Calendars

```bash
calendar_sync list-calendars
```

### Import Events from CSV

```bash
# Import to primary calendar
calendar_sync import --file events.csv

# Import to a specific calendar
calendar_sync import --file events.csv --calendar-id your-calendar-id@group.calendar.google.com

# Dry run (preview without creating events)
calendar_sync import --file events.csv --dry-run
```

## CSV Format

The CSV file should have the following columns:

| Column | Required | Description |
|--------|----------|-------------|
| `title` | Yes | Event title/summary |
| `description` | No | Event description |
| `location` | No | Event location |
| `start_date` | Yes | Start date (YYYY-MM-DD or MM/DD/YYYY) |
| `start_time` | No | Start time (HH:MM or HH:MM AM/PM) |
| `end_date` | No | End date (defaults to start_date) |
| `end_time` | No | End time |

### Example CSV

```csv
title,description,location,start_date,start_time,end_date,end_time
Team Meeting,Weekly sync,Conference Room A,2024-02-15,10:00,,11:00
Conference,Annual tech conference,Convention Center,2024-03-01,,,2024-03-03
Doctor Appointment,,123 Medical Plaza,2024-02-20,14:30,,15:30
```

- Events with no times are created as all-day events
- Events spanning multiple days without times create multi-day all-day events

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GOOGLE_CREDENTIALS_PATH` | Path to OAuth credentials JSON file |
| `GOOGLE_TOKEN_CACHE_PATH` | Path to store cached auth tokens |
| `RUST_LOG` | Logging level (error, warn, info, debug, trace) |

## Roadmap

- [ ] Google Sheets integration
- [ ] Coda.io integration
- [ ] Event update/sync (not just create)
- [ ] Duplicate detection
- [ ] Configurable timezone
- [ ] Interactive mode

## License

MIT
