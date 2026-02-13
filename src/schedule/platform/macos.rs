use crate::schedule::config::WakeupSchedule;
use crate::schedule::parse::format_time;
use anyhow::{Context, Result};
use chrono::Timelike;
use plist::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const LAUNCH_AGENT_LABEL: &str = "com.codex-usage.wakeup";

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

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
        escape_xml(LAUNCH_AGENT_LABEL),
        program_args
            .iter()
            .map(|s| format!("<string>{}</string>", escape_xml(s)))
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

    let uid = nix::unistd::Uid::current().as_raw();
    let target = format!("gui/{}", uid);

    let _ = Command::new("launchctl")
        .arg("bootout")
        .arg(&target)
        .arg(&plist_path)
        .output();

    let output = Command::new("launchctl")
        .arg("bootstrap")
        .arg(&target)
        .arg(&plist_path)
        .output()
        .context("Failed to bootstrap launchd agent")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to bootstrap launchd agent: {}", stderr);
    }

    println!(
        "Installed wakeup schedule: {} at {}",
        schedule.name,
        times_str.join(", ")
    );

    if schedule.wake_system {
        install_system_wake(schedule)?;
    }

    Ok(())
}

pub fn remove_schedule() -> Result<()> {
    let plist_path = get_launch_agent_path()?;

    let mut should_remove_system_wake = false;

    if plist_path.exists() {
        if let Ok(content) = fs::read_to_string(&plist_path) {
            if let Ok(plist) = Value::from_reader_xml(content.as_bytes()) {
                if let Some(dict) = plist.as_dictionary() {
                    if let Some(args) = dict.get("ProgramArguments").and_then(|v| v.as_array()) {
                        should_remove_system_wake = args
                            .iter()
                            .any(|arg| arg.as_string() == Some("--wake-system"));
                    }
                }
            }
        }

        let uid = nix::unistd::Uid::current().as_raw();
        let target = format!("gui/{}/{}", uid, LAUNCH_AGENT_LABEL);

        let output = Command::new("launchctl")
            .arg("bootout")
            .arg(&target)
            .output();

        match output {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("Warning: Failed to bootout launchd agent: {}", stderr);
            }
            Err(e) => {
                eprintln!("Warning: Failed to run bootout: {}", e);
            }
        }

        fs::remove_file(&plist_path).context("Failed to remove launchd plist")?;
        println!("Removed wakeup schedule.");
    }

    if should_remove_system_wake {
        remove_system_wake()?;
    }

    Ok(())
}

pub fn list_schedules() -> Result<Vec<String>> {
    let plist_path = get_launch_agent_path()?;

    if !plist_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&plist_path)?;

    if !content.contains(LAUNCH_AGENT_LABEL) {
        return Ok(Vec::new());
    }

    let plist = Value::from_reader_xml(content.as_bytes()).context("Failed to parse plist")?;

    let dict = plist.as_dictionary().context("Plist is not a dictionary")?;

    let mut schedules = Vec::new();

    if let Some(calendar_intervals) = dict.get("StartCalendarInterval") {
        if let Some(intervals) = calendar_intervals.as_array() {
            for interval in intervals {
                if let Some(interval_dict) = interval.as_dictionary() {
                    let hour: u64 = interval_dict
                        .get("Hour")
                        .and_then(|v: &Value| v.as_unsigned_integer())
                        .unwrap_or(0);
                    let minute: u64 = interval_dict
                        .get("Minute")
                        .and_then(|v: &Value| v.as_unsigned_integer())
                        .unwrap_or(0);
                    let time_str = format!("{:02}:{:02}", hour, minute);
                    schedules.push(time_str);
                }
            }
        }
    }

    Ok(schedules)
}

fn install_system_wake(schedule: &WakeupSchedule) -> Result<()> {
    use nix::unistd::Uid;

    if !Uid::effective().is_root() {
        anyhow::bail!("--wake-system requires root privileges. Run with sudo.");
    }

    let days_map = [
        ('1', "M"),
        ('2', "T"),
        ('3', "W"),
        ('4', "R"),
        ('5', "F"),
        ('6', "S"),
        ('7', "U"),
    ];
    let days: String = schedule
        .days
        .iter()
        .filter_map(|&d| {
            days_map
                .iter()
                .find(|(n, _)| *n == char::from_digit(d as u32, 10).unwrap())
                .map(|(_, letter)| *letter)
        })
        .collect();

    let days = if days.is_empty() {
        "MTWRF".to_string()
    } else {
        days
    };

    if schedule.times.len() > 1 {
        println!(
            "Warning: pmset repeat only supports one schedule. Using first time: {}",
            schedule.times[0].format("%H:%M:%S")
        );
    }

    let schedule_str = schedule.times[0].format("%H:%M:%S").to_string();

    let output = Command::new("pmset")
        .arg("repeat")
        .arg("wakeorpoweron")
        .arg(&days)
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
    use nix::unistd::Uid;

    if !Uid::effective().is_root() {
        anyhow::bail!("--wake-system requires root privileges. Run with sudo.");
    }

    let output = Command::new("pmset")
        .arg("repeat")
        .arg("cancel")
        .output()
        .context("Failed to cancel pmset wake schedule")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to cancel system wake: {}", stderr);
    }

    println!("Removed system wake schedule.");
    Ok(())
}
