use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
#[allow(unused_imports)]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod schedule;

#[derive(Parser)]
#[command(name = "codex-usage")]
#[command(about = "Track OpenAI Codex usage with multi-account support", long_about = None)]
#[command(arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to config directory (default: ~/.codex-usage)
    #[arg(short, long, env = "CODEX_USAGE_DIR")]
    config_dir: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true, env = "CODEX_USAGE_VERBOSE")]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
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

    /// Wakeup/schedule command for scheduled cycling
    Wakeup {
        /// Install the wakeup schedule to system scheduler
        #[arg(long, group = "wakeup_action")]
        install: bool,

        /// Remove the wakeup schedule from system scheduler
        #[arg(long, group = "wakeup_action")]
        remove: bool,

        /// List current wakeup schedules
        #[arg(long, group = "wakeup_action")]
        list: bool,

        /// Time to trigger (repeatable, e.g., 08:00, 14:00)
        #[arg(long, value_name = "TIME")]
        at: Vec<String>,

        /// Run periodically after --at times (e.g., 1h, 30m)
        #[arg(long, value_name = "DURATION")]
        interval: Option<String>,

        /// Specific account to wake (default: all with cycling)
        #[arg(long, value_name = "NAME")]
        account: Option<String>,

        /// Force wake even if Codex is running
        #[arg(long)]
        force: bool,

        /// Attempt to wake system from sleep (macOS: pmset)
        #[arg(long)]
        wake_system: bool,

        /// Run wakeup now (used by scheduler)
        #[arg(long, group = "wakeup_action", required = true)]
        run: bool,
    },

    /// Cycle through accounts when limits exhausted
    Cycle {
        #[command(subcommand)]
        command: CycleCommands,
    },

    /// Continuously watch usage with live updates
    Watch {
        /// Poll interval (e.g., 10s, 30s, 1m)
        #[arg(long, default_value = "10s")]
        interval: String,

        /// Watch all accounts
        #[arg(short, long)]
        all: bool,

        /// Force refresh on each poll (skip cache)
        #[arg(short, long)]
        refresh: bool,
    },
}

#[derive(Subcommand)]
enum AccountCommands {
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
enum CycleCommands {
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
enum ScheduleCommands {
    /// Enable scheduled cycling
    Enable {
        /// Check interval in minutes
        #[arg(long, default_value = "60")]
        interval: u32,
    },

    /// Disable scheduled cycling
    Disable,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Config {
    active_account: Option<String>,
    accounts: HashMap<String, AccountInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AccountInfo {
    added_at: String,
    last_used: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct CycleConfig {
    enabled: bool,
    thresholds: CycleThresholds,
    mode: String,
    accounts: Vec<String>,
    current_index: usize,
    last_cycle: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct CycleThresholds {
    five_hour: f64,
    weekly: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CycleHistoryEntry {
    timestamp: String,
    from_account: String,
    to_account: String,
    reason: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct CodexAuth {
    #[serde(rename = "OPENAI_API_KEY")]
    api_key: Option<String>,
    tokens: Option<CodexTokens>,
}

#[derive(Debug, Deserialize, Clone)]
struct CodexTokens {
    access_token: Option<String>,
    account_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct UsageData {
    pub account_name: String,
    pub status: String,
    pub plan: Option<String>,
    pub primary_window: Option<RateWindow>,
    pub secondary_window: Option<RateWindow>,
    pub code_review: Option<CodeReview>,
    pub limit_reached: bool,
    pub auth_type: String,
}

#[derive(Debug, Serialize, Clone)]
struct RateWindow {
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub window: String,
    pub resets_in: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct CodeReview {
    pub used_percent: f64,
}

#[derive(Debug, Clone)]
struct UsageSample {
    timestamp: std::time::Instant,
    primary_used: f64,
    secondary_used: f64,
    code_review_used: f64,
}

#[derive(Debug, Clone)]
struct BurnRateStats {
    primary_burn: f64,
    primary_stddev: f64,
    secondary_burn: f64,
    secondary_stddev: f64,
    code_review_burn: f64,
    code_review_stddev: f64,
}

const USAGE_API_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const CACHE_TTL_SECS: u64 = 300;

fn get_config_dir() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".codex-usage"))
        .unwrap_or_else(|| PathBuf::from(".codex-usage"))
}

fn get_codex_dir() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".codex"))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn get_codex_auth_path() -> PathBuf {
    get_codex_dir().join("auth.json")
}

fn get_accounts_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("accounts")
}

fn get_account_auth_path(config_dir: &Path, name: &str) -> PathBuf {
    get_accounts_dir(config_dir).join(name).join("auth.json")
}

fn get_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("config.json")
}

fn get_cache_path(config_dir: &Path) -> PathBuf {
    config_dir.join("usage_cache.json")
}

fn get_cycle_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("cycle.json")
}

fn get_cycle_history_path(config_dir: &Path) -> PathBuf {
    config_dir.join("cycle_history.jsonl")
}

fn load_config(config_dir: &Path) -> Result<Config> {
    let config_path = get_config_path(config_dir);
    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&content).context("Failed to parse config")?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

fn save_config(config_dir: &Path, config: &Config) -> Result<()> {
    let config_path = get_config_path(config_dir);
    let content = serde_json::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(&config_path, content).context("Failed to write config")?;
    Ok(())
}

fn load_cycle_config(config_dir: &Path) -> Result<CycleConfig> {
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

fn save_cycle_config(config_dir: &Path, config: &CycleConfig) -> Result<()> {
    let path = get_cycle_config_path(config_dir);
    let content =
        serde_json::to_string_pretty(config).context("Failed to serialize cycle config")?;
    fs::write(&path, content).context("Failed to write cycle config")?;
    Ok(())
}

fn load_codex_auth(path: &Path) -> Result<Option<CodexAuth>> {
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
        let output = Command::new("pgrep").arg("-f").arg("codex").output();
        if let Ok(output) = output {
            return output.status.success();
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
                    return true;
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

fn cmd_accounts_list(config_dir: &Path) -> Result<()> {
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

fn cmd_accounts_add(config_dir: &Path, name: &str) -> Result<()> {
    let codex_auth = get_codex_auth_path();
    if !codex_auth.exists() {
        anyhow::bail!(
            "No Codex auth found. Please run 'codex login' first to authenticate with Codex."
        );
    }

    let account_auth_path = get_account_auth_path(config_dir, name);
    let accounts_dir = get_accounts_dir(config_dir);
    fs::create_dir_all(&accounts_dir).context("Failed to create accounts directory")?;
    copy_auth_file(&codex_auth, &account_auth_path)?;

    let mut config = load_config(config_dir)?;
    config.accounts.insert(
        name.to_string(),
        AccountInfo {
            added_at: chrono::Utc::now().to_rfc3339(),
            last_used: None,
        },
    );
    save_config(config_dir, &config)?;

    println!("Added account '{}' successfully.", name);
    println!("Auth file saved to: {:?}", account_auth_path);
    Ok(())
}

fn cmd_accounts_switch(config_dir: &Path, name: &str, force: bool) -> Result<()> {
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

fn cmd_accounts_remove(config_dir: &Path, name: &str) -> Result<()> {
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

fn fetch_usage(access_token: &str, account_id: &str) -> Result<UsageData> {
    let client = reqwest::blocking::Client::new();
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
    Ok(parse_usage_response(data, "current"))
}

fn get_cached_usage(config_dir: &Path) -> Option<UsageData> {
    let cache_path = get_cache_path(config_dir);
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

    let account_name = data
        .get("account_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let status = data
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("error")
        .to_string();
    let plan = data
        .get("plan")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let limit_reached = data
        .get("limit_reached")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let auth_type = data
        .get("auth_type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let primary_window = data.get("primary_window").and_then(|pw| {
        Some(RateWindow {
            used_percent: pw.get("used_percent")?.as_f64()?,
            remaining_percent: pw.get("remaining_percent")?.as_f64()?,
            window: pw.get("window")?.as_str()?.to_string(),
            resets_in: pw
                .get("resets_in")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    });

    let secondary_window = data.get("secondary_window").and_then(|sw| {
        Some(RateWindow {
            used_percent: sw.get("used_percent")?.as_f64()?,
            remaining_percent: sw.get("remaining_percent")?.as_f64()?,
            window: sw.get("window")?.as_str()?.to_string(),
            resets_in: sw
                .get("resets_in")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    });

    let code_review = data.get("code_review").and_then(|cr| {
        Some(CodeReview {
            used_percent: cr.get("used_percent")?.as_f64()?,
        })
    });

    Some(UsageData {
        account_name,
        status,
        plan,
        primary_window,
        secondary_window,
        code_review,
        limit_reached,
        auth_type,
    })
}

fn save_cache(config_dir: &Path, usage: &UsageData) -> Result<()> {
    let cache_path = get_cache_path(config_dir);
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

fn cmd_status(
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
                            if let Some(cached) = get_cached_usage(config_dir) {
                                if json {
                                    println!("{}", serde_json::to_string_pretty(&cached)?);
                                } else if oneline {
                                    print_oneline(&cached);
                                } else {
                                    print_usage(&cached);
                                }
                                return Ok(());
                            }
                        }

                        match fetch_usage(access_token, account_id) {
                            Ok(usage) => {
                                let _ = save_cache(config_dir, &usage);
                                if json {
                                    println!("{}", serde_json::to_string_pretty(&usage)?);
                                } else if oneline {
                                    print_oneline(&usage);
                                } else {
                                    print_usage(&usage);
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

    for account_name in &accounts_to_check {
        let account_auth_path = get_account_auth_path(config_dir, account_name);
        let auth = load_codex_auth(&account_auth_path)?;

        if let Some(auth) = auth {
            if let Some(tokens) = auth.tokens {
                if let (Some(access_token), Some(account_id)) =
                    (&tokens.access_token, &tokens.account_id)
                {
                    if !refresh {
                        if let Some(cached) = get_cached_usage(config_dir) {
                            if cached.account_name == *account_name {
                                all_usages.push(cached);
                                continue;
                            }
                        }
                    }

                    match fetch_usage(access_token, account_id) {
                        Ok(mut usage) => {
                            usage.account_name = account_name.clone();
                            let _ = save_cache(config_dir, &usage);
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

    if json {
        if all_usages.len() == 1 {
            println!("{}", serde_json::to_string_pretty(&all_usages[0])?);
        } else {
            println!("{}", serde_json::to_string_pretty(&all_usages)?);
        }
    } else if oneline {
        for usage in &all_usages {
            print_oneline(usage);
        }
    } else {
        for usage in &all_usages {
            print_usage(usage);
            println!();
        }
    }

    Ok(())
}

fn print_usage(usage: &UsageData) {
    println!("{}", "=".repeat(50));
    println!("  {}", usage.account_name);
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

fn cmd_cycle_status(config_dir: &Path) -> Result<()> {
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

fn cmd_cycle_config(
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

fn cmd_cycle_enable(config_dir: &Path) -> Result<()> {
    let mut cycle_config = load_cycle_config(config_dir)?;
    cycle_config.enabled = true;
    save_cycle_config(config_dir, &cycle_config)?;
    println!("Cycling enabled.");
    Ok(())
}

fn cmd_cycle_disable(config_dir: &Path) -> Result<()> {
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
            let mut parts = Vec::new();
            if five_hour_trigger {
                parts.push(format!("5h: {:.0}% remaining", five_hour_remaining));
            }
            if weekly_trigger {
                parts.push(format!("weekly: {:.0}% remaining", weekly_remaining));
            }
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

fn cmd_cycle_now(config_dir: &Path, force: bool) -> Result<()> {
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

    let current = config.active_account.as_deref().unwrap_or("");

    let current_idx = accounts
        .iter()
        .position(|a| a.as_str() == current)
        .unwrap_or(0);

    let next_idx = (current_idx + 1) % accounts.len();
    let next_account = &accounts[next_idx];

    let account_auth_path = get_account_auth_path(config_dir, next_account);
    let auth = load_codex_auth(&account_auth_path)?;

    if let Some(auth) = auth {
        if let Some(tokens) = auth.tokens {
            if let (Some(access_token), Some(account_id)) =
                (&tokens.access_token, &tokens.account_id)
            {
                let usage = fetch_usage(access_token, account_id)?;

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
                    copy_auth_file(&account_auth_path, &codex_auth)?;

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
            }
        }
    }

    Ok(())
}

fn cmd_cycle_history(config_dir: &Path) -> Result<()> {
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

fn parse_interval(s: &str) -> Result<std::time::Duration> {
    let s = s.trim();
    if let Some(stripped) = s.strip_suffix('s') {
        let val = stripped.parse::<u64>()?;
        Ok(std::time::Duration::from_secs(val))
    } else if let Some(stripped) = s.strip_suffix('m') {
        let val = stripped.parse::<u64>()?;
        Ok(std::time::Duration::from_secs(val * 60))
    } else if let Some(stripped) = s.strip_suffix('h') {
        let val = stripped.parse::<u64>()?;
        Ok(std::time::Duration::from_secs(val * 3600))
    } else if let Ok(val) = s.parse::<u64>() {
        Ok(std::time::Duration::from_secs(val))
    } else {
        anyhow::bail!(
            "Invalid interval format: {}. Use format like '10s', '30s', '1m', '1h'",
            s
        );
    }
}

fn calculate_burn_rate(samples: &[UsageSample]) -> Option<BurnRateStats> {
    if samples.len() < 2 {
        return None;
    }

    let first = &samples[0];
    let last = &samples[samples.len() - 1];
    let elapsed_secs = first.timestamp.elapsed().as_secs_f64();

    if elapsed_secs == 0.0 {
        return None;
    }

    let primary_burn = (last.primary_used - first.primary_used) / elapsed_secs * 60.0;
    let secondary_burn = (last.secondary_used - first.secondary_used) / elapsed_secs * 60.0;
    let code_review_burn = (last.code_review_used - first.code_review_used) / elapsed_secs * 60.0;

    let mut primary_diffs = Vec::new();
    let mut secondary_diffs = Vec::new();
    let mut code_review_diffs = Vec::new();

    for i in 1..samples.len() {
        let dt = samples[i].timestamp.elapsed().as_secs_f64();
        if dt > 0.0 {
            primary_diffs.push((samples[i].primary_used - samples[i - 1].primary_used) / dt * 60.0);
            secondary_diffs
                .push((samples[i].secondary_used - samples[i - 1].secondary_used) / dt * 60.0);
            code_review_diffs
                .push((samples[i].code_review_used - samples[i - 1].code_review_used) / dt * 60.0);
        }
    }

    fn mean(v: &[f64]) -> f64 {
        if v.is_empty() {
            return 0.0;
        }
        v.iter().sum::<f64>() / v.len() as f64
    }

    fn stddev(v: &[f64]) -> f64 {
        if v.len() < 2 {
            return 0.0;
        }
        let m = mean(v);
        let variance = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64;
        variance.sqrt()
    }

    Some(BurnRateStats {
        primary_burn,
        primary_stddev: stddev(&primary_diffs),
        secondary_burn,
        secondary_stddev: stddev(&secondary_diffs),
        code_review_burn,
        code_review_stddev: stddev(&code_review_diffs),
    })
}

fn format_burn_rate(burn: f64, stddev: f64) -> String {
    if stddev > 0.0 {
        format!("{:.1}%/min ±{:.1}", burn.abs(), stddev.abs())
    } else {
        format!("{:.1}%/min", burn.abs())
    }
}

fn print_progress_bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn format_uptime(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

fn process_account_usage(
    account_name: &str,
    access_token: &str,
    account_id: &str,
    samples_map: &mut HashMap<String, VecDeque<UsageSample>>,
) -> Result<()> {
    let usage = fetch_usage(access_token, account_id)?;

    let primary_used = usage
        .primary_window
        .as_ref()
        .map(|w| w.used_percent)
        .unwrap_or(0.0);
    let secondary_used = usage
        .secondary_window
        .as_ref()
        .map(|w| w.used_percent)
        .unwrap_or(0.0);
    let code_review_used = usage
        .code_review
        .as_ref()
        .map(|w| w.used_percent)
        .unwrap_or(0.0);

    let samples = samples_map.entry(account_name.to_string()).or_default();
    samples.push_back(UsageSample {
        timestamp: std::time::Instant::now(),
        primary_used,
        secondary_used,
        code_review_used,
    });

    while samples.len() > 30 {
        samples.pop_front();
    }

    print_watch_usage(&usage, samples.make_contiguous());
    Ok(())
}

fn cmd_status_watch(
    config_dir: &Path,
    interval_str: &str,
    all: bool,
    _refresh: bool,
) -> Result<()> {
    let interval = parse_interval(interval_str)?;
    let start_time = std::time::Instant::now();
    let mut samples_map: HashMap<String, VecDeque<UsageSample>> = HashMap::new();
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst);
    })?;

    println!("Watching usage (Ctrl+C to stop)...");
    println!();

    loop {
        if !running.load(Ordering::SeqCst) {
            println!("\nStopped.");
            break;
        }

        let config = load_config(config_dir)?;

        let accounts_to_check: Vec<String> = if all {
            config.accounts.keys().cloned().collect()
        } else {
            vec![config
                .active_account
                .clone()
                .unwrap_or_else(|| "default".to_string())]
        };

        let now = chrono::Local::now();
        println!("\x1B[2J\x1B[1H");
        println!("Last updated: {}", now.format("%Y-%m-%d %H:%M:%S"));
        let total_samples: usize = samples_map.values().map(VecDeque::len).sum();
        println!(
            "Uptime: {} | Samples: {}",
            format_uptime(start_time.elapsed()),
            total_samples
        );
        println!("{}", "=".repeat(60));

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
                            if let Err(e) = process_account_usage(
                                "default",
                                access_token,
                                account_id,
                                &mut samples_map,
                            ) {
                                eprintln!("Error fetching usage: {}", e);
                            }
                        }
                    }
                }
            } else {
                println!("No active account. Run 'codex login' first.");
            }
        } else {
            for account_name in &accounts_to_check {
                let account_auth_path = get_account_auth_path(config_dir, account_name);
                let auth = match load_codex_auth(&account_auth_path) {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("Error loading auth for {}: {}", account_name, e);
                        continue;
                    }
                };

                if let Some(auth) = auth {
                    if let Some(tokens) = auth.tokens {
                        if let (Some(access_token), Some(account_id)) =
                            (&tokens.access_token, &tokens.account_id)
                        {
                            if let Err(e) = process_account_usage(
                                account_name,
                                access_token,
                                account_id,
                                &mut samples_map,
                            ) {
                                eprintln!("Error fetching usage for {}: {}", account_name, e);
                            }
                        }
                    }
                }
            }
        }

        let sleep_slice = std::time::Duration::from_millis(250);
        let mut remaining = interval;
        while remaining > sleep_slice {
            if !running.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(sleep_slice);
            remaining = remaining.checked_sub(sleep_slice).unwrap_or_default();
        }
        if running.load(Ordering::SeqCst) {
            std::thread::sleep(remaining);
        }
    }

    Ok(())
}

fn print_watch_usage(usage: &UsageData, samples: &[UsageSample]) {
    let burn_stats = calculate_burn_rate(samples);

    println!("\n{}", usage.account_name);
    println!("{}", "-".repeat(40));

    if let Some(pw) = &usage.primary_window {
        let burn_str = burn_stats
            .as_ref()
            .map(|b| {
                format!(
                    " (burn: {})",
                    format_burn_rate(b.primary_burn, b.primary_stddev)
                )
            })
            .unwrap_or_default();
        println!("  {} Window:", pw.window);
        println!(
            "    {}  {:.1}% remaining{}",
            print_progress_bar(pw.remaining_percent, 10),
            pw.remaining_percent,
            burn_str
        );
        if let Some(reset) = &pw.resets_in {
            println!("    Resets in: {}", reset);
        }
    }

    if let Some(sw) = &usage.secondary_window {
        let burn_str = burn_stats
            .as_ref()
            .map(|b| {
                format!(
                    " (burn: {})",
                    format_burn_rate(b.secondary_burn, b.secondary_stddev)
                )
            })
            .unwrap_or_default();
        println!("  {} Window:", sw.window);
        println!(
            "    {}  {:.1}% remaining{}",
            print_progress_bar(sw.remaining_percent, 10),
            sw.remaining_percent,
            burn_str
        );
        if let Some(reset) = &sw.resets_in {
            println!("    Resets in: {}", reset);
        }
    }

    if let Some(cr) = &usage.code_review {
        let burn_str = burn_stats
            .as_ref()
            .map(|b| {
                format!(
                    " (burn: {})",
                    format_burn_rate(b.code_review_burn, b.code_review_stddev)
                )
            })
            .unwrap_or_default();
        println!("  Code Review:");
        println!(
            "    {}  {:.1}% used{}",
            print_progress_bar(cr.used_percent, 10),
            cr.used_percent,
            burn_str
        );
    }

    if usage.limit_reached {
        println!("  ⚠️  Rate limit reached!");
    }
}

fn cmd_cycle_reorder(config_dir: &Path, accounts: Vec<String>) -> Result<()> {
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

fn cmd_wakeup_install(
    config_dir: &Path,
    times: &[String],
    interval: Option<&str>,
    account: Option<&str>,
    wake_system: bool,
) -> Result<()> {
    use crate::schedule::{
        create_schedule, load_wakeup_config_with_dir, parse_duration, parse_time, platform,
        save_wakeup_config_with_dir,
    };

    if times.is_empty() {
        anyhow::bail!("At least one --at time must be specified");
    }

    let parsed_times: Result<Vec<chrono::NaiveTime>, _> =
        times.iter().map(|t| parse_time(t)).collect();
    let times = parsed_times.context("Failed to parse times")?;

    let interval_duration = if let Some(i) = interval {
        Some(parse_duration(i).context("Failed to parse interval")?)
    } else {
        None
    };

    let schedule = create_schedule(
        "default",
        times,
        interval_duration,
        account.map(String::from),
        wake_system,
    )?;

    let schedule_name = schedule.name.clone();
    let times_str: Vec<String> = schedule
        .times
        .iter()
        .map(|t| t.format("%H:%M").to_string())
        .collect();

    platform::install(&schedule)?;

    let mut config = load_wakeup_config_with_dir(config_dir)?;
    config.add_schedule(schedule);
    save_wakeup_config_with_dir(config_dir, &config)?;

    println!(
        "Installed wakeup schedule '{}' at {}",
        schedule_name,
        times_str.join(", ")
    );

    Ok(())
}

fn cmd_wakeup_remove(config_dir: &Path) -> Result<()> {
    use crate::schedule::{load_wakeup_config_with_dir, platform, save_wakeup_config_with_dir};

    platform::remove()?;

    let mut config = load_wakeup_config_with_dir(config_dir)?;
    config.clear_schedules();
    save_wakeup_config_with_dir(config_dir, &config)?;

    Ok(())
}

fn cmd_wakeup_list() -> Result<()> {
    use crate::schedule::platform;

    let schedules = platform::list()?;

    if schedules.is_empty() {
        println!("No wakeup schedules configured.");
    } else {
        println!("Wakeup schedules:");
        for schedule in &schedules {
            println!("  - {}", schedule);
        }
    }

    Ok(())
}

fn cmd_wakeup_run(config_dir: &Path, account: Option<&str>, force: bool) -> Result<()> {
    if is_codex_running() && !force {
        anyhow::bail!("Codex is running — use --force to run wakeup anyway.");
    }

    if let Some(account_name) = account {
        println!("Waking specific account: {}", account_name);
        cmd_accounts_switch(config_dir, account_name, force)?;
    } else {
        println!("Running wakeup cycle...");
        cmd_cycle_now(config_dir, force)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_dir = cli.config_dir.unwrap_or_else(get_config_dir);

    tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .init();

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
            install,
            remove,
            list,
            at,
            interval,
            account,
            force,
            wake_system,
            run,
        } => {
            if run {
                cmd_wakeup_run(&config_dir, account.as_deref(), force)?;
            } else if list {
                cmd_wakeup_list()?;
            } else if remove {
                cmd_wakeup_remove(&config_dir)?;
            } else if install {
                cmd_wakeup_install(
                    &config_dir,
                    &at,
                    interval.as_deref(),
                    account.as_deref(),
                    wake_system,
                )?;
            } else {
                anyhow::bail!("Must specify one of --install, --remove, --list, or --run");
            }
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
        Commands::Watch {
            interval,
            all,
            refresh,
        } => {
            cmd_status_watch(&config_dir, &interval, all, refresh)?;
        }
    }

    Ok(())
}
