/// Waste Detectors — Tab Tax & Doomscroll Detector
///
/// Scans system state each daemon tick to identify digital waste patterns
/// and generate actionable alerts for the user.

use device_query::{DeviceQuery, DeviceState};
use sysinfo::System;

/// An actionable waste alert sent to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WasteAlert {
    /// "tab_tax" | "doomscroll"
    pub alert_type: String,
    /// Human-readable message for the alert banner
    pub message: String,
    /// Estimated grams of CO₂ the user can save by acting
    pub recoverable_co2: f64,
    /// Number of idle browser processes (for tab_tax)
    pub idle_count: u32,
    /// Total idle RAM in MB (for tab_tax)
    pub idle_ram_mb: f64,
}

/// Persistent state for the Doomscroll Detector — tracks consecutive idle ticks.
pub struct DoomscrollTracker {
    /// How many consecutive ticks had high net + zero input
    idle_ticks: u32,
    /// Device state handle (reused across ticks)
    device_state: DeviceState,
}

impl DoomscrollTracker {
    pub fn new() -> Self {
        DoomscrollTracker {
            idle_ticks: 0,
            device_state: DeviceState::new(),
        }
    }

    /// Check for passive consumption: high network throughput + zero user input.
    /// `rx_rate_bytes_per_sec`: network receive rate in bytes/sec
    /// Returns an alert if the pattern persists for >5 consecutive ticks (25s).
    pub fn check(&mut self, rx_rate_bytes_per_sec: u64) -> Option<WasteAlert> {
        let keys = self.device_state.get_keys();
        let mouse = self.device_state.get_mouse();
        let mouse_idle = !mouse.button_pressed.iter().any(|&b| b);

        // High download (>2 MB/s) with zero user interaction
        if rx_rate_bytes_per_sec > 2_000_000 && keys.is_empty() && mouse_idle {
            self.idle_ticks += 1;
        } else {
            self.idle_ticks = 0;
        }

        // Trigger after 5 consecutive idle ticks (~25 seconds for prototype;
        // PRD says 5 minutes but shorter threshold for demo/testing)
        if self.idle_ticks >= 5 {
            Some(WasteAlert {
                alert_type: "doomscroll".to_string(),
                message: format!(
                    "Passive data stream detected for {}s — consider pausing background downloads.",
                    self.idle_ticks * 5
                ),
                recoverable_co2: 2.5,
                idle_count: 0,
                idle_ram_mb: 0.0,
            })
        } else {
            None
        }
    }
}

/// Tab Tax — detect browser processes hoarding RAM while idle (near-zero CPU).
/// Scans all running processes for Chrome, Edge, Firefox with <1% CPU.
pub fn check_tab_tax(sys: &System) -> Option<WasteAlert> {
    let mut idle_ram_bytes: u64 = 0;
    let mut idle_count: u32 = 0;

    for (_pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        let is_browser = name.contains("chrome")
            || name.contains("chromium")
            || name.contains("msedge")
            || name.contains("firefox")
            || name.contains("brave");

        if is_browser && process.cpu_usage() < 1.0 {
            idle_ram_bytes += process.memory();
            idle_count += 1;
        }
    }

    let idle_ram_mb = idle_ram_bytes as f64 / 1_048_576.0;

    // Trigger if >500 MB of RAM is consumed by idle browsers
    if idle_ram_mb > 500.0 {
        // Estimate CO₂: idle RAM power ≈ 0.4W per GB
        // CO₂ per 5s tick at 400 gCO₂/kWh = 0.4W * (5/3600)h * 400 / 1000 * GB
        let idle_gb = idle_ram_mb / 1024.0;
        let recoverable = (idle_gb * 0.4 * (300.0 / 3600.0) * 400.0 / 1000.0 * idle_count as f64).max(1.0);

        Some(WasteAlert {
            alert_type: "tab_tax".to_string(),
            message: format!(
                "{} idle browser processes are hoarding {:.0} MB of RAM.",
                idle_count, idle_ram_mb
            ),
            recoverable_co2: (recoverable * 100.0).round() / 100.0,
            idle_count,
            idle_ram_mb: (idle_ram_mb * 10.0).round() / 10.0,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_tax_scans_system() {
        let sys = System::new_all();
        // Just verify it doesn't panic — result depends on running browsers
        let _alert = check_tab_tax(&sys);
    }

    #[test]
    fn test_doomscroll_tracker_init() {
        let mut tracker = DoomscrollTracker::new();
        // Low network = no alert
        let result = tracker.check(100_000);
        assert!(result.is_none());
    }

    #[test]
    fn test_doomscroll_needs_consecutive_ticks() {
        let mut tracker = DoomscrollTracker::new();
        // Even with high net, needs 5 consecutive ticks
        for _ in 0..4 {
            let result = tracker.check(5_000_000);
            assert!(result.is_none(), "Should not trigger before 5 ticks");
        }
    }
}
