use anyhow::Context;
use serde::Deserialize;
use std::fs::File;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub azure: AzureConfig,
    pub polling: PollingConfig,
}

#[derive(Debug, Deserialize)]
pub struct AzureConfig {
    pub account: String,
    pub access_key: String,
    pub container: String,
    pub path_prefix: String,
}

#[derive(Debug, Deserialize)]
pub struct PollingConfig {
    pub duration_seconds: u64,
}

pub fn load_config() -> anyhow::Result<Config> {
    let file = File::open("config.yaml").context("Failed to open config.yaml")?;
    let config: Config =
        serde_yaml::from_reader(file).context("Failed to parse config.yaml")?;
    Ok(config)
}