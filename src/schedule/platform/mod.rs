use crate::schedule::config::WakeupSchedule;
use anyhow::Result;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod unix;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
#[allow(unused_imports)]
pub use macos::*;

#[cfg(target_os = "linux")]
#[allow(unused_imports)]
pub use unix::*;

#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows::*;

pub fn install(schedule: &WakeupSchedule) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        crate::schedule::platform::macos::install_schedule(schedule)
    }

    #[cfg(target_os = "linux")]
    {
        crate::schedule::platform::unix::install_schedule(schedule)
    }

    #[cfg(target_os = "windows")]
    {
        crate::schedule::platform::windows::install_schedule(schedule)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("Unsupported operating system")
    }
}

pub fn remove() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        crate::schedule::platform::macos::remove_schedule()
    }

    #[cfg(target_os = "linux")]
    {
        crate::schedule::platform::unix::remove_schedule()
    }

    #[cfg(target_os = "windows")]
    {
        crate::schedule::platform::windows::remove_schedule()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("Unsupported operating system")
    }
}

pub fn list() -> Result<Vec<String>> {
    #[cfg(target_os = "macos")]
    {
        crate::schedule::platform::macos::list_schedules()
    }

    #[cfg(target_os = "linux")]
    {
        crate::schedule::platform::unix::list_schedules()
    }

    #[cfg(target_os = "windows")]
    {
        crate::schedule::platform::windows::list_schedules()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok(Vec::new())
    }
}
