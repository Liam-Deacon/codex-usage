use crate::schedule::config::WakeupSchedule;
use crate::schedule::parse::format_time;
use anyhow::{Context, Result};
use std::process::Command;

const CRON_TASK_NAME: &str = "codex-usage-wakeup";

pub fn install_schedule(schedule: &WakeupSchedule) -> Result<()> {
    let times_str: Vec<String> = schedule.times.iter().map(format_time).collect();

    let mut args = vec!["wakeup".to_string(), "--run".to_string()];
    if let Some(ref account) = schedule.account {
        args.push("--account".to_string());
        args.push(account.clone());
    }

    let mut cron_entries = Vec::new();
    for time_str in &times_str {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() >= 2 {
            let minute = parts[1];
            let hour = parts[0];
            let entry = format!(
                "{} {} * * 1-5 codex-usage {} # {}",
                minute,
                hour,
                args.join(" "),
                CRON_TASK_NAME
            );
            cron_entries.push(entry);
        }
    }

    let existing_crontab = get_current_crontab().unwrap_or_default();
    let filtered: Vec<String> = existing_crontab
        .lines()
        .filter(|line| !line.contains(CRON_TASK_NAME))
        .map(|s| s.to_string())
        .collect();

    let new_crontab = if filtered.is_empty() {
        cron_entries.join("\n")
    } else {
        format!("{}\n{}", filtered.join("\n"), cron_entries.join("\n"))
    };
    let new_crontab = if !new_crontab.ends_with('\n') {
        format!("{}\n", new_crontab)
    } else {
        new_crontab
    };

    set_crontab(&new_crontab)?;

    println!(
        "Installed wakeup schedule: {} at {}",
        schedule.name,
        times_str.join(", ")
    );
    Ok(())
}

pub fn remove_schedule() -> Result<()> {
    let existing_crontab = get_current_crontab().unwrap_or_default();

    let filtered: Vec<String> = existing_crontab
        .lines()
        .filter(|line| !line.contains(CRON_TASK_NAME))
        .map(|s| s.to_string())
        .collect();

    if filtered.is_empty() {
        let mut cmd = Command::new("crontab");
        cmd.arg("-r");
        let output = cmd.output();

        if output.is_err() || !output.as_ref().map(|o| o.status.success()).unwrap_or(false) {
            println!("No crontab to remove.");
            return Ok(());
        }
    } else {
        let filtered_crontab = filtered.join("\n");
        let filtered_crontab = if !filtered_crontab.ends_with('\n') {
            format!("{}\n", filtered_crontab)
        } else {
            filtered_crontab
        };
        set_crontab(&filtered_crontab)?;
    }

    println!("Removed wakeup schedule.");
    Ok(())
}

pub fn list_schedules() -> Result<Vec<String>> {
    let crontab = get_current_crontab().unwrap_or_default();
    let schedules: Vec<String> = crontab
        .lines()
        .filter(|line| line.contains(CRON_TASK_NAME))
        .map(|s| s.to_string())
        .collect();

    Ok(schedules)
}

fn get_current_crontab() -> Result<String> {
    let output = Command::new("crontab")
        .arg("-l")
        .output()
        .context("Failed to get crontab")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Ok(String::new())
    }
}

fn set_crontab(content: &str) -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    let mut cmd = Command::new("crontab");
    cmd.arg("-");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn crontab process")?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(content.as_bytes())?;
    }

    let output = child.wait_with_output().context("Failed to set crontab")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to set crontab: {}", stderr);
    }

    Ok(())
}
