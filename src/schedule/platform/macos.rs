use crate::schedule::config::WakeupSchedule;
use crate::schedule::parse::format_time;
use anyhow::{Context, Result};
use chrono::Timelike;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const LAUNCH_AGENT_LABEL: &str = "com.codex-usage.wakeup";

pub fn get_launch_agent_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    let launch_agents = home.join("Library/LaunchAgents");
    Ok(launch_agents.join(format!("{}.plist", LAUNCH_AGENT_LABEL)))
}

pub fn install_schedule(schedule: &WakeupSchedule) -> Result<()> {
    let plist_path = get_launch_agent_path()?;

    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent).context("Failed to create LaunchAgents directory")?;
    }

    let times_str: Vec<String> = schedule.times.iter().map(format_time).collect();
    let times_arg = times_str.join(",");

    let mut program_args = vec!["wakeup".to_string(), "--run".to_string()];
    if let Some(ref account) = schedule.account {
        program_args.push("--account".to_string());
        program_args.push(account.clone());
    }
    if schedule.wake_system {
        program_args.push("--wake-system".to_string());
    }

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>codex-usage</string>
        {}
    </array>
    <key>StartCalendarInterval</key>
    <array>
        {}
    </array>
    <key>RunAtLoad</key>
    <false/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>"#,
        LAUNCH_AGENT_LABEL,
        program_args
            .iter()
            .map(|s| format!("<string>{}</string>", s))
            .collect::<Vec<_>>()
            .join("\n        "),
        schedule
            .times
            .iter()
            .map(|t| format!(
                "        <dict>\n            <key>Hour</key>\n            <integer>{}</integer>\n            <key>Minute</key>\n            <integer>{}</integer>\n        </dict>",
                t.hour(),
                t.minute()
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    fs::write(&plist_path, plist_content).context("Failed to write launchd plist")?;

    Command::new("launchctl")
        .arg("load")
        .arg(&plist_path)
        .output()
        .context("Failed to load launchd agent")?;

    println!(
        "Installed wakeup schedule: {} at {}",
        schedule.name,
        times_str.join(", ")
    );

    if schedule.wake_system {
        install_system_wake(&schedule.times)?;
    }

    Ok(())
}

pub fn remove_schedule() -> Result<()> {
    let plist_path = get_launch_agent_path()?;

    if plist_path.exists() {
        Command::new("launchctl")
            .arg("unload")
            .arg(&plist_path)
            .output()
            .context("Failed to unload launchd agent")?;

        fs::remove_file(&plist_path).context("Failed to remove launchd plist")?;
        println!("Removed wakeup schedule.");
    }

    remove_system_wake()?;

    Ok(())
}

pub fn list_schedules() -> Result<Vec<String>> {
    let plist_path = get_launch_agent_path()?;

    if !plist_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&plist_path)?;
    let mut schedules = Vec::new();

    if content.contains(LAUNCH_AGENT_LABEL) {
        schedules.push(LAUNCH_AGENT_LABEL.to_string());
    }

    Ok(schedules)
}

fn install_system_wake(times: &[chrono::NaiveTime]) -> Result<()> {
    let days = "MTWRF";
    let times_str: Vec<String> = times
        .iter()
        .map(|t| t.format("%H:%M:%S").to_string())
        .collect();
    let schedule_str = times_str.join(" ");

    let output = Command::new("pmset")
        .arg("repeat")
        .arg("wakeorpoweron")
        .arg(days)
        .arg(&schedule_str)
        .output()
        .context("Failed to set pmset wake schedule")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to set system wake: {}", stderr);
    }

    println!("Configured system wake for {} at {}", days, schedule_str);
    Ok(())
}

fn remove_system_wake() -> Result<()> {
    let output = Command::new("pmset")
        .arg("repeat")
        .arg("cancel")
        .output()
        .context("Failed to cancel pmset wake schedule")?;

    if output.status.success() {
        println!("Removed system wake schedule.");
    }

    Ok(())
}
