use crate::config::system_settings::PerformanceMode;
use std::sync::{Arc, Mutex};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

/// Monitors system resources to prevent ensuring the app doesn't impact performance
/// Designed to be "respectful" of user's hardware
pub struct ResourceMonitor {
    sys: Arc<Mutex<System>>,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        // Initialize with specific refresh kinds to minimize overhead
        let sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        Self {
            sys: Arc::new(Mutex::new(sys)),
        }
    }

    /// Check if the system is under heavy load based on the current mode
    /// Returns true if the app should PAUSE/THROTTLE background activities
    pub fn should_pause(&self, mode: PerformanceMode) -> bool {
        let mut sys = self.sys.lock().unwrap();

        // Refresh only what we need
        sys.refresh_cpu();
        sys.refresh_memory();

        // Calculate global CPU usage
        let cpu_count = sys.cpus().len() as f32;
        if cpu_count == 0.0 {
            return false; // Should not happen, but safe fallback
        }

        let global_cpu_usage: f32 =
            sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / cpu_count;
        let memory_usage = sys.used_memory() as f64 / sys.total_memory() as f64;

        // Check battery (if available, this handles laptops)
        // Note: sysinfo may not fully support battery on all platforms in the System struct directly
        // usually need components, but for now we focus on CPU/RAM as primary indicators of "busy"

        match mode {
            PerformanceMode::Eco => {
                // strict limits for battery saving
                if global_cpu_usage > 30.0 {
                    tracing::debug!(
                        "ResourceMonitor: Pausing (Eco) - CPU at {:.1}%",
                        global_cpu_usage
                    );
                    return true;
                }
                if memory_usage > 0.70 {
                    tracing::debug!(
                        "ResourceMonitor: Pausing (Eco) - Memory at {:.1}%",
                        memory_usage * 100.0
                    );
                    return true;
                }
            }
            PerformanceMode::Balanced => {
                // standard limits
                if global_cpu_usage > 70.0 {
                    tracing::debug!(
                        "ResourceMonitor: Pausing (Balanced) - CPU at {:.1}%",
                        global_cpu_usage
                    );
                    return true;
                }
                if memory_usage > 0.85 {
                    tracing::debug!(
                        "ResourceMonitor: Pausing (Balanced) - Memory at {:.1}%",
                        memory_usage * 100.0
                    );
                    return true;
                }
            }
            PerformanceMode::High => {
                // loose limits, mostly just preventing crash
                if memory_usage > 0.95 {
                    tracing::debug!(
                        "ResourceMonitor: Pausing (High) - Memory at {:.1}%",
                        memory_usage * 100.0
                    );
                    return true;
                }
            }
        }

        false
    }
}
