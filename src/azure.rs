use crate::config::AzureConfig;
use anyhow::{anyhow, Context};
use azure_storage::StorageCredentials;
use azure_storage_blobs::prelude::*;
use chrono::{DateTime, Utc};
use futures::stream::StreamExt;
use log::{debug, info};

// --- TEMPORARY DEBUGGING CHANGE ---
// This will force the program to stop scanning and proceed after finding just 5 blobs,
// regardless of their timestamp. This is to test the rest of the pipeline.
const DEBUG_DOWNLOAD_LIMIT: usize = 2;

pub struct DownloadedSplit {
    pub name: String,
    pub content: Vec<u8>,
}

pub async fn poll_and_download_splits(
    config: &AzureConfig,
    _start_time: DateTime<Utc>, // We ignore the timestamps for this debug run
    _end_time: DateTime<Utc>,
) -> anyhow::Result<Vec<DownloadedSplit>> {
    let storage_credentials =
        StorageCredentials::access_key(config.account.clone(), config.access_key.clone());
    let blob_service_client =
        BlobServiceClient::new(&config.account, storage_credentials);
    let container_client = blob_service_client.container_client(&config.container);

    info!("!!! DEBUG MODE ACTIVE !!!");
    info!("Scanning for ANY {} blobs to test the download/decompress/index pipeline.", DEBUG_DOWNLOAD_LIMIT);
    info!("Timestamps will be ignored for this run.");

    let mut relevant_splits = Vec::new();
    let mut stream = container_client
        .list_blobs()
        .prefix(config.path_prefix.clone())
        .into_stream();

    let mut blob_scan_count = 0;

    // We use a labeled loop to break out of the nested loops instantly once we hit our limit.
    'outer: while let Some(response) = stream.next().await {
        let response = response.context("Failed to list blobs from Azure")?;

        for blob in response.blobs.blobs() {
            blob_scan_count += 1;
            if blob_scan_count % 1000 == 0 {
                info!("[PROGRESS] Scanned {} blobs so far...", blob_scan_count);
            }

            // --- TEMPORARY DEBUGGING CHANGE ---
            // We are NOT checking the timestamp. We are grabbing the first few blobs we see.
            info!("Found a test blob: '{}'. Adding it to the download queue.", blob.name);
            
            let blob_client = container_client.blob_client(&blob.name);
            
            info!("--> Starting download for '{}'...", blob.name);
            let mut content = Vec::new();
            let mut download_stream = blob_client.get().into_stream();
            while let Some(value) = download_stream.next().await {
                 let data = value.context("Failed to download blob chunk")?.data;
                 let bytes_chunk = data.collect()
                    .await
                    .context("Failed to read downloaded chunk into bytes")?;
                 content.extend_from_slice(&bytes_chunk);
            }
            info!("--> Download complete for '{}'. Size: {} bytes.", blob.name, content.len());

            relevant_splits.push(DownloadedSplit {
                name: blob.name.clone(),
                content,
            });

            // Check if we have reached our debug limit.
            if relevant_splits.len() >= DEBUG_DOWNLOAD_LIMIT {
                info!("Reached debug limit of {}. Stopping scan and proceeding.", DEBUG_DOWNLOAD_LIMIT);
                break 'outer; // Break out of all loops and proceed.
            }
        }
    }

    info!("Finished scanning. Found {} blobs to process.", relevant_splits.len());
    Ok(relevant_splits)
}