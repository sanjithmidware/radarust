use anyhow::Context;
use chrono::{Duration, Utc};
use log::{error, info}; // Import log macros
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::Path;
use std::time::Duration as StdDuration;

mod azure;
mod config;
mod search;

use crate::search::IndexManager;

const LOCAL_STORAGE_DIR: &str = "local_storage";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --- INITIALIZE THE LOGGER ---
    // This is the most important step. It enables the logging framework.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Application starting up...");
    let config = config::load_config().context("Application startup failed: could not load configuration")?;
    let polling_interval = Duration::seconds(config.polling.duration_seconds as i64);
    info!("Configuration loaded. Polling interval: {} seconds.", polling_interval.num_seconds());

    let mut last_polled_timestamp = Utc::now() - polling_interval;
    info!("Initial polling window starts at: {}", last_polled_timestamp.to_rfc3339());

    let mut rl = DefaultEditor::new()?;
    info!("Interactive search prompt is ready.");

    loop {
        let poll_start_time = Utc::now();
        let poll_end_time = poll_start_time;
        
        // On subsequent runs, update the timestamp to the last known start time
        if last_polled_timestamp.timestamp() != (poll_start_time - polling_interval).timestamp() {
             last_polled_timestamp = poll_start_time - polling_interval;
        }

        let splits = azure::poll_and_download_splits(
            &config.azure,
            last_polled_timestamp,
            poll_end_time,
        ).await?;

        if splits.is_empty() {
            info!("Poll complete. No new splits found in the current time window.");
        } else {
            info!("\nPoll complete. Successfully downloaded {} new splits. Starting ingestion process.", splits.len());

            let index_manager = IndexManager::create(Path::new(LOCAL_STORAGE_DIR))?;
            let count = index_manager.index_splits(splits)?;
            info!("\nIngestion process complete. Successfully indexed {} new documents.", count);
            info!("The new data is now available for searching.");

            loop {
                let readline = rl.readline("\n>> Enter a search query (or press Enter to continue polling): ");
                match readline {
                    Ok(line) if !line.trim().is_empty() => {
                        rl.add_history_entry(line.as_str())?;
                        if let Err(e) = index_manager.search(&line) {
                            error!("Error during search: {:?}", e);
                        }
                    }
                    Ok(_) => {
                        info!("User requested to continue polling. Pausing search mode.");
                        break;
                    }
                    Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                        info!("Exit signal received. Shutting down poller.");
                        return Ok(());
                    }
                    Err(err) => {
                        error!("Could not read from interactive prompt: {:?}. Exiting search mode.", err);
                        break;
                    }
                }
            }
        }
        
        last_polled_timestamp = poll_start_time;

        info!("Waiting for {} seconds until the next poll cycle starts at {}...", polling_interval.num_seconds(), Utc::now() + polling_interval);
        tokio::time::sleep(StdDuration::from_secs(polling_interval.num_seconds() as u64)).await;
        info!("\n--------------------------------- NEW POLLING CYCLE ---------------------------------\n");
    }
}