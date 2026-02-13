use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn is_codex_running() -> bool {
    // Check for codex process on Unix-like systems
    #[cfg(unix)]
    {
        let output = Command::new("pgrep").arg("-f").arg("codex").output();

        if let Ok(output) = output {
            return output.status.success();
        }
    }

    // Check for lock file
    let lock_path = get_codex_dir().join(".codex.lock");
    if lock_path.exists() {
        // Check if process is actually running by reading lock file
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
                    return true; // Assume running if lock exists on Windows
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

    // Ensure parent directory exists
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

    // Create accounts directory
    let accounts_dir = get_accounts_dir(config_dir);
    fs::create_dir_all(&accounts_dir).context("Failed to create accounts directory")?;

    // Copy auth file
    copy_auth_file(&codex_auth, &account_auth_path)?;

    // Update config
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
    // Check if Codex is running
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

    // Backup current auth if exists
    if codex_auth.exists() {
        let backup_path = codex_auth.with_extension("json.backup");
        fs::copy(&codex_auth, &backup_path).ok();
    }

    // Copy new auth
    copy_auth_file(&account_auth_path, &codex_auth)?;

    // Update config
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

    // Remove account files
    if let Some(parent) = account_auth_path.parent() {
        fs::remove_dir_all(parent).context("Failed to remove account directory")?;
    }

    // Update config
    let mut config = load_config(config_dir)?;
    config.accounts.remove(name);

    if config.active_account.as_deref() == Some(name) {
        config.active_account = None;
    }

    save_config(config_dir, &config)?;

    println!("Removed account '{}' successfully.", name);

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config_dir = cli.config_dir.unwrap_or_else(get_config_dir);

    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .init();

    tracing::debug!("Config directory: {:?}", config_dir);

    // Ensure config directory exists
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
            tracing::debug!(
                "Status command: all={}, json={}, oneline={}, refresh={}",
                all,
                json,
                oneline,
                refresh
            );
            println!("codex-usage status - use --all to check all accounts");
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
    }

    Ok(())
}
