use rusqlite::{Connection, Result, params};
use chrono::{Local, NaiveDate};
use std::path::PathBuf;
use std::sync::Mutex;

/// Telemetry row representing a single system metric snapshot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricRow {
    pub id: i64,
    pub timestamp: String,
    pub cpu_usage: f64,
    pub ram_used: u64,
    pub ram_total: u64,
    pub net_rx: u64,
    pub net_tx: u64,
}

/// The user's Eco-Credit wallet (singleton row).
#[derive(Debug, Clone, serde::Serialize)]
pub struct WalletRow {
    pub total_credits: f64,
    pub lifetime_credits: f64,
    pub current_streak: i32,
    pub longest_streak: i32,
    pub last_active_date: Option<String>,
}

/// A single credit transaction in the immutable log.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CreditLogRow {
    pub timestamp: String,
    pub credits: f64,
    pub co2_grams: f64,
    pub action_type: String,
    pub description: Option<String>,
}

/// Thread-safe wrapper around the SQLite connection.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open (or create) the DWI database at the given directory.
    /// Creates the `system_metrics` table if it doesn't exist.
    pub fn init(data_dir: &PathBuf) -> Result<Self> {
        std::fs::create_dir_all(data_dir).ok();
        let db_path = data_dir.join("dwi.db");
        let conn = Connection::open(&db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS system_metrics (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp  TEXT    NOT NULL,
                cpu_usage  REAL    NOT NULL,
                ram_used   INTEGER NOT NULL,
                ram_total  INTEGER NOT NULL,
                net_rx     INTEGER NOT NULL,
                net_tx     INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_metrics_ts ON system_metrics(timestamp);

            CREATE TABLE IF NOT EXISTS user_wallet (
                id              INTEGER PRIMARY KEY CHECK (id = 1),
                total_credits   REAL    NOT NULL DEFAULT 0.0,
                lifetime_credits REAL   NOT NULL DEFAULT 0.0,
                current_streak  INTEGER NOT NULL DEFAULT 0,
                longest_streak  INTEGER NOT NULL DEFAULT 0,
                last_active_date TEXT,
                created_at      TEXT    NOT NULL,
                updated_at      TEXT    NOT NULL
            );

            CREATE TABLE IF NOT EXISTS credit_log (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp   TEXT    NOT NULL,
                credits     REAL    NOT NULL,
                co2_grams   REAL    NOT NULL,
                action_type TEXT    NOT NULL,
                description TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_credit_log_ts ON credit_log(timestamp);",
        )?;

        // Ensure the singleton wallet row exists
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO user_wallet (id, created_at, updated_at) VALUES (1, ?1, ?1)",
            params![now],
        )?;

        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    /// Insert a single telemetry snapshot.
    pub fn insert_metric(
        &self,
        timestamp: &str,
        cpu_usage: f64,
        ram_used: u64,
        ram_total: u64,
        net_rx: u64,
        net_tx: u64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO system_metrics (timestamp, cpu_usage, ram_used, ram_total, net_rx, net_tx)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![timestamp, cpu_usage, ram_used as i64, ram_total as i64, net_rx as i64, net_tx as i64],
        )?;
        Ok(())
    }

    /// Retrieve the most recent N metric rows.
    pub fn get_recent_metrics(&self, limit: usize) -> Result<Vec<MetricRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, cpu_usage, ram_used, ram_total, net_rx, net_tx
             FROM system_metrics ORDER BY id DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(MetricRow {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                cpu_usage: row.get(2)?,
                ram_used: row.get::<_, i64>(3)? as u64,
                ram_total: row.get::<_, i64>(4)? as u64,
                net_rx: row.get::<_, i64>(5)? as u64,
                net_tx: row.get::<_, i64>(6)? as u64,
            })
        })?;

        rows.collect()
    }

    /// Get today's aggregate metrics for the end-of-day report.
    pub fn get_today_summary(&self) -> Result<Option<MetricRow>> {
        let conn = self.conn.lock().unwrap();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut stmt = conn.prepare(
            "SELECT 0, ?1, AVG(cpu_usage), MAX(ram_used), MAX(ram_total), MAX(net_rx), MAX(net_tx)
             FROM system_metrics WHERE timestamp LIKE ?2",
        )?;

        let pattern = format!("{}%", today);
        let row = stmt.query_row(params![today, pattern], |row| {
            Ok(MetricRow {
                id: 0,
                timestamp: row.get(1)?,
                cpu_usage: row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                ram_used: row.get::<_, Option<i64>>(3)?.unwrap_or(0) as u64,
                ram_total: row.get::<_, Option<i64>>(4)?.unwrap_or(0) as u64,
                net_rx: row.get::<_, Option<i64>>(5)?.unwrap_or(0) as u64,
                net_tx: row.get::<_, Option<i64>>(6)?.unwrap_or(0) as u64,
            })
        });

        match row {
            Ok(r) => Ok(Some(r)),
            Err(_) => Ok(None),
        }
    }

    /// Get the total count of stored metrics.
    pub fn get_metric_count(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM system_metrics", [], |row| row.get(0))
    }

    // ─── Eco-Credit Wallet Methods ───

    /// Get the current wallet state.
    pub fn get_wallet(&self) -> Result<WalletRow> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT total_credits, lifetime_credits, current_streak, longest_streak, last_active_date
             FROM user_wallet WHERE id = 1",
            [],
            |row| {
                Ok(WalletRow {
                    total_credits: row.get(0)?,
                    lifetime_credits: row.get(1)?,
                    current_streak: row.get(2)?,
                    longest_streak: row.get(3)?,
                    last_active_date: row.get(4)?,
                })
            },
        )
    }

    /// Award credits and log the transaction. Manages streak logic atomically.
    pub fn award_credits(
        &self,
        credits: f64,
        co2_grams: f64,
        action_type: &str,
        description: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Local::now();
        let today_str = now.format("%Y-%m-%d").to_string();
        let timestamp = now.to_rfc3339();

        // Read current wallet state
        let (mut total, mut lifetime, current_streak, longest, last_date): (f64, f64, i32, i32, Option<String>) =
            conn.query_row(
                "SELECT total_credits, lifetime_credits, current_streak, longest_streak, last_active_date
                 FROM user_wallet WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )?;

        // Streak logic
        let new_streak = if let Some(ref last_date_str) = last_date {
            if let Ok(last) = NaiveDate::parse_from_str(last_date_str, "%Y-%m-%d") {
                let today_date = now.naive_local().date();
                let diff = (today_date - last).num_days();
                if diff == 1 {
                    current_streak + 1
                } else if diff > 1 {
                    1
                } else {
                    current_streak // same day — no change
                }
            } else {
                1
            }
        } else {
            1 // first interaction ever
        };

        let new_longest = std::cmp::max(longest, new_streak);
        total += credits;
        lifetime += credits;

        // Atomic wallet update
        conn.execute(
            "UPDATE user_wallet SET
                total_credits = ?1, lifetime_credits = ?2, current_streak = ?3,
                longest_streak = ?4, last_active_date = ?5, updated_at = ?6
             WHERE id = 1",
            params![total, lifetime, new_streak, new_longest, today_str, timestamp],
        )?;

        // Append to immutable credit log
        conn.execute(
            "INSERT INTO credit_log (timestamp, credits, co2_grams, action_type, description)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![timestamp, credits, co2_grams, action_type, description],
        )?;

        Ok(())
    }

    /// Get the most recent N credit transactions.
    pub fn get_credit_history(&self, limit: usize) -> Result<Vec<CreditLogRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT timestamp, credits, co2_grams, action_type, description
             FROM credit_log ORDER BY id DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(CreditLogRow {
                timestamp: row.get(0)?,
                credits: row.get(1)?,
                co2_grams: row.get(2)?,
                action_type: row.get(3)?,
                description: row.get(4)?,
            })
        })?;

        rows.collect()
    }

    /// Get total credits earned today.
    pub fn get_today_credits(&self) -> Result<f64> {
        let conn = self.conn.lock().unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        let pattern = format!("{}%", today);
        conn.query_row(
            "SELECT COALESCE(SUM(credits), 0.0) FROM credit_log WHERE timestamp LIKE ?1",
            params![pattern],
            |row| row.get(0),
        )
    }

    /// Get total CO₂ saved today (grams) from credit log actions.
    pub fn get_today_co2_saved(&self) -> Result<f64> {
        let conn = self.conn.lock().unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        let pattern = format!("{}%", today);
        conn.query_row(
            "SELECT COALESCE(SUM(co2_grams), 0.0) FROM credit_log WHERE timestamp LIKE ?1",
            params![pattern],
            |row| row.get(0),
        )
    }

    /// Get the number of credit actions today.
    pub fn get_today_action_count(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        let pattern = format!("{}%", today);
        conn.query_row(
            "SELECT COUNT(*) FROM credit_log WHERE timestamp LIKE ?1",
            params![pattern],
            |row| row.get(0),
        )
    }

    /// Count today's metric snapshots.
    pub fn get_today_snapshot_count(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        let pattern = format!("{}%", today);
        conn.query_row(
            "SELECT COUNT(*) FROM system_metrics WHERE timestamp LIKE ?1",
            params![pattern],
            |row| row.get(0),
        )
    }

    /// Deduct credits for a redemption. Returns error if insufficient balance.
    pub fn deduct_credits(&self, amount: f64, reason: &str) -> Result<f64> {
        let conn = self.conn.lock().unwrap();
        let now = Local::now();
        let timestamp = now.to_rfc3339();

        let current: f64 = conn.query_row(
            "SELECT total_credits FROM user_wallet WHERE id = 1",
            [],
            |row| row.get(0),
        )?;

        if current < amount {
            return Err(rusqlite::Error::QueryReturnedNoRows); // Insufficient funds
        }

        let new_balance = current - amount;
        conn.execute(
            "UPDATE user_wallet SET total_credits = ?1, updated_at = ?2 WHERE id = 1",
            params![new_balance, timestamp],
        )?;

        // Log as negative transaction
        conn.execute(
            "INSERT INTO credit_log (timestamp, credits, co2_grams, action_type, description)
             VALUES (?1, ?2, 0.0, 'redemption', ?3)",
            params![timestamp, -amount, reason],
        )?;

        Ok(new_balance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_init_and_insert() {
        let dir = PathBuf::from("/tmp/dwi_test_db");
        let _ = std::fs::remove_dir_all(&dir);

        let db = Database::init(&dir).expect("Failed to init DB");
        db.insert_metric("2025-01-01T00:00:00", 45.5, 8_000_000, 16_000_000, 1000, 500)
            .expect("Failed to insert");

        let count = db.get_metric_count().expect("Failed to count");
        assert_eq!(count, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_recent() {
        let dir = PathBuf::from("/tmp/dwi_test_recent");
        let _ = std::fs::remove_dir_all(&dir);

        let db = Database::init(&dir).expect("Failed to init DB");
        for i in 0..5 {
            db.insert_metric(
                &format!("2025-01-01T00:00:0{}", i),
                (i as f64) * 10.0,
                8_000_000,
                16_000_000,
                i * 100,
                i * 50,
            )
            .expect("Failed to insert");
        }

        let recent = db.get_recent_metrics(3).expect("Failed to query");
        assert_eq!(recent.len(), 3);
        // Most recent first
        assert!(recent[0].cpu_usage > recent[2].cpu_usage);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ─── Eco-Credit Wallet Tests ───

    #[test]
    fn test_wallet_init() {
        let dir = PathBuf::from("/tmp/dwi_test_wallet_init");
        let _ = std::fs::remove_dir_all(&dir);

        let db = Database::init(&dir).expect("Failed to init DB");
        let wallet = db.get_wallet().expect("Failed to get wallet");

        assert_eq!(wallet.total_credits, 0.0);
        assert_eq!(wallet.lifetime_credits, 0.0);
        assert_eq!(wallet.current_streak, 0);
        assert_eq!(wallet.longest_streak, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_award_credits() {
        let dir = PathBuf::from("/tmp/dwi_test_award");
        let _ = std::fs::remove_dir_all(&dir);

        let db = Database::init(&dir).expect("Failed to init DB");
        db.award_credits(5.0, 5.0, "tab_close", "Closed 5 idle tabs")
            .expect("Failed to award");

        let wallet = db.get_wallet().expect("Failed to get wallet");
        assert_eq!(wallet.total_credits, 5.0);
        assert_eq!(wallet.lifetime_credits, 5.0);
        assert_eq!(wallet.current_streak, 1);

        // Award more
        db.award_credits(3.0, 3.0, "duplicate_delete", "Removed 2 duplicates")
            .expect("Failed to award");

        let wallet = db.get_wallet().expect("Failed to get wallet");
        assert_eq!(wallet.total_credits, 8.0);
        assert_eq!(wallet.lifetime_credits, 8.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_credit_history() {
        let dir = PathBuf::from("/tmp/dwi_test_history");
        let _ = std::fs::remove_dir_all(&dir);

        let db = Database::init(&dir).expect("Failed to init DB");
        db.award_credits(1.0, 1.0, "tab_close", "Action 1").unwrap();
        db.award_credits(2.0, 2.0, "idle_kill", "Action 2").unwrap();
        db.award_credits(3.0, 3.0, "duplicate_delete", "Action 3").unwrap();

        let history = db.get_credit_history(2).expect("Failed to get history");
        assert_eq!(history.len(), 2);
        // Most recent first
        assert_eq!(history[0].credits, 3.0);
        assert_eq!(history[1].credits, 2.0);

        // Full history
        let all = db.get_credit_history(10).expect("Failed to get history");
        assert_eq!(all.len(), 3);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_today_credits() {
        let dir = PathBuf::from("/tmp/dwi_test_today");
        let _ = std::fs::remove_dir_all(&dir);

        let db = Database::init(&dir).expect("Failed to init DB");
        db.award_credits(4.0, 4.0, "tab_close", "Test").unwrap();
        db.award_credits(6.0, 6.0, "idle_kill", "Test2").unwrap();

        let today = db.get_today_credits().expect("Failed");
        assert_eq!(today, 10.0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
