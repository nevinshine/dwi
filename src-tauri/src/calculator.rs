/// DWI Calculation Engine
/// Translates system metrics (CPU, Network, Storage) into carbon impact.
///
/// Core Equation:  DWI Score = E_cpu + E_network + Storage_waste
///   - E_cpu     = Wattage × Load × Time
///   - E_network = GB × 0.81 kWh/GB
///   - Carbon    = Energy × Grid Factor (gCO₂/kWh)

/// Default system TDP (Thermal Design Power) in Watts.
/// 65W is a reasonable baseline for desktop CPUs.
const BASE_WATTAGE: f64 = 65.0;

/// Energy cost of network transfer: 0.81 kWh per GB
const NETWORK_KWH_PER_GB: f64 = 0.81;

/// Regional grid carbon intensity: gCO₂ per kWh
/// 400 gCO₂/kWh is a global average. Future: use ElectricityMaps API.
const GRID_FACTOR: f64 = 400.0;

/// Carbon impact snapshot — what the frontend displays.
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct CarbonSnapshot {
    /// Total DWI score in grams of CO₂ for this session
    pub total_co2_grams: f64,
    /// CO₂ from CPU usage (grams)
    pub cpu_co2_grams: f64,
    /// CO₂ from network usage (grams)
    pub net_co2_grams: f64,
    /// CPU energy consumed (Wh)
    pub cpu_energy_wh: f64,
    /// Network energy consumed (Wh)
    pub net_energy_wh: f64,
    /// Equivalent: how many smartphone charges
    pub equiv_phone_charges: f64,
    /// Equivalent: meters of car driving
    pub equiv_car_meters: f64,
    /// Equivalent: minutes of LED bulb
    pub equiv_led_minutes: f64,
    /// Session duration in seconds
    pub session_seconds: u64,
}

/// Accumulator that tracks running totals across the monitoring session.
pub struct CarbonAccumulator {
    /// Total CPU energy in Watt-hours
    cpu_energy_wh: f64,
    /// Total network energy in Watt-hours
    net_energy_wh: f64,
    /// Polling interval in seconds
    interval_secs: f64,
    /// Session start time
    started_at: std::time::Instant,
}

impl CarbonAccumulator {
    pub fn new(interval_secs: f64) -> Self {
        CarbonAccumulator {
            cpu_energy_wh: 0.0,
            net_energy_wh: 0.0,
            interval_secs,
            started_at: std::time::Instant::now(),
        }
    }

    /// Record a single polling tick's worth of CPU and network usage.
    ///
    /// - `cpu_load_percent`: CPU usage 0–100%
    /// - `net_bytes_delta`: bytes transferred since last tick
    pub fn record_tick(&mut self, cpu_load_percent: f64, net_bytes_delta: u64) {
        // CPU Energy: Wattage × Load fraction × Time (hours)
        let load_fraction = cpu_load_percent / 100.0;
        let time_hours = self.interval_secs / 3600.0;
        let cpu_wh = BASE_WATTAGE * load_fraction * time_hours;
        self.cpu_energy_wh += cpu_wh;

        // Network Energy: GB × 0.81 kWh/GB → convert to Wh
        let gb = net_bytes_delta as f64 / 1_073_741_824.0;
        let net_wh = gb * NETWORK_KWH_PER_GB * 1000.0; // kWh → Wh
        self.net_energy_wh += net_wh;
    }

    /// Get the current carbon snapshot.
    pub fn snapshot(&self) -> CarbonSnapshot {
        let cpu_co2 = self.cpu_energy_wh / 1000.0 * GRID_FACTOR; // Wh → kWh → gCO₂
        let net_co2 = self.net_energy_wh / 1000.0 * GRID_FACTOR;
        let total_co2 = cpu_co2 + net_co2;

        let session_secs = self.started_at.elapsed().as_secs();

        CarbonSnapshot {
            total_co2_grams: round2(total_co2),
            cpu_co2_grams: round2(cpu_co2),
            net_co2_grams: round2(net_co2),
            cpu_energy_wh: round2(self.cpu_energy_wh),
            net_energy_wh: round2(self.net_energy_wh),
            // A smartphone charge ≈ 10 Wh
            equiv_phone_charges: round2((self.cpu_energy_wh + self.net_energy_wh) / 10.0),
            // Average car emits ~121 gCO₂/km → ~0.121 gCO₂/m
            equiv_car_meters: round2(total_co2 / 0.121),
            // 10W LED bulb: 10W → in 1 minute = 10/60 Wh = 0.167 Wh
            equiv_led_minutes: round2((self.cpu_energy_wh + self.net_energy_wh) / 0.167),
            session_seconds: session_secs,
        }
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_carbon_calculation() {
        let mut acc = CarbonAccumulator::new(5.0);

        // Simulate 100% CPU for 1 hour (720 ticks × 5s = 3600s)
        for _ in 0..720 {
            acc.record_tick(100.0, 0);
        }

        let snap = acc.snapshot();
        // E_cpu = 65W × 1.0 × 1h = 65 Wh
        assert!((snap.cpu_energy_wh - 65.0).abs() < 0.1,
            "Expected ~65 Wh, got {}", snap.cpu_energy_wh);
        // CO₂ = 65Wh / 1000 × 400 = 26g
        assert!((snap.cpu_co2_grams - 26.0).abs() < 0.1,
            "Expected ~26g CO₂, got {}", snap.cpu_co2_grams);
    }

    #[test]
    fn test_network_carbon_calculation() {
        let mut acc = CarbonAccumulator::new(5.0);

        // Simulate 1 GB of network transfer in a single tick
        acc.record_tick(0.0, 1_073_741_824);

        let snap = acc.snapshot();
        // E_net = 1GB × 0.81 kWh/GB = 810 Wh
        assert!((snap.net_energy_wh - 810.0).abs() < 1.0,
            "Expected ~810 Wh, got {}", snap.net_energy_wh);
        // CO₂ = 810Wh / 1000 × 400 = 324g
        assert!((snap.net_co2_grams - 324.0).abs() < 1.0,
            "Expected ~324g CO₂, got {}", snap.net_co2_grams);
    }

    #[test]
    fn test_combined_score() {
        let mut acc = CarbonAccumulator::new(5.0);
        acc.record_tick(50.0, 500_000_000); // 50% CPU + 0.47 GB

        let snap = acc.snapshot();
        assert!(snap.total_co2_grams > 0.0);
        // Use approximate comparison to avoid f64 precision issues
        let expected_total = snap.cpu_co2_grams + snap.net_co2_grams;
        assert!((snap.total_co2_grams - expected_total).abs() < 0.01,
            "Expected ~{}, got {}", expected_total, snap.total_co2_grams);
    }
}
