//! Lightweight performance telemetry

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerfSnapshot {
    pub timestamp: u64,
    pub app_uptime_secs: u64,
    pub memory_bytes: Option<u64>,
    pub cpu_usage: Option<f32>,
    pub load_avg: Option<f32>,
    pub battery_percent: Option<u8>,
}

static START_TIME: std::sync::OnceLock<SystemTime> = std::sync::OnceLock::new();

fn start_time() -> SystemTime {
    *START_TIME.get_or_init(SystemTime::now)
}

fn uptime_secs() -> u64 {
    start_time()
        .elapsed()
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

#[tauri::command]
pub fn get_perf_snapshot() -> PerfSnapshot {
    PerfSnapshot {
        timestamp: crate::core::utils::current_timestamp(),
        app_uptime_secs: uptime_secs(),
        memory_bytes: current_memory_bytes(),
        cpu_usage: current_cpu_usage(),
        load_avg: current_load_avg(),
        battery_percent: current_battery_percent(),
    }
}

#[cfg(target_os = "macos")]
fn current_memory_bytes() -> Option<u64> {
    use std::mem;

    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: u64,
        system_time: u64,
        policy: i32,
        suspend_count: i32,
    }

    const KERN_SUCCESS: i32 = 0;
    const MACH_TASK_BASIC_INFO: i32 = 20;

    extern "C" {
        fn mach_task_self() -> u32;
        fn task_info(
            target_task: u32,
            flavor: i32,
            task_info_out: *mut u32,
            task_info_outCnt: *mut u32,
        ) -> i32;
    }

    unsafe {
        let mut info = MachTaskBasicInfo {
            virtual_size: 0,
            resident_size: 0,
            resident_size_max: 0,
            user_time: 0,
            system_time: 0,
            policy: 0,
            suspend_count: 0,
        };
        let mut count = (mem::size_of::<MachTaskBasicInfo>() / mem::size_of::<u32>()) as u32;
        let kr = task_info(
            mach_task_self(),
            MACH_TASK_BASIC_INFO,
            &mut info as *mut _ as *mut u32,
            &mut count,
        );
        if kr == KERN_SUCCESS {
            return Some(info.resident_size);
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn current_memory_bytes() -> Option<u64> {
    None
}

fn current_cpu_usage() -> Option<f32> {
    let mut system = sysinfo::System::new();
    system.refresh_cpu();
    let cpus = system.cpus();
    if cpus.is_empty() {
        return None;
    }
    let total: f32 = cpus.iter().map(|c| c.cpu_usage()).sum();
    Some(total / cpus.len() as f32)
}

#[cfg(target_os = "macos")]
fn current_load_avg() -> Option<f32> {
    unsafe {
        let mut load = [0.0f64; 3];
        if libc::getloadavg(load.as_mut_ptr(), 3) > 0 {
            return Some(load[0] as f32);
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn current_load_avg() -> Option<f32> {
    None
}

#[cfg(target_os = "macos")]
fn current_battery_percent() -> Option<u8> {
    let output = std::process::Command::new("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for token in text.split_whitespace() {
        if let Some(percent) = token.strip_suffix('%') {
            if let Ok(value) = percent.parse::<u8>() {
                return Some(value);
            }
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn current_battery_percent() -> Option<u8> {
    None
}
