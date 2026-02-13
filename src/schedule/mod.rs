pub mod config;
pub mod parse;
pub mod platform;

pub use config::{WakeupConfig, WakeupSchedule};
pub use parse::{format_duration, format_time, parse_duration, parse_time};

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn get_wakeup_config_path() -> Result<PathBuf> {
    let config_dir = dirs::home_dir()
        .map(|p| p.join(".codex-usage"))
        .unwrap_or_else(|| PathBuf::from(".codex-usage"));
    Ok(config_dir.join("wakeup.json"))
}

pub fn get_wakeup_config_path_from_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("wakeup.json")
}

pub fn load_wakeup_config() -> Result<WakeupConfig> {
    let config_path = get_wakeup_config_path()?;

    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: WakeupConfig =
            serde_json::from_str(&content).context("Failed to parse wakeup config")?;
        Ok(config)
    } else {
        Ok(WakeupConfig::new())
    }
}

pub fn load_wakeup_config_with_dir(config_dir: &Path) -> Result<WakeupConfig> {
    let config_path = get_wakeup_config_path_from_dir(config_dir);

    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: WakeupConfig =
            serde_json::from_str(&content).context("Failed to parse wakeup config")?;
        Ok(config)
    } else {
        Ok(WakeupConfig::new())
    }
}

pub fn save_wakeup_config(config: &WakeupConfig) -> Result<()> {
    let config_path = get_wakeup_config_path()?;
    save_wakeup_config_to_path(&config_path, config)
}

pub fn save_wakeup_config_with_dir(config_dir: &Path, config: &WakeupConfig) -> Result<()> {
    let config_path = get_wakeup_config_path_from_dir(config_dir);
    save_wakeup_config_to_path(&config_path, config)
}

fn save_wakeup_config_to_path(config_path: &PathBuf, config: &WakeupConfig) -> Result<()> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }

    let content =
        serde_json::to_string_pretty(config).context("Failed to serialize wakeup config")?;
    fs::write(&config_path, content).context("Failed to write wakeup config")?;
    Ok(())
}

pub fn create_schedule(
    name: &str,
    times: Vec<chrono::NaiveTime>,
    interval: Option<Duration>,
    account: Option<String>,
    wake_system: bool,
) -> Result<WakeupSchedule> {
    let schedule = WakeupSchedule::new(name)
        .with_times(times)
        .with_account(account)
        .with_wake_system(wake_system);

    let schedule = if let Some(i) = interval {
        schedule.with_interval(i)
    } else {
        schedule
    };

    schedule.validate().map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(schedule)
}
