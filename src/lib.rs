use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub mod history;

#[cfg(unix)]
use std::process::Command;

#[cfg(feature = "pyo3")]
use pyo3::{prelude::*, types::PyModule, wrap_pyfunction};

#[cfg(feature = "pyo3")]
#[pymodule]
fn codex_usage(m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_py, m)?)?;
    Ok(())
}

#[cfg(feature = "pyo3")]
#[pyfunction]
fn run_py() -> PyResult<String> {
    let result = std::panic::catch_unwind(|| run_cli());

    match result {
        Ok(Ok(())) => Ok("Success".to_string()),
        Ok(Err(e)) => {
            let msg = format!("Error: {}", e);
            eprintln!("{}", msg);
            Err(pyo3::exceptions::PyRuntimeError::new_err(msg))
        }
        Err(e) => {
            let msg = format!("Panic: {:?}", e);
            eprintln!("{}", msg);
            Err(pyo3::exceptions::PyRuntimeError::new_err(msg))
        }
    }
}

#[derive(Parser)]
#[command(name = "codex-usage")]
#[command(about = "Track OpenAI Codex usage with multi-account support", long_about = None)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to config directory (default: ~/.codex-usage)
    #[arg(short, long, env = "CODEX_USAGE_DIR")]
    pub config_dir: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true, env = "CODEX_USAGE_VERBOSE")]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Check usage for active account (or all with --all)
    #[command(alias = "quota")]
    Status {
        /// Check all connected accounts
        #[arg(short, long)]
        all: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Compact one-line output
        #[arg(long)]
        oneline: bool,

        /// Force refresh (skip cache)
        #[arg(short, long)]
        refresh: bool,
    },

    /// Manage accounts
    Accounts {
        #[command(subcommand)]
        command: AccountCommands,
    },

    /// Wakeup Codex to utilize limits
    Wakeup {
        /// Wakeup all accounts
        #[arg(short, long)]
        all: bool,

        /// Configure wakeup schedule
        #[arg(long)]
        config: bool,

        /// Install to system scheduler
        #[arg(long)]
        install: bool,

        /// Uninstall from system scheduler
        #[arg(long)]
        uninstall: bool,
    },

    /// Cycle through accounts when limits exhausted
    Cycle {
        #[command(subcommand)]
        command: CycleCommands,
    },

    /// Track and analyze usage history
    History {
        #[command(subcommand)]
        command: HistoryCommands,
    },
}

#[derive(Subcommand)]
pub enum AccountCommands {
    /// List all connected accounts
    List,

    /// Add current Codex auth as new account
    Add {
        /// Account name/email
        name: String,
    },

    /// Switch to another account
    Switch {
        /// Account name/email to switch to
        name: String,

        /// Force switch even if Codex is running
        #[arg(short, long)]
        force: bool,
    },

    /// Remove an account
    Remove {
        /// Account name/email to remove
        name: String,
    },
}

#[derive(Subcommand)]
pub enum CycleCommands {
    /// Show current cycle status
    Status,

    /// Configure cycle thresholds
    Config {
        /// 5h threshold (remaining % that triggers switch)
        #[arg(long)]
        five_hour: Option<f64>,

        /// Weekly threshold (remaining % that triggers switch)
        #[arg(long)]
        weekly: Option<f64>,

        /// Mode: and (both) or or (either)
        #[arg(long)]
        mode: Option<String>,
    },

    /// Enable cycling
    Enable,

    /// Disable cycling
    Disable,

    /// Manually trigger cycle check
    Now {
        /// Force switch even if Codex is running
        #[arg(short, long)]
        force: bool,
    },

    /// Show cycle history
    History,

    /// Reorder accounts in cycle
    Reorder {
        /// Accounts in new order
        accounts: Vec<String>,
    },

    /// Manage schedule
    Schedule {
        #[command(subcommand)]
        command: ScheduleCommands,
    },
}

#[derive(Subcommand)]
pub enum ScheduleCommands {
    /// Enable scheduled cycling
    Enable {
        /// Check interval in minutes
        #[arg(long, default_value = "60")]
        interval: u32,
    },

    /// Disable scheduled cycling
    Disable,
}

#[derive(Subcommand)]
pub enum HistoryCommands {
    /// Manage background recording daemon
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },

    /// Show usage history
    Show {
        /// Time period (day, week, month)
        #[arg(long)]
        period: Option<String>,

        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// End date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,

        /// Account name
        #[arg(long)]
        account: Option<String>,
    },

    /// Show terminal bar chart visualization
    Chart {
        /// Account names (default: all accounts)
        accounts: Vec<String>,
    },

    /// Show allowance tracking and analysis
    Allowance {
        /// Show projected usage
        #[arg(long)]
        projected: bool,

        /// Show dead time analysis
        #[arg(long)]
        dead_time: bool,

        /// Account name
        #[arg(long)]
        account: Option<String>,
    },

    /// Configure notifications
    Notify {
        /// Enable notifications
        #[arg(long)]
        enable: bool,

        /// Disable notifications
        #[arg(long)]
        disable: bool,

        /// Hours before reset to notify
        #[arg(long)]
        hours_before: Option<i32>,

        /// Show notification status
        #[arg(long)]
        status: bool,

        /// Account name
        #[arg(long)]
        account: Option<String>,
    },

    /// Export history data
    Export {
        /// Output file path
        #[arg(long)]
        output: Option<String>,

        /// Export format (json)
        #[arg(long, default_value = "json")]
        format: String,

        /// Time period (day, week, month)
        #[arg(long)]
        period: Option<String>,

        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// End date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum DaemonCommands {
    /// Start the background daemon
    Start {
        /// Poll interval (e.g., 5m, 10m)
        #[arg(long, default_value = "5m")]
        interval: String,
    },

    /// Stop the background daemon
    Stop,

    /// Show daemon status
    Status,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub active_account: Option<String>,
    pub accounts: HashMap<String, AccountInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountInfo {
    pub added_at: String,
    pub last_used: Option<String>,
    pub auth_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CycleConfig {
    pub enabled: bool,
    pub thresholds: CycleThresholds,
    pub mode: String,
    pub accounts: Vec<String>,
    pub current_index: usize,
    pub last_cycle: Option<String>,
}

impl Default for CycleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            thresholds: CycleThresholds::default(),
            mode: "and".to_string(),
            accounts: Vec::new(),
            current_index: 0,
            last_cycle: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CycleThresholds {
    pub five_hour: f64,
    pub weekly: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CycleHistoryEntry {
    pub timestamp: String,
    pub from_account: String,
    pub to_account: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CodexAuth {
    #[serde(rename = "OPENAI_API_KEY")]
    #[allow(dead_code)]
    pub api_key: Option<String>,
    pub tokens: Option<CodexTokens>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CodexTokens {
    pub access_token: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsageData {
    pub account_name: String,
    pub status: String,
    pub plan: Option<String>,
    pub primary_window: Option<RateWindow>,
    pub secondary_window: Option<RateWindow>,
    pub code_review: Option<CodeReview>,
    pub limit_reached: bool,
    pub auth_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateWindow {
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub window: String,
    pub resets_in: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodeReview {
    pub used_percent: f64,
}

const USAGE_API_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const CACHE_TTL_SECS: u64 = 300;

pub fn get_config_dir() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".codex-usage"))
        .unwrap_or_else(|| PathBuf::from(".codex-usage"))
}

fn get_codex_dir() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".codex"))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

pub fn get_codex_auth_path() -> PathBuf {
    get_codex_dir().join("auth.json")
}

pub fn get_accounts_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("accounts")
}

pub fn get_account_auth_path(config_dir: &Path, name: &str) -> PathBuf {
    let sanitized = sanitize_account_name(name);
    get_accounts_dir(config_dir)
        .join(&sanitized)
        .join("auth.json")
}

fn sanitize_account_name(name: &str) -> String {
    let sanitized = name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");
    if sanitized.contains("..") || sanitized.starts_with('/') || sanitized.starts_with('\\') {
        panic!("Invalid account name: {}", name);
    }
    sanitized
}

pub fn get_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("config.json")
}

pub fn get_cache_path(config_dir: &Path, account_name: &str) -> PathBuf {
    let sanitized =
        account_name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");
    config_dir.join(format!("usage_cache_{}.json", sanitized))
}

pub fn get_cycle_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("cycle.json")
}

pub fn get_cycle_history_path(config_dir: &Path) -> PathBuf {
    config_dir.join("cycle_history.jsonl")
}

pub fn load_config(config_dir: &Path) -> Result<Config> {
    let config_path = get_config_path(config_dir);
    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&content).context("Failed to parse config")?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

pub fn save_config(config_dir: &Path, config: &Config) -> Result<()> {
    let config_path = get_config_path(config_dir);
    let content = serde_json::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(&config_path, content).context("Failed to write config")?;
    Ok(())
}

pub fn load_cycle_config(config_dir: &Path) -> Result<CycleConfig> {
    let path = get_cycle_config_path(config_dir);
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let config: CycleConfig =
            serde_json::from_str(&content).context("Failed to parse cycle config")?;
        Ok(config)
    } else {
        Ok(CycleConfig::default())
    }
}

pub fn save_cycle_config(config_dir: &Path, config: &CycleConfig) -> Result<()> {
    let path = get_cycle_config_path(config_dir);
    let content =
        serde_json::to_string_pretty(config).context("Failed to serialize cycle config")?;
    fs::write(&path, content).context("Failed to write cycle config")?;
    Ok(())
}

pub fn load_codex_auth(path: &Path) -> Result<Option<CodexAuth>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let auth: CodexAuth = serde_json::from_str(&content).context("Failed to parse auth.json")?;
    Ok(Some(auth))
}

fn is_codex_running() -> bool {
    #[cfg(unix)]
    {
        let current_pid = std::process::id();
        let output = Command::new("pgrep").arg("-f").arg("codex ").output();
        if let Ok(output) = output {
            if output.status.success() {
                let pids = String::from_utf8_lossy(&output.stdout);
                for line in pids.lines() {
                    if let Ok(pid) = line.trim().parse::<u32>() {
                        if pid != current_pid {
                            return true;
                        }
                    }
                }
            }
        }
    }

    let lock_path = get_codex_dir().join(".codex.lock");
    if lock_path.exists() {
        if let Ok(content) = fs::read_to_string(&lock_path) {
            let pid: u32 = content.trim().parse().unwrap_or(0);
            if pid > 0 {
                #[cfg(unix)]
                {
                    return Command::new("kill")
                        .arg("-0")
                        .arg(pid.to_string())
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                }
                #[cfg(windows)]
                {
                    use sysinfo::System;
                    let mut sys = System::new();
                    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                    if let Some(process) = sys.process(sysinfo::Pid::from_u32(pid)) {
                        return process.status() != sysinfo::ProcessStatus::Run;
                    }
                    return false;
                }
            }
        }
        return true;
    }

    false
}

fn warn_codex_running() {
    eprintln!("Warning: Codex appears to be running!");
    eprintln!("Use --force to switch anyway (this may disrupt active sessions)");
}

fn copy_auth_file(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        anyhow::bail!("Source auth file not found: {:?}", from);
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).context("Failed to create parent directory")?;
    }
    fs::copy(from, to).context("Failed to copy auth file")?;
    Ok(())
}

pub fn cmd_accounts_list(config_dir: &Path) -> Result<()> {
    let config = load_config(config_dir)?;
    if config.accounts.is_empty() {
        println!("No accounts configured. Run 'codex-usage accounts add <name>' to add one.");
        return Ok(());
    }

    println!("Configured accounts:");
    println!();

    for (name, info) in &config.accounts {
        let active_marker = config
            .active_account
            .as_ref()
            .filter(|a| *a == name)
            .map(|_| " (active)")
            .unwrap_or("");

        println!("  - {}{}", name, active_marker);
        println!("    Added: {}", info.added_at);
        if let Some(last_used) = &info.last_used {
            println!("    Last used: {}", last_used);
        }
    }

    Ok(())
}

pub fn cmd_accounts_add(config_dir: &Path, name: &str) -> Result<()> {
    let codex_auth = get_codex_auth_path();
    if !codex_auth.exists() {
        anyhow::bail!(
            "No Codex auth found. Please run 'codex login' first to authenticate with Codex."
        );
    }

    let auth_content = fs::read_to_string(&codex_auth)?;
    let auth_digest = Sha256::digest(auth_content.as_bytes());
    let auth_hash = format!("{:x}", auth_digest);

    let mut config = load_config(config_dir)?;

    for (existing_name, info) in &config.accounts {
        if let Some(existing_hash) = &info.auth_hash {
            if let Ok(existing_digest) = hex::decode(existing_hash) {
                use std::cmp::Ordering;
                if auth_digest.as_slice().cmp(&existing_digest) == Ordering::Equal {
                    anyhow::bail!(
                        "This account has already been added as '{}'. Use 'codex-usage accounts switch {}' to switch to it.",
                        existing_name, existing_name
                    );
                }
            }
        }
    }

    let account_auth_path = get_account_auth_path(config_dir, name);
    let accounts_dir = get_accounts_dir(config_dir);
    fs::create_dir_all(&accounts_dir).context("Failed to create accounts directory")?;
    copy_auth_file(&codex_auth, &account_auth_path)?;

    config.accounts.insert(
        name.to_string(),
        AccountInfo {
            added_at: chrono::Utc::now().to_rfc3339(),
            last_used: None,
            auth_hash: Some(auth_hash),
        },
    );
    save_config(config_dir, &config)?;

    println!("Added account '{}' successfully.", name);
    println!("Auth file saved to: {:?}", account_auth_path);
    Ok(())
}

pub fn cmd_accounts_switch(config_dir: &Path, name: &str, force: bool) -> Result<()> {
    if is_codex_running() {
        warn_codex_running();
        if !force {
            anyhow::bail!("Aborted. Use --force to switch anyway.");
        }
    }

    let account_auth_path = get_account_auth_path(config_dir, name);
    if !account_auth_path.exists() {
        anyhow::bail!(
            "Account '{}' not found. Run 'codex-usage accounts list' to see available accounts.",
            name
        );
    }

    let codex_auth = get_codex_auth_path();
    if codex_auth.exists() {
        let backup_path = codex_auth.with_extension("json.backup");
        fs::copy(&codex_auth, &backup_path).ok();
    }
    copy_auth_file(&account_auth_path, &codex_auth)?;

    let mut config = load_config(config_dir)?;
    config.active_account = Some(name.to_string());
    if let Some(account_info) = config.accounts.get_mut(name) {
        account_info.last_used = Some(chrono::Utc::now().to_rfc3339());
    }
    save_config(config_dir, &config)?;

    println!("Switched to account '{}' successfully.", name);
    Ok(())
}

pub fn cmd_accounts_remove(config_dir: &Path, name: &str) -> Result<()> {
    let account_auth_path = get_account_auth_path(config_dir, name);
    if !account_auth_path.exists() {
        anyhow::bail!("Account '{}' not found.", name);
    }

    if let Some(parent) = account_auth_path.parent() {
        fs::remove_dir_all(parent).context("Failed to remove account directory")?;
    }

    let mut config = load_config(config_dir)?;
    config.accounts.remove(name);
    if config.active_account.as_deref() == Some(name) {
        config.active_account = None;
    }
    save_config(config_dir, &config)?;

    println!("Removed account '{}' successfully.", name);
    Ok(())
}

fn format_reset_time(seconds: u64) -> String {
    let hours = seconds / 3600;
    let remainder = seconds % 3600;
    let minutes = remainder / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

fn parse_usage_response(data: serde_json::Value, account_name: &str) -> UsageData {
    let mut usage = UsageData {
        account_name: account_name.to_string(),
        status: "ok".to_string(),
        plan: None,
        primary_window: None,
        secondary_window: None,
        code_review: None,
        limit_reached: false,
        auth_type: "OAuth (ChatGPT)".to_string(),
    };

    if let Some(plan) = data.get("plan_type").and_then(|v| v.as_str()) {
        usage.plan = Some(plan.to_string());
    }

    if let Some(rate_limit) = data.get("rate_limit") {
        if let Some(primary) = rate_limit.get("primary_window") {
            let window_seconds = primary
                .get("limit_window_seconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(18000);
            let window_hours = window_seconds / 3600;
            let used_percent = primary
                .get("used_percent")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let remaining_percent = 100.0 - used_percent;
            let reset_secs = primary
                .get("reset_after_seconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            usage.primary_window = Some(RateWindow {
                used_percent,
                remaining_percent,
                window: format!("{}h", window_hours),
                resets_in: if reset_secs > 0 {
                    Some(format_reset_time(reset_secs))
                } else {
                    None
                },
            });
        }

        if let Some(secondary) = rate_limit.get("secondary_window") {
            let window_seconds = secondary
                .get("limit_window_seconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(604800);
            let window_days = window_seconds / 86400;
            let used_percent = secondary
                .get("used_percent")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let remaining_percent = 100.0 - used_percent;
            let reset_secs = secondary
                .get("reset_after_seconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            usage.secondary_window = Some(RateWindow {
                used_percent,
                remaining_percent,
                window: format!("{}d", window_days),
                resets_in: if reset_secs > 0 {
                    Some(format_reset_time(reset_secs))
                } else {
                    None
                },
            });
        }

        if let Some(limit_reached) = rate_limit.get("limit_reached").and_then(|v| v.as_bool()) {
            usage.limit_reached = limit_reached;
        }
    }

    if let Some(review_limit) = data.get("code_review_rate_limit") {
        if let Some(primary) = review_limit.get("primary_window") {
            let used_percent = primary
                .get("used_percent")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            usage.code_review = Some(CodeReview { used_percent });
        }
    }

    usage
}

fn fetch_usage(
    client: &reqwest::blocking::Client,
    access_token: &str,
    account_id: &str,
    account_name: &str,
) -> Result<UsageData> {
    let response = client
        .get(USAGE_API_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("chatgpt-account-id", account_id)
        .header("User-Agent", "codex-cli")
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .context("Failed to fetch usage")?;

    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("API returned error: {}", status);
    }

    let data: serde_json::Value = response.json().context("Failed to parse response")?;
    Ok(parse_usage_response(data, account_name))
}

fn get_cached_usage(config_dir: &Path, account_name: &str) -> Option<UsageData> {
    let cache_path = get_cache_path(config_dir, account_name);
    if !cache_path.exists() {
        return None;
    }

    let content = match fs::read_to_string(&cache_path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let cached: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let timestamp = cached.get("timestamp")?.as_f64()?;
    let data = cached.get("data")?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let elapsed = now - timestamp;
    if elapsed > CACHE_TTL_SECS as f64 {
        return None;
    }

    serde_json::from_value(data.clone()).ok()
}

fn save_cache(config_dir: &Path, usage: &UsageData, account_name: &str) -> Result<()> {
    let cache_path = get_cache_path(config_dir, account_name);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let cache_data = serde_json::json!({
        "timestamp": timestamp,
        "data": usage
    });
    let content = serde_json::to_string_pretty(&cache_data).context("Failed to serialize cache")?;
    fs::write(&cache_path, content).context("Failed to write cache")?;
    Ok(())
}

fn get_status_icon(percent: f64) -> &'static str {
    if percent >= 100.0 {
        "❌"
    } else if percent >= 90.0 {
        "🔴"
    } else if percent >= 70.0 {
        "⚠️"
    } else {
        "✅"
    }
}

pub fn cmd_status(
    config_dir: &Path,
    all: bool,
    json: bool,
    oneline: bool,
    refresh: bool,
) -> Result<()> {
    let config = load_config(config_dir)?;

    let accounts_to_check: Vec<String> = if all {
        config.accounts.keys().cloned().collect()
    } else {
        vec![config
            .active_account
            .clone()
            .unwrap_or_else(|| "default".to_string())]
    };

    if accounts_to_check.is_empty()
        || (accounts_to_check.len() == 1 && accounts_to_check[0] == "default")
    {
        let codex_auth_path = get_codex_auth_path();
        if codex_auth_path.exists() {
            let auth = load_codex_auth(&codex_auth_path)?;
            if let Some(auth) = auth {
                if let Some(tokens) = auth.tokens {
                    if let (Some(access_token), Some(account_id)) =
                        (&tokens.access_token, &tokens.account_id)
                    {
                        if !refresh {
                            let default_account = config
                                .active_account
                                .clone()
                                .unwrap_or_else(|| "default".to_string());
                            if let Some(cached) = get_cached_usage(config_dir, &default_account) {
                                if json {
                                    println!("{}", serde_json::to_string_pretty(&cached)?);
                                } else if oneline {
                                    print_oneline(&cached);
                                } else {
                                    print_usage(&cached, true);
                                }
                                return Ok(());
                            }
                        }

                        let client = reqwest::blocking::Client::new();
                        let default_account = config
                            .active_account
                            .clone()
                            .unwrap_or_else(|| "default".to_string());
                        match fetch_usage(&client, access_token, account_id, &default_account) {
                            Ok(usage) => {
                                let _ = save_cache(config_dir, &usage, &default_account);
                                if json {
                                    println!("{}", serde_json::to_string_pretty(&usage)?);
                                } else if oneline {
                                    print_oneline(&usage);
                                } else {
                                    print_usage(&usage, true);
                                }
                                return Ok(());
                            }
                            Err(e) => {
                                anyhow::bail!("Failed to fetch usage: {}", e);
                            }
                        }
                    }
                }
            }
        }
        anyhow::bail!(
            "No active account. Run 'codex login' or use 'codex-usage accounts add' first."
        );
    }

    let mut all_usages: Vec<UsageData> = Vec::new();
    let client = reqwest::blocking::Client::new();

    for account_name in &accounts_to_check {
        let account_auth_path = get_account_auth_path(config_dir, account_name);
        let auth = load_codex_auth(&account_auth_path)?;

        if let Some(auth) = auth {
            if let Some(tokens) = auth.tokens {
                if let (Some(access_token), Some(account_id)) =
                    (&tokens.access_token, &tokens.account_id)
                {
                    if !refresh {
                        if let Some(cached) = get_cached_usage(config_dir, account_name) {
                            if cached.account_name == *account_name {
                                all_usages.push(cached);
                                continue;
                            }
                        }
                    }

                    match fetch_usage(&client, access_token, account_id, account_name) {
                        Ok(mut usage) => {
                            usage.account_name = account_name.clone();
                            let _ = save_cache(config_dir, &usage, account_name);
                            all_usages.push(usage);
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to fetch usage for {}: {}", account_name, e);
                        }
                    }
                }
            }
        }
    }

    if all_usages.is_empty() {
        anyhow::bail!("No usage data available for any account.");
    }

    let usage_map: HashMap<String, UsageData> = all_usages
        .into_iter()
        .map(|u| (u.account_name.clone(), u))
        .collect();

    if json {
        if usage_map.len() == 1 {
            println!(
                "{}",
                serde_json::to_string_pretty(&usage_map.values().next())?
            );
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(&usage_map.values().collect::<Vec<_>>())?
            );
        }
    } else if oneline {
        for usage in usage_map.values() {
            print_oneline(usage);
        }
    } else {
        for account_name in &accounts_to_check {
            let is_current = config.active_account.as_deref() == Some(account_name.as_str());
            if let Some(usage) = usage_map.get(account_name) {
                print_usage(usage, is_current);
                println!();
            }
        }
    }

    Ok(())
}

fn print_usage(usage: &UsageData, is_current: bool) {
    let current_marker = if is_current { " *" } else { "" };
    println!("{}", "=".repeat(50));
    println!("  {}{}", usage.account_name, current_marker);
    println!("{}", "=".repeat(50));

    println!("  🔑 Auth: {}", usage.auth_type);
    if let Some(plan) = &usage.plan {
        println!("  📊 Plan: {}", plan);
    }

    if usage.status == "ok" {
        println!("  ✅ Connected");
    } else {
        println!("  ❌ Error: {}", usage.status);
    }

    if let Some(pw) = &usage.primary_window {
        println!();
        println!("  {} Window:", pw.window);
        println!(
            "    Used:      {:.1}% {}",
            pw.used_percent,
            get_status_icon(pw.used_percent)
        );
        println!("    Remaining: {:.1}%", pw.remaining_percent);
        if let Some(reset) = &pw.resets_in {
            println!("    Resets in: {}", reset);
        }
    }

    if let Some(sw) = &usage.secondary_window {
        println!();
        println!("  {} Window:", sw.window);
        println!(
            "    Used:      {:.1}% {}",
            sw.used_percent,
            get_status_icon(sw.used_percent)
        );
        println!("    Remaining: {:.1}%", sw.remaining_percent);
        if let Some(reset) = &sw.resets_in {
            println!("    Resets in: {}", reset);
        }
    }

    if let Some(cr) = &usage.code_review {
        println!();
        println!("  Code Review: {:.1}% used", cr.used_percent);
    }

    if usage.limit_reached {
        println!();
        println!("  ⚠️  Rate limit reached!");
    }
}

fn print_oneline(usage: &UsageData) {
    let mut parts = Vec::new();

    if let Some(pw) = &usage.primary_window {
        parts.push(format!(
            "{:.0}% ({}) {}",
            pw.used_percent,
            pw.window,
            get_status_icon(pw.used_percent)
        ));
    }

    if let Some(sw) = &usage.secondary_window {
        parts.push(format!("{:.0}% ({})", sw.used_percent, sw.window));
    }

    if parts.is_empty() {
        println!("{}: No data", usage.account_name);
    } else {
        println!("{}: {}", usage.account_name, parts.join(" / "));
    }
}

pub fn cmd_cycle_status(config_dir: &Path) -> Result<()> {
    let cycle_config = load_cycle_config(config_dir)?;
    let config = load_config(config_dir)?;

    println!("{}", "=".repeat(50));
    println!("  Cycle Status");
    println!("{}", "=".repeat(50));

    if cycle_config.enabled {
        println!("  ✅ Cycling enabled");
    } else {
        println!("  ❌ Cycling disabled");
    }

    println!();
    println!("  Thresholds:");
    println!(
        "    5h:    <= {:.0}% remaining",
        cycle_config.thresholds.five_hour
    );
    println!(
        "    Weekly: <= {:.0}% remaining",
        cycle_config.thresholds.weekly
    );
    println!("    Mode:   {}", cycle_config.mode);

    println!();
    println!("  Accounts in cycle:");
    if cycle_config.accounts.is_empty() {
        println!("    (none - will use all configured accounts)");
        for name in config.accounts.keys() {
            let marker = if Some(name.as_str()) == config.active_account.as_deref() {
                " (current)"
            } else {
                ""
            };
            println!("    {}{}", name, marker);
        }
    } else {
        for (i, name) in cycle_config.accounts.iter().enumerate() {
            let marker = if i == cycle_config.current_index {
                " (next)"
            } else if Some(name.as_str()) == config.active_account.as_deref() {
                " (current)"
            } else {
                ""
            };
            println!("    {}. {}{}", i + 1, name, marker);
        }
    }

    if let Some(last_cycle) = &cycle_config.last_cycle {
        println!();
        println!("  Last cycle: {}", last_cycle);
    }

    Ok(())
}

pub fn cmd_cycle_config(
    config_dir: &Path,
    five_hour: Option<f64>,
    weekly: Option<f64>,
    mode: Option<String>,
) -> Result<()> {
    let mut cycle_config = load_cycle_config(config_dir)?;

    if let Some(fh) = five_hour {
        cycle_config.thresholds.five_hour = fh;
    }
    if let Some(w) = weekly {
        cycle_config.thresholds.weekly = w;
    }
    if let Some(m) = mode {
        if m != "and" && m != "or" {
            anyhow::bail!("Mode must be 'and' or 'or'");
        }
        cycle_config.mode = m;
    }

    save_cycle_config(config_dir, &cycle_config)?;

    println!("Cycle configuration updated:");
    println!("  5h threshold:  {:.0}%", cycle_config.thresholds.five_hour);
    println!("  Weekly threshold: {:.0}%", cycle_config.thresholds.weekly);
    println!("  Mode: {}", cycle_config.mode);

    Ok(())
}

pub fn cmd_cycle_enable(config_dir: &Path) -> Result<()> {
    let mut cycle_config = load_cycle_config(config_dir)?;
    cycle_config.enabled = true;
    save_cycle_config(config_dir, &cycle_config)?;
    println!("Cycling enabled.");
    Ok(())
}

pub fn cmd_cycle_disable(config_dir: &Path) -> Result<()> {
    let mut cycle_config = load_cycle_config(config_dir)?;
    cycle_config.enabled = false;
    save_cycle_config(config_dir, &cycle_config)?;
    println!("Cycling disabled.");
    Ok(())
}

fn should_cycle(usage: &UsageData, config: &CycleConfig) -> (bool, String) {
    let five_hour_remaining = usage
        .primary_window
        .as_ref()
        .map(|w| w.remaining_percent)
        .unwrap_or(100.0);

    let weekly_remaining = usage
        .secondary_window
        .as_ref()
        .map(|w| w.remaining_percent)
        .unwrap_or(100.0);

    let five_hour_trigger = five_hour_remaining <= config.thresholds.five_hour;
    let weekly_trigger = weekly_remaining <= config.thresholds.weekly;

    let reason = if config.mode == "and" {
        if five_hour_trigger && weekly_trigger {
            let parts = [
                format!("5h: {:.0}% remaining", five_hour_remaining),
                format!("weekly: {:.0}% remaining", weekly_remaining),
            ];
            (true, parts.join(", "))
        } else {
            (
                false,
                format!(
                    "5h: {:.0}%, weekly: {:.0}%",
                    five_hour_remaining, weekly_remaining
                ),
            )
        }
    } else if five_hour_trigger {
        (true, format!("5h: {:.0}% remaining", five_hour_remaining))
    } else if weekly_trigger {
        (true, format!("weekly: {:.0}% remaining", weekly_remaining))
    } else {
        (
            false,
            format!(
                "5h: {:.0}%, weekly: {:.0}%",
                five_hour_remaining, weekly_remaining
            ),
        )
    };

    reason
}

pub fn cmd_cycle_now(config_dir: &Path, force: bool) -> Result<()> {
    let cycle_config = load_cycle_config(config_dir)?;
    let config = load_config(config_dir)?;

    if !cycle_config.enabled {
        println!("Cycling is disabled. Use 'codex-usage cycle enable' to enable.");
        return Ok(());
    }

    let accounts: Vec<String> = if cycle_config.accounts.is_empty() {
        config.accounts.keys().cloned().collect()
    } else {
        cycle_config.accounts.clone()
    };

    if accounts.is_empty() {
        anyhow::bail!("No accounts configured. Add accounts first.");
    }

    let current = config.active_account.as_deref();

    if current.is_none() {
        anyhow::bail!("No active account set. Use 'codex-usage accounts switch <name>' to set an active account.");
    }

    let current = current.unwrap();

    let current_idx = accounts
        .iter()
        .position(|a| a.as_str() == current)
        .unwrap_or(0);

    let next_idx = (current_idx + 1) % accounts.len();
    let next_account = &accounts[next_idx];

    let current_account_auth_path = get_account_auth_path(config_dir, current);
    let current_auth = load_codex_auth(&current_account_auth_path)?;

    let client = reqwest::blocking::Client::new();

    if let Some(auth) = current_auth {
        if let Some(tokens) = auth.tokens {
            if let (Some(access_token), Some(account_id)) =
                (&tokens.access_token, &tokens.account_id)
            {
                let usage = fetch_usage(&client, access_token, account_id, current)?;

                let (should_switch, reason) = should_cycle(&usage, &cycle_config);

                if should_switch {
                    if is_codex_running() {
                        warn_codex_running();
                        if !force {
                            anyhow::bail!("Aborted. Use --force to switch anyway.");
                        }
                    }

                    let codex_auth = get_codex_auth_path();
                    if codex_auth.exists() {
                        let backup_path = codex_auth.with_extension("json.backup");
                        fs::copy(&codex_auth, &backup_path).ok();
                    }
                    let next_account_auth_path = get_account_auth_path(config_dir, next_account);
                    copy_auth_file(&next_account_auth_path, &codex_auth)?;

                    let mut updated_config = load_config(config_dir)?;
                    updated_config.active_account = Some(next_account.clone());
                    save_config(config_dir, &updated_config)?;

                    let mut updated_cycle = load_cycle_config(config_dir)?;
                    updated_cycle.current_index = next_idx;
                    updated_cycle.last_cycle = Some(chrono::Utc::now().to_rfc3339());
                    save_cycle_config(config_dir, &updated_cycle)?;

                    println!(
                        "Cycled from '{}' to '{}' (reason: {})",
                        current, next_account, reason
                    );

                    let history_entry = CycleHistoryEntry {
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        from_account: current.to_string(),
                        to_account: next_account.clone(),
                        reason,
                    };

                    let history_path = get_cycle_history_path(config_dir);
                    let line = serde_json::to_string(&history_entry)?;
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&history_path)?;
                    use std::io::Write;
                    writeln!(file, "{}", line)?;
                } else {
                    println!("No cycle needed (thresholds not met: {})", reason);
                }
            } else {
                anyhow::bail!("Missing access_token or account_id for current account. Cannot fetch usage for cycling.");
            }
        } else {
            anyhow::bail!(
                "No tokens found in current account auth. Cannot fetch usage for cycling."
            );
        }
    } else {
        anyhow::bail!("No auth data found for current account. Cannot fetch usage for cycling.");
    }

    Ok(())
}

pub fn cmd_cycle_history(config_dir: &Path) -> Result<()> {
    let history_path = get_cycle_history_path(config_dir);

    if !history_path.exists() {
        println!("No cycle history found.");
        return Ok(());
    }

    let content = fs::read_to_string(&history_path)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        println!("No cycle history found.");
        return Ok(());
    }

    println!("Cycle History:");
    println!();

    for line in lines.iter().rev().take(20) {
        if let Ok(entry) = serde_json::from_str::<CycleHistoryEntry>(line) {
            println!(
                "  {}: {} -> {} ({})",
                entry.timestamp, entry.from_account, entry.to_account, entry.reason
            );
        }
    }

    Ok(())
}

pub fn cmd_cycle_reorder(config_dir: &Path, accounts: Vec<String>) -> Result<()> {
    let config = load_config(config_dir)?;

    for name in &accounts {
        if !config.accounts.contains_key(name) {
            anyhow::bail!("Account '{}' not found. Use 'codex-usage accounts list' to see available accounts.", name);
        }
    }

    let mut cycle_config = load_cycle_config(config_dir)?;
    cycle_config.accounts = accounts.clone();

    let current = config.active_account.as_deref();
    if let Some(c) = current {
        if let Some(idx) = accounts.iter().position(|a| a.as_str() == c) {
            cycle_config.current_index = idx;
        }
    }

    save_cycle_config(config_dir, &cycle_config)?;

    println!("Cycle accounts reordered:");
    for (i, name) in accounts.iter().enumerate() {
        println!("  {}. {}", i + 1, name);
    }

    Ok(())
}

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let config_dir = cli.config_dir.unwrap_or_else(get_config_dir);

    tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .try_init()
        .ok();

    tracing::debug!("Config directory: {:?}", config_dir);

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
        tracing::info!("Created config directory: {:?}", config_dir);
    }

    match cli.command {
        Commands::Status {
            all,
            json,
            oneline,
            refresh,
        } => {
            cmd_status(&config_dir, all, json, oneline, refresh)?;
        }
        Commands::Accounts { command } => match command {
            AccountCommands::List => {
                cmd_accounts_list(&config_dir)?;
            }
            AccountCommands::Add { name } => {
                cmd_accounts_add(&config_dir, &name)?;
            }
            AccountCommands::Switch { name, force } => {
                cmd_accounts_switch(&config_dir, &name, force)?;
            }
            AccountCommands::Remove { name } => {
                cmd_accounts_remove(&config_dir, &name)?;
            }
        },
        Commands::Wakeup {
            all,
            config,
            install,
            uninstall,
        } => {
            tracing::debug!(
                "Wakeup command: all={}, config={}, install={}, uninstall={}",
                all,
                config,
                install,
                uninstall
            );
            println!("codex-usage wakeup - use --all to wakeup all accounts");
        }
        Commands::Cycle { command } => match command {
            CycleCommands::Status => {
                cmd_cycle_status(&config_dir)?;
            }
            CycleCommands::Config {
                five_hour,
                weekly,
                mode,
            } => {
                cmd_cycle_config(&config_dir, five_hour, weekly, mode)?;
            }
            CycleCommands::Enable => {
                cmd_cycle_enable(&config_dir)?;
            }
            CycleCommands::Disable => {
                cmd_cycle_disable(&config_dir)?;
            }
            CycleCommands::Now { force } => {
                cmd_cycle_now(&config_dir, force)?;
            }
            CycleCommands::History => {
                cmd_cycle_history(&config_dir)?;
            }
            CycleCommands::Reorder { accounts } => {
                cmd_cycle_reorder(&config_dir, accounts)?;
            }
            CycleCommands::Schedule { command } => match command {
                ScheduleCommands::Enable { interval } => {
                    println!(
                        "Schedule enable with interval {} minutes - not yet implemented",
                        interval
                    );
                }
                ScheduleCommands::Disable => {
                    println!("Schedule disable - not yet implemented");
                }
            },
        },
        Commands::History { command } => {
            use crate::history::{HistoryDatabase, NotificationConfig};
            let db = HistoryDatabase::new(&config_dir)?;

            match command {
                HistoryCommands::Daemon { command } => match command {
                    DaemonCommands::Start { interval } => {
                        println!("Starting daemon with interval {} - use 'codex-usage history daemon start --interval {}'", interval, interval);
                        println!(
                            "Daemon functionality requires the daemonize crate implementation"
                        );
                    }
                    DaemonCommands::Stop => {
                        println!("Stopping daemon...");
                    }
                    DaemonCommands::Status => {
                        println!("Daemon status: not running");
                    }
                },
                HistoryCommands::Show {
                    period: _,
                    from: _,
                    to: _,
                    account,
                } => {
                    let account_name = account.unwrap_or_else(|| "default".to_string());
                    let snapshots = db.get_snapshots(&account_name, None, None, Some(100))?;

                    if snapshots.is_empty() {
                        println!("No history found for account '{}'.", account_name);
                        println!("Start the daemon to begin recording usage history.");
                        return Ok(());
                    }

                    println!("Usage History for {}:", account_name);
                    println!("{}", "=".repeat(50));

                    for snapshot in snapshots.iter().take(20) {
                        let dt = chrono::DateTime::from_timestamp(snapshot.timestamp, 0)
                            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        println!("{}", dt);
                        if let Some(p) = snapshot.five_hour_percent {
                            println!("  5h window:  {:.1}% used", p);
                        }
                        if let Some(p) = snapshot.weekly_percent {
                            println!("  Weekly:       {:.1}% used", p);
                        }
                        println!();
                    }
                }
                HistoryCommands::Chart { accounts: _ } => {
                    println!("Terminal chart visualization");
                    println!("This feature requires ratatui integration.");
                    let all_accounts = db.get_accounts()?;
                    if all_accounts.is_empty() {
                        println!("No history data available. Start the daemon to begin recording.");
                    } else {
                        println!("Available accounts: {:?}", all_accounts);
                    }
                }
                HistoryCommands::Allowance {
                    projected,
                    dead_time,
                    account,
                } => {
                    let account_name = account.unwrap_or_else(|| "default".to_string());
                    let snapshots = db.get_snapshots(&account_name, None, None, None)?;

                    if snapshots.is_empty() {
                        println!("No history found for account '{}'.", account_name);
                        return Ok(());
                    }

                    println!("Allowance Analysis for {}", account_name);
                    println!("{}", "=".repeat(50));

                    let total_snapshots = snapshots.len();
                    if let Some(latest) = snapshots.first() {
                        if let Some(weekly) = latest.weekly_percent {
                            println!("Current weekly usage: {:.1}%", weekly);
                        }
                    }
                    println!("Total snapshots recorded: {}", total_snapshots);

                    if projected {
                        println!(
                            "\nProjection: Enable daemon for more data to generate projections."
                        );
                    }
                    if dead_time {
                        println!("\nDead time analysis: Enable daemon for more data.");
                    }
                }
                HistoryCommands::Notify {
                    #[allow(unused_variables)]
                    enable,
                    disable,
                    hours_before,
                    status,
                    account,
                } => {
                    let account_name = account.unwrap_or_else(|| "default".to_string());

                    if status {
                        if let Some(config) = db.get_notification_config(&account_name)? {
                            println!("Notification config for {}:", account_name);
                            println!("  Enabled: {}", config.enabled);
                            println!(
                                "  Notify {} hours before reset",
                                config.notify_before_reset_hours
                            );
                            if let Some(ts) = config.last_notified {
                                let dt = chrono::DateTime::from_timestamp(ts, 0)
                                    .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                                    .unwrap_or_else(|| "unknown".to_string());
                                println!("  Last notified: {}", dt);
                            }
                        } else {
                            println!(
                                "No notification config for {}. Use --enable to configure.",
                                account_name
                            );
                        }
                        return Ok(());
                    }

                    let enabled = enable || !disable;
                    let config = NotificationConfig {
                        id: None,
                        account_name: account_name.clone(),
                        notify_before_reset_hours: hours_before.unwrap_or(12),
                        enabled,
                        last_notified: None,
                    };
                    db.set_notification_config(&config)?;

                    if !enabled {
                        println!("Notifications disabled for {}.", account_name);
                    } else {
                        println!(
                            "Notifications enabled for {} (notify {} hours before reset).",
                            account_name, config.notify_before_reset_hours
                        );
                    }
                }
                HistoryCommands::Export {
                    output,
                    format: _,
                    period,
                    from,
                    to,
                } => {
                    let export_data = serde_json::json!({
                        "exported_at": chrono::Utc::now().to_rfc3339(),
                        "period": period,
                        "from": from,
                        "to": to,
                    });

                    let json_str = serde_json::to_string_pretty(&export_data)?;

                    if let Some(path) = output {
                        fs::write(&path, &json_str)?;
                        println!("Exported to {}", path);
                    } else {
                        println!("{}", json_str);
                    }
                }
            }
        }
    }

    Ok(())
}
