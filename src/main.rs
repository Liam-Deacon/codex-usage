use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

fn get_config_dir() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".codex-usage"))
        .unwrap_or_else(|| PathBuf::from(".codex-usage"))
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
        std::fs::create_dir_all(&config_dir)?;
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
                println!("Listing accounts...");
            }
            AccountCommands::Add { name } => {
                println!("Adding account: {}", name);
            }
            AccountCommands::Switch { name, force } => {
                println!("Switching to account: {} (force={})", name, force);
            }
            AccountCommands::Remove { name } => {
                println!("Removing account: {}", name);
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
