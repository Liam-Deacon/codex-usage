use crate::schedule::config::WakeupSchedule;
use crate::schedule::parse::format_time;
use anyhow::{Context, Result};
use std::process::Command;

const TASK_NAME: &str = "CodexUsageWakeup";

pub fn install_schedule(schedule: &WakeupSchedule) -> Result<()> {
    let times_str: Vec<String> = schedule.times.iter().map(format_time).collect();

    let mut args = vec!["wakeup".to_string(), "--run".to_string()];
    if let Some(ref account) = schedule.account {
        args.push("--account".to_string());
        args.push(account.clone());
    }

    let exe_path = std::env::current_exe()
        .context("Failed to get current executable path")?
        .to_string_lossy()
        .to_string();

    for time_str in &times_str {
        let task_name = format!("{}_{}", TASK_NAME, time_str.replace(":", ""));

        let quoted_args: Vec<String> = args.iter().map(|a| format!("\"{}\"", a)).collect();

        let mut cmd = Command::new("schtasks");
        cmd.arg("/create");
        cmd.arg("/tn");
        cmd.arg(&task_name);
        cmd.arg("/tr");
        cmd.arg(format!("\"{}\" {}", exe_path, quoted_args.join(" ")));
        cmd.arg("/sc");
        cmd.arg("daily");
        cmd.arg("/st");
        cmd.arg(time_str);
        cmd.arg("/f");

        let output = cmd.output().context("Failed to create scheduled task")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create scheduled task: {}", stderr);
        }

        println!("Created scheduled task: {}", task_name);
    }

    if schedule.wake_system {
        enable_system_wake()?;
    }

    println!(
        "Installed wakeup schedule: {} at {}",
        schedule.name,
        times_str.join(", ")
    );
    Ok(())
}

pub fn remove_schedule() -> Result<()> {
    let output = Command::new("schtasks")
        .arg("/query")
        .arg("/fo")
        .arg("LIST")
        .output()
        .context("Failed to query scheduled tasks")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains(TASK_NAME) {
                let task_name = if let Some((_, name)) = line.split_once(':') {
                    name.trim().to_string()
                } else {
                    line.trim().to_string()
                };

                if task_name.is_empty() {
                    continue;
                }

                let del_output = Command::new("schtasks")
                    .arg("/delete")
                    .arg("/tn")
                    .arg(&task_name)
                    .arg("/f")
                    .output();

                if del_output.is_ok()
                    && del_output
                        .as_ref()
                        .map(|o| o.status.success())
                        .unwrap_or(false)
                {
                    println!("Deleted scheduled task: {}", task_name);
                }
            }
        }
    }

    disable_system_wake()?;

    println!("Removed wakeup schedule.");
    Ok(())
}

pub fn list_schedules() -> Result<Vec<String>> {
    let output = Command::new("schtasks")
        .arg("/query")
        .arg("/fo")
        .arg("LIST")
        .output()
        .context("Failed to query scheduled tasks")?;

    let schedules = if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .filter(|line| line.contains(TASK_NAME))
            .filter_map(|s| {
                if let Some((_, name)) = s.split_once(':') {
                    Some(name.trim().to_string())
                } else {
                    Some(s.trim().to_string())
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(schedules)
}

/// Enables system wake from sleep on Windows.
///
/// Note: Windows does not support automated wake-from-sleep scheduling via CLI.
/// Users must manually configure power settings:
///   powercfg /deviceenablewake "<device name>"
/// Or use: Control Panel > Hardware > Power Management > Allow wake timers
fn enable_system_wake() -> Result<()> {
    // TODO: implement Windows wake configuration via powercfg or return a specific Err variant
    println!("Note: To enable wake from sleep on Windows, configure power settings:");
    println!("  powercfg /deviceenablewake \"<device name>\"");
    println!("Or use: Control Panel > Hardware > Power Management > Allow wake timers");
    Ok(())
}

/// Disables system wake from sleep on Windows.
///
/// Note: Windows does not support automated wake-from-sleep scheduling via CLI.
fn disable_system_wake() -> Result<()> {
    // TODO: implement Windows wake configuration via powercfg or return a specific Err variant
    Ok(())
}
