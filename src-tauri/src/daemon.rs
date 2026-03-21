use std::sync::Arc;
use sysinfo::{Networks, System};
use crate::calculator::{CarbonAccumulator, CarbonSnapshot};
use crate::db::Database;
use crate::detectors::{self, DoomscrollTracker, WasteAlert};

/// Snapshot of current system metrics + carbon impact, sent to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemSnapshot {
    pub cpu_usage: f64,
    pub ram_used_mb: f64,
    pub ram_total_mb: f64,
    pub ram_percent: f64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub net_rx_rate_kbps: f64,
    pub net_tx_rate_kbps: f64,
    pub uptime_seconds: u64,
    pub timestamp: String,
    /// Live carbon impact from the CarbonAccumulator
    pub carbon: CarbonSnapshot,
    /// Active waste alert (if any detector triggered)
    pub active_alert: Option<WasteAlert>,
}

/// Collect a single snapshot of system metrics.
fn collect_snapshot(sys: &mut System, networks: &mut Networks) -> (f64, u64, u64, u64, u64, f64, f64, f64, u64, String) {
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    networks.refresh();

    let cpu_usage = sys.global_cpu_usage() as f64;
    let ram_used = sys.used_memory();
    let ram_total = sys.total_memory();
    let ram_used_mb = ram_used as f64 / 1_048_576.0;
    let ram_total_mb = ram_total as f64 / 1_048_576.0;
    let ram_percent = if ram_total > 0 {
        (ram_used as f64 / ram_total as f64) * 100.0
    } else {
        0.0
    };

    let mut total_rx: u64 = 0;
    let mut total_tx: u64 = 0;
    for (_name, data) in networks.iter() {
        total_rx += data.total_received();
        total_tx += data.total_transmitted();
    }

    let ts = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    (cpu_usage, ram_used, ram_total, total_rx, total_tx, ram_used_mb, ram_total_mb, ram_percent, sysinfo::System::uptime(), ts)
}

/// Spawn the background daemon that polls metrics every 5 seconds.
/// Now also feeds the CarbonAccumulator and runs waste detectors each tick.
pub fn spawn_daemon(db: Arc<Database>, snapshot_state: Arc<std::sync::Mutex<Option<SystemSnapshot>>>) {
    std::thread::spawn(move || {
        let mut sys = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();

        // Carbon accumulator — tracks running energy/CO₂ totals
        let mut carbon = CarbonAccumulator::new(5.0);

        // Doomscroll tracker — needs persistent state across ticks
        let mut doomscroll = DoomscrollTracker::new();

        // Let sysinfo settle for accurate initial CPU reading
        std::thread::sleep(std::time::Duration::from_millis(500));

        let mut prev_rx: u64 = 0;
        let mut prev_tx: u64 = 0;
        let mut first_run = true;

        loop {
            let (cpu_usage, _ram_used, _ram_total, total_rx, total_tx,
                 ram_used_mb, ram_total_mb, ram_percent, uptime, ts) =
                collect_snapshot(&mut sys, &mut networks);

            // Calculate network deltas
            let mut rx_rate_kbps = 0.0;
            let mut tx_rate_kbps = 0.0;
            let mut rx_delta: u64 = 0;

            if !first_run {
                let interval_secs = 5.0;
                rx_delta = total_rx.saturating_sub(prev_rx);
                let tx_delta = total_tx.saturating_sub(prev_tx);
                rx_rate_kbps = (rx_delta as f64) / 1024.0 / interval_secs;
                tx_rate_kbps = (tx_delta as f64) / 1024.0 / interval_secs;

                // Feed the carbon accumulator with this tick's data
                carbon.record_tick(cpu_usage, rx_delta + tx_delta);
            }

            prev_rx = total_rx;
            prev_tx = total_tx;
            first_run = false;

            // Run waste detectors
            // Tab Tax: refresh process list and scan for idle browsers
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            let tab_alert = detectors::check_tab_tax(&sys);

            // Doomscroll: check network vs input
            let rx_bytes_per_sec = (rx_delta as f64 / 5.0) as u64;
            let doomscroll_alert = doomscroll.check(rx_bytes_per_sec);

            // Priority: Tab Tax > Doomscroll (tab tax is more immediately actionable)
            let active_alert = tab_alert.or(doomscroll_alert);

            // Build the full snapshot
            let snap = SystemSnapshot {
                cpu_usage,
                ram_used_mb,
                ram_total_mb,
                ram_percent,
                net_rx_bytes: total_rx,
                net_tx_bytes: total_tx,
                net_rx_rate_kbps: rx_rate_kbps,
                net_tx_rate_kbps: tx_rate_kbps,
                uptime_seconds: uptime,
                timestamp: ts.clone(),
                carbon: carbon.snapshot(),
                active_alert,
            };

            // Log to database (fire-and-forget, never block)
            let _ = db.insert_metric(
                &ts,
                cpu_usage,
                (ram_used_mb * 1_048_576.0) as u64,
                (ram_total_mb * 1_048_576.0) as u64,
                total_rx,
                total_tx,
            );

            // Update the shared snapshot for the frontend
            if let Ok(mut state) = snapshot_state.lock() {
                *state = Some(snap);
            }

            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    });
}

#[cfg(test)]
mod tests {
    use sysinfo::{Networks, System};

    #[test]
    fn test_collect_metrics() {
        let mut sys = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();
        std::thread::sleep(std::time::Duration::from_millis(250));

        let (cpu, _ru, _rt, _rx, _tx, _rum, rtm, _rp, _up, ts) =
            super::collect_snapshot(&mut sys, &mut networks);

        // CPU usage should be a valid percentage
        assert!(cpu >= 0.0 && cpu <= 100.0, "CPU usage out of range: {}", cpu);

        // RAM should be non-zero
        assert!(rtm > 0.0, "RAM total should be > 0");

        // Timestamp should not be empty
        assert!(!ts.is_empty(), "Timestamp should not be empty");
    }
}
