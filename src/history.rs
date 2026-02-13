#![allow(dead_code)]
#![allow(unused_variables)]

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsageSnapshot {
    pub id: Option<i64>,
    pub account_name: String,
    pub timestamp: i64,
    pub five_hour_percent: Option<f64>,
    pub weekly_percent: Option<f64>,
    pub weekly_reset_timestamp: Option<i64>,
    pub five_hour_reset_timestamp: Option<i64>,
    pub plan: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NotificationConfig {
    pub id: Option<i64>,
    pub account_name: String,
    pub notify_before_reset_hours: i32,
    pub enabled: bool,
    pub last_notified: Option<i64>,
}

pub struct HistoryDatabase {
    conn: Mutex<Connection>,
}

impl HistoryDatabase {
    pub fn new(config_dir: &Path) -> Result<Self> {
        let db_path = config_dir.join("history.db");
        let conn = Connection::open(&db_path).context("Failed to open history database")?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS usage_snapshots (
                id INTEGER PRIMARY KEY,
                account_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                five_hour_percent REAL,
                weekly_percent REAL,
                weekly_reset_timestamp INTEGER,
                five_hour_reset_timestamp INTEGER,
                plan TEXT,
                status TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_time ON usage_snapshots(account_name, timestamp)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS notification_config (
                id INTEGER PRIMARY KEY,
                account_name TEXT NOT NULL UNIQUE,
                notify_before_reset_hours INTEGER DEFAULT 12,
                enabled INTEGER DEFAULT 1,
                last_notified INTEGER
            )",
            [],
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[allow(dead_code)]
    pub fn insert_snapshot(&self, snapshot: &UsageSnapshot) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO usage_snapshots (account_name, timestamp, five_hour_percent, weekly_percent, weekly_reset_timestamp, five_hour_reset_timestamp, plan, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                snapshot.account_name,
                snapshot.timestamp,
                snapshot.five_hour_percent,
                snapshot.weekly_percent,
                snapshot.weekly_reset_timestamp,
                snapshot.five_hour_reset_timestamp,
                snapshot.plan,
                snapshot.status,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_snapshots(
        &self,
        account_name: &str,
        from_timestamp: Option<i64>,
        to_timestamp: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<UsageSnapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from("SELECT id, account_name, timestamp, five_hour_percent, weekly_percent, weekly_reset_timestamp, five_hour_reset_timestamp, plan, status FROM usage_snapshots WHERE account_name = ?1");

        let from_param = from_timestamp.as_ref();
        let to_param = to_timestamp.as_ref();

        if from_param.is_some() {
            sql.push_str(" AND timestamp >= ?2");
        }
        if to_param.is_some() {
            sql.push_str(" AND timestamp <= ?3");
        }
        sql.push_str(" ORDER BY timestamp DESC");

        if let Some(l) = limit {
            sql.push_str(&format!(" LIMIT {}", l));
        }

        let mut stmt = conn.prepare(&sql)?;

        let mut snapshots = Vec::new();

        match (from_param, to_param) {
            (Some(from), Some(to)) => {
                let rows = stmt.query_map(params![account_name, from, to], |row| {
                    Ok(UsageSnapshot {
                        id: Some(row.get(0)?),
                        account_name: row.get(1)?,
                        timestamp: row.get(2)?,
                        five_hour_percent: row.get(3)?,
                        weekly_percent: row.get(4)?,
                        weekly_reset_timestamp: row.get(5)?,
                        five_hour_reset_timestamp: row.get(6)?,
                        plan: row.get(7)?,
                        status: row.get(8)?,
                    })
                })?;
                for row in rows {
                    snapshots.push(row?);
                }
            }
            (Some(from), None) => {
                let rows = stmt.query_map(params![account_name, from], |row| {
                    Ok(UsageSnapshot {
                        id: Some(row.get(0)?),
                        account_name: row.get(1)?,
                        timestamp: row.get(2)?,
                        five_hour_percent: row.get(3)?,
                        weekly_percent: row.get(4)?,
                        weekly_reset_timestamp: row.get(5)?,
                        five_hour_reset_timestamp: row.get(6)?,
                        plan: row.get(7)?,
                        status: row.get(8)?,
                    })
                })?;
                for row in rows {
                    snapshots.push(row?);
                }
            }
            (None, Some(_to)) => {
                let rows = stmt.query_map(params![account_name], |row| {
                    Ok(UsageSnapshot {
                        id: Some(row.get(0)?),
                        account_name: row.get(1)?,
                        timestamp: row.get(2)?,
                        five_hour_percent: row.get(3)?,
                        weekly_percent: row.get(4)?,
                        weekly_reset_timestamp: row.get(5)?,
                        five_hour_reset_timestamp: row.get(6)?,
                        plan: row.get(7)?,
                        status: row.get(8)?,
                    })
                })?;
                for row in rows {
                    snapshots.push(row?);
                }
            }
            (None, None) => {
                let rows = stmt.query_map(params![account_name], |row| {
                    Ok(UsageSnapshot {
                        id: Some(row.get(0)?),
                        account_name: row.get(1)?,
                        timestamp: row.get(2)?,
                        five_hour_percent: row.get(3)?,
                        weekly_percent: row.get(4)?,
                        weekly_reset_timestamp: row.get(5)?,
                        five_hour_reset_timestamp: row.get(6)?,
                        plan: row.get(7)?,
                        status: row.get(8)?,
                    })
                })?;
                for row in rows {
                    snapshots.push(row?);
                }
            }
        }

        Ok(snapshots)
    }

    pub fn get_notification_config(
        &self,
        account_name: &str,
    ) -> Result<Option<NotificationConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, account_name, notify_before_reset_hours, enabled, last_notified FROM notification_config WHERE account_name = ?1"
        )?;

        let mut rows = stmt.query(params![account_name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(NotificationConfig {
                id: Some(row.get(0)?),
                account_name: row.get(1)?,
                notify_before_reset_hours: row.get(2)?,
                enabled: row.get::<_, i32>(3)? == 1,
                last_notified: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn set_notification_config(&self, config: &NotificationConfig) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO notification_config (account_name, notify_before_reset_hours, enabled, last_notified)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                config.account_name,
                config.notify_before_reset_hours,
                if config.enabled { 1 } else { 0 },
                config.last_notified,
            ],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn update_last_notified(&self, account_name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE notification_config SET last_notified = ?1 WHERE account_name = ?2",
            params![now, account_name],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_all_notification_configs(&self) -> Result<Vec<NotificationConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, account_name, notify_before_reset_hours, enabled, last_notified FROM notification_config WHERE enabled = 1"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(NotificationConfig {
                id: Some(row.get(0)?),
                account_name: row.get(1)?,
                notify_before_reset_hours: row.get(2)?,
                enabled: row.get::<_, i32>(3)? == 1,
                last_notified: row.get(4)?,
            })
        })?;

        let mut configs = Vec::new();
        for row in rows {
            configs.push(row?);
        }
        Ok(configs)
    }

    pub fn get_accounts(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT DISTINCT account_name FROM usage_snapshots ORDER BY account_name")?;
        let rows = stmt.query_map([], |row| row.get(0))?;

        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row?);
        }
        Ok(accounts)
    }
}

#[allow(dead_code)]
pub fn get_history_db_path(config_dir: &Path) -> std::path::PathBuf {
    config_dir.join("history.db")
}
