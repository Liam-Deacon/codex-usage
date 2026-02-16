# codex-usage

CLI tool to track OpenAI Codex usage with multi-account support and automatic account cycling.

## Features

- **Usage Tracking**: Check 5-hour and weekly usage limits for Codex CLI accounts
- **Multi-Account Management**: Add, switch, remove, and list multiple Codex accounts
- **Automatic Cycling**: Automatically switch accounts when usage limits are exhausted
- **Multiple Output Formats**: Table, JSON, and compact oneline formats
- **Caching**: 5-minute cache to reduce API calls

## Installation

### macOS

```bash
# Using Homebrew (coming soon)
brew install Liam-Deacon/tap/codex-usage
```

### Linux

```bash
# Download from releases
curl -L https://github.com/Liam-Deacon/codex-usage/releases/latest/download/codex-usage-x86_64-unknown-linux-gnu -o codex-usage
chmod +x codex-usage
sudo mv codex-usage /usr/local/bin/
```

### Windows

```bash
# Using Scoop (coming soon)
scoop bucket add Liam-Deacon https://github.com/Liam-Deacon/scoop-bucket
scoop install codex-usage
```

### Node.js (npx / bunx)

```bash
npx codex-usage-cli --help
bunx codex-usage-cli --help
```

### Python (uvx)

```bash
uvx codex-usage --help
```

### Docker

```bash
docker pull ghcr.io/liam-deacon/codex-usage:latest
docker run --rm ghcr.io/liam-deacon/codex-usage:latest --help
```

### Build from Source

```bash
cargo build --release
sudo cp target/release/codex-usage /usr/local/bin/
```

## Quick Start

1. **Login to Codex** (if not already):
   ```bash
   codex login
   ```

2. **Add your account**:
   ```bash
   codex-usage accounts add myaccount
   ```

3. **Check usage**:
   ```bash
   codex-usage status
   ```

## Usage

### Check Usage

```bash
# Check active account usage
codex-usage status

# Check all connected accounts
codex-usage status --all

# Output as JSON
codex-usage status --json

# Compact oneline output
codex-usage status --oneline

# Force refresh (skip cache)
codex-usage status --refresh
```

### Account Management

```bash
# List all connected accounts
codex-usage accounts list

# Add current Codex auth as new account
codex-usage accounts add myaccount

# Switch to another account
codex-usage accounts switch myaccount

# Switch with force (override safety check)
codex-usage accounts switch myaccount --force

# Remove an account
codex-usage accounts remove myaccount
```

### Automatic Cycling

Configure automatic account switching when usage limits are exhausted:

```bash
# Show cycle status
codex-usage cycle status

# Configure thresholds
codex-usage cycle config --five-hour 0 --weekly 10 --mode or

# Enable cycling
codex-usage cycle enable

# Disable cycling
codex-usage cycle disable

# Manually trigger cycle check
codex-usage cycle now

# View cycle history
codex-usage cycle history

# Reorder accounts in cycle
codex-usage cycle reorder account1 account2 account3
```

### Wakeup

Trigger Codex to utilize daily/weekly limits:

```bash
# Run wakeup now
codex-usage wakeup --run

# Run wakeup for specific account
codex-usage wakeup --run --account myaccount

# Force wake even if Codex is running
codex-usage wakeup --run --force

# Install wakeup schedule (e.g., daily at 8am and 2pm)
codex-usage wakeup install --at 08:00 --at 14:00

# Install with interval between --at times
codex-usage wakeup install --at 08:00 --interval 1h

# List current wakeup schedules
codex-usage wakeup list

# Remove wakeup schedule
codex-usage wakeup remove

# Attempt to wake system from sleep (macOS)
codex-usage wakeup --run --wake-system
```

### Watch

Continuously monitor usage with live updates:

```bash
# Watch active account (default 10s interval)
codex-usage watch

# Watch with custom interval
codex-usage watch --interval 30s

# Watch all accounts
codex-usage watch --all

# Force refresh on each poll (skip cache)
codex-usage watch --refresh
```

### History

Track and analyze usage over time:

```bash
# Manage background recording daemon
codex-usage history daemon start --interval 5m
codex-usage history daemon stop
codex-usage history daemon status

# Show usage history
codex-usage history show
codex-usage history show --period week
codex-usage history show --from 2025-01-01 --to 2025-01-31
codex-usage history show --account myaccount

# Show terminal bar chart
codex-usage history chart
codex-usage history chart account1 account2

# Show allowance analysis
codex-usage history allowance
codex-usage history allowance --projected
codex-usage history allowance --dead-time

# Configure notifications
codex-usage history notify --enable
codex-usage history notify --disable
codex-usage history notify --hours-before 2
codex-usage history notify --status

# Export history data
codex-usage history export
codex-usage history export --output usage.json
codex-usage history export --period month --format json
```

## Configuration

### Config Directory

Default: `~/.codex-usage/`

Override with:
```bash
codex-usage --config-dir /path/to/config status
```

Or set environment variable:
```bash
export CODEX_USAGE_DIR=/path/to/config
```

### Files

- `config.json` - Main configuration
- `accounts/` - Stored account auth files
- `cycle.json` - Cycle configuration
- `cycle_history.jsonl` - Cycle history
- `usage_cache.json` - Usage data cache
- `wakeup.json` - Wakeup configuration

## Environment Variables

| Variable | Description |
|----------|-------------|
| `CODEX_USAGE_DIR` | Override config directory |
| `CODEX_USAGE_VERBOSE` | Enable verbose logging |

## License

MIT License - see [LICENSE](LICENSE) file.

## Related

- [Codex CLI](https://github.com/openai/codex)
- [agent-skills](https://github.com/Liam-Deacon/agent-skills)
