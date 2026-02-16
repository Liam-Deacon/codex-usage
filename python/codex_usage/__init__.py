"""Python wrapper for codex-usage Rust library.

This module provides Python-friendly interfaces to the codex-usage functionality.
All functions return Python dicts/lists by parsing the JSON returned from Rust.
"""

import json
from typing import Optional, Dict, List, Any


def get_usage(
    account: Optional[str] = None,
    config_dir: Optional[str] = None,
    refresh: bool = False,
) -> Dict[str, Any]:
    """Get usage data for an account.

    Args:
        account: Account name (optional, uses active account if not specified)
        config_dir: Config directory path (optional)
        refresh: Force refresh, bypassing cache (default: False)

    Returns:
        Dictionary with usage data including:
        - account_name: str
        - status: str
        - plan: Optional[str]
        - primary_window: Optional[Dict] with used_percent, remaining_percent, window, resets_in
        - secondary_window: Optional[Dict] with used_percent, remaining_percent, window, resets_in
        - code_review: Optional[Dict] with used_percent
        - limit_reached: bool
        - auth_type: str

    Raises:
        RuntimeError: If fetching usage fails
    """
    from codex_usage import get_usage as _get_usage
    result = _get_usage(account, config_dir, refresh)
    return json.loads(result)


def list_accounts(config_dir: Optional[str] = None) -> List[Dict[str, Any]]:
    """List all configured accounts.

    Args:
        config_dir: Config directory path (optional)

    Returns:
        List of account dictionaries with:
        - name: str
        - active: bool
        - added_at: str
        - last_used: Optional[str]
    """
    from codex_usage import list_accounts as _list_accounts
    result = _list_accounts(config_dir)
    return json.loads(result)


def switch_account(
    name: str,
    config_dir: Optional[str] = None,
    force: bool = False,
) -> str:
    """Switch to a different account.

    Args:
        name: Account name to switch to
        config_dir: Config directory path (optional)
        force: Force switch even if Codex is running (default: False)

    Returns:
        Success message

    Raises:
        RuntimeError: If switching fails
    """
    from codex_usage import switch_account as _switch_account
    return _switch_account(name, config_dir, force)


def add_account(name: str, config_dir: Optional[str] = None) -> str:
    """Add current Codex auth as a new account.

    Args:
        name: Name for the new account
        config_dir: Config directory path (optional)

    Returns:
        Success message

    Raises:
        RuntimeError: If adding fails (e.g., no Codex auth found)
    """
    from codex_usage import add_account as _add_account
    return _add_account(name, config_dir)


def remove_account(name: str, config_dir: Optional[str] = None) -> str:
    """Remove an account.

    Args:
        name: Account name to remove
        config_dir: Config directory path (optional)

    Returns:
        Success message

    Raises:
        RuntimeError: If removal fails
    """
    from codex_usage import remove_account as _remove_account
    return _remove_account(name, config_dir)


def get_cycle_config(config_dir: Optional[str] = None) -> Dict[str, Any]:
    """Get cycling configuration.

    Args:
        config_dir: Config directory path (optional)

    Returns:
        Dictionary with:
        - enabled: bool
        - five_hour: float (threshold %)
        - weekly: float (threshold %)
        - mode: str ("and" or "or")
        - accounts: List[str]
        - current_index: int
        - last_cycle: Optional[str]
    """
    from codex_usage import get_cycle_config as _get_cycle_config
    result = _get_cycle_config(config_dir)
    return json.loads(result)


def set_cycle_config(
    config_dir: Optional[str] = None,
    five_hour: Optional[float] = None,
    weekly: Optional[float] = None,
    mode: Optional[str] = None,
) -> str:
    """Configure cycling thresholds.

    Args:
        config_dir: Config directory path (optional)
        five_hour: 5-hour threshold (remaining % that triggers switch)
        weekly: Weekly threshold (remaining % that triggers switch)
        mode: "and" (both thresholds) or "or" (either threshold)

    Returns:
        Success message

    Raises:
        RuntimeError: If configuration fails
    """
    from codex_usage import set_cycle_config as _set_cycle_config
    return _set_cycle_config(config_dir, five_hour, weekly, mode)


def cycle_enable(config_dir: Optional[str] = None) -> str:
    """Enable automatic cycling.

    Args:
        config_dir: Config directory path (optional)

    Returns:
        Success message
    """
    from codex_usage import cycle_enable as _cycle_enable
    return _cycle_enable(config_dir)


def cycle_disable(config_dir: Optional[str] = None) -> str:
    """Disable automatic cycling.

    Args:
        config_dir: Config directory path (optional)

    Returns:
        Success message
    """
    from codex_usage import cycle_disable as _cycle_disable
    return _cycle_disable(config_dir)


def cycle_now(
    force: bool = False,
    config_dir: Optional[str] = None,
) -> str:
    """Manually trigger cycle check.

    Args:
        force: Force cycle even if Codex is running (default: False)
        config_dir: Config directory path (optional)

    Returns:
        Success message

    Raises:
        RuntimeError: If cycling fails
    """
    from codex_usage import cycle_now as _cycle_now
    return _cycle_now(force, config_dir)


def get_cycle_status(config_dir: Optional[str] = None) -> Dict[str, Any]:
    """Get detailed cycle status.

    Args:
        config_dir: Config directory path (optional)

    Returns:
        Dictionary with:
        - enabled: bool
        - five_hour_threshold: float
        - weekly_threshold: float
        - mode: str
        - accounts: List[Dict] with name, is_current, is_next
        - last_cycle: Optional[str]
    """
    from codex_usage import get_cycle_status as _get_cycle_status
    result = _get_cycle_status(config_dir)
    return json.loads(result)


def get_config_dir() -> str:
    """Get the default config directory path.

    Returns:
        Path to config directory as string
    """
    from codex_usage import get_config_dir as _get_config_dir
    return _get_config_dir()


def run() -> str:
    """Run the CLI (equivalent to running `codex-usage` command).

    Returns:
        "Success" if CLI runs successfully

    Raises:
        RuntimeError: If CLI fails
    """
    from codex_usage import run as _run
    return _run()
