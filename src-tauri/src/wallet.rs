/// Eco-Credit Wallet — Gamification engine for DWI.
///
/// Users earn credits proportional to CO₂ they prevent.
/// Prototype rate: 1g CO₂ saved = 1 Eco-Credit.

use crate::db::{CreditLogRow, Database, WalletRow};
use serde::Serialize;

/// Full wallet status sent to the frontend dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct WalletStatus {
    pub balance: f64,
    pub lifetime_total: f64,
    pub streak: i32,
    pub longest_streak: i32,
    pub today_credits: f64,
    pub recent_actions: Vec<CreditLogRow>,
}

/// Calculate how many Eco-Credits a mitigation action earns.
/// Prototype: 1g CO₂ = 1 Eco-Credit (linear).
pub fn calculate_credit_yield(co2_grams: f64) -> f64 {
    co2_grams
}

/// Build the full wallet status for the frontend.
pub fn get_full_status(db: &Database) -> Result<WalletStatus, rusqlite::Error> {
    let wallet: WalletRow = db.get_wallet()?;
    let history = db.get_credit_history(5)?;
    let today = db.get_today_credits()?;

    Ok(WalletStatus {
        balance: wallet.total_credits,
        lifetime_total: wallet.lifetime_credits,
        streak: wallet.current_streak,
        longest_streak: wallet.longest_streak,
        today_credits: today,
        recent_actions: history,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_credit_yield() {
        assert_eq!(calculate_credit_yield(10.0), 10.0);
        assert_eq!(calculate_credit_yield(0.0), 0.0);
        assert_eq!(calculate_credit_yield(0.5), 0.5);
    }

    #[test]
    fn test_full_status() {
        let dir = PathBuf::from("/tmp/dwi_test_wallet_status");
        let _ = std::fs::remove_dir_all(&dir);

        let db = Database::init(&dir).expect("Failed to init DB");
        db.award_credits(5.0, 5.0, "tab_close", "Closed idle tabs").unwrap();

        let status = get_full_status(&db).expect("Failed to get status");
        assert_eq!(status.balance, 5.0);
        assert_eq!(status.lifetime_total, 5.0);
        assert_eq!(status.streak, 1);
        assert_eq!(status.today_credits, 5.0);
        assert_eq!(status.recent_actions.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
