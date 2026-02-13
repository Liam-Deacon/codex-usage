use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScheduleError {
    #[error("At least one time must be specified")]
    NoTimesSpecified,
    #[allow(dead_code)]
    #[error("Invalid time: {0}")]
    InvalidTime(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeupSchedule {
    pub name: String,
    pub account: Option<String>,
    pub times: Vec<NaiveTime>,
    #[serde(with = "serde_duration")]
    pub interval: Option<Duration>,
    pub wake_system: bool,
    pub enabled: bool,
}

mod serde_duration {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(d) => serializer.serialize_some(&d.as_secs_f64()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs: Option<f64> = Option::deserialize(deserializer)?;
        Ok(secs.map(Duration::from_secs_f64))
    }
}

impl Default for WakeupSchedule {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            account: None,
            times: Vec::new(),
            interval: None,
            wake_system: false,
            enabled: true,
        }
    }
}

impl WakeupSchedule {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    pub fn with_times(mut self, times: Vec<NaiveTime>) -> Self {
        self.times = times;
        self
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    pub fn with_account(mut self, account: Option<String>) -> Self {
        self.account = account;
        self
    }

    pub fn with_wake_system(mut self, wake_system: bool) -> Self {
        self.wake_system = wake_system;
        self
    }

    pub fn validate(&self) -> Result<(), ScheduleError> {
        if self.times.is_empty() {
            return Err(ScheduleError::NoTimesSpecified);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WakeupConfig {
    pub schedules: Vec<WakeupSchedule>,
}

impl WakeupConfig {
    pub fn new() -> Self {
        Self {
            schedules: Vec::new(),
        }
    }

    pub fn add_schedule(&mut self, schedule: WakeupSchedule) {
        if let Some(existing) = self.schedules.iter_mut().find(|s| s.name == schedule.name) {
            *existing = schedule;
        } else {
            self.schedules.push(schedule);
        }
    }

    #[allow(dead_code)]
    pub fn get_schedule(&self, name: &str) -> Option<&WakeupSchedule> {
        self.schedules.iter().find(|s| s.name == name)
    }

    #[allow(dead_code)]
    pub fn get_schedule_mut(&mut self, name: &str) -> Option<&mut WakeupSchedule> {
        self.schedules.iter_mut().find(|s| s.name == name)
    }

    #[allow(dead_code)]
    pub fn remove_schedule(&mut self, name: &str) -> bool {
        let len_before = self.schedules.len();
        self.schedules.retain(|s| s.name != name);
        self.schedules.len() < len_before
    }

    pub fn clear_schedules(&mut self) {
        self.schedules.clear();
    }
}
