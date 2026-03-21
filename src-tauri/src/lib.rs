mod db;
mod daemon;
mod calculator;
mod detectors;
mod wallet;
mod cleaner;

use db::Database;
use daemon::SystemSnapshot;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Shared application state — accessible from Tauri commands.
pub struct AppState {
    pub db: Arc<Database>,
    pub snapshot: Arc<Mutex<Option<SystemSnapshot>>>,
}

// ─── System Monitoring Commands ───

/// Tauri command: get the latest system snapshot for the frontend.
#[tauri::command]
fn get_system_status(state: tauri::State<'_, AppState>) -> Result<SystemSnapshot, String> {
    let snap = state.snapshot.lock().map_err(|e| e.to_string())?;
    snap.clone().ok_or_else(|| "No metrics collected yet — daemon is warming up".into())
}

/// Tauri command: get recent metric history.
#[tauri::command]
fn get_metric_history(state: tauri::State<'_, AppState>, limit: Option<usize>) -> Result<Vec<db::MetricRow>, String> {
    let lim = limit.unwrap_or(60);
    state.db.get_recent_metrics(lim).map_err(|e| e.to_string())
}

/// Tauri command: get total metric count.
#[tauri::command]
fn get_metric_count(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    state.db.get_metric_count().map_err(|e| e.to_string())
}

// ─── Eco-Credit Wallet Commands ───

/// Tauri command: get full wallet status for the dashboard.
#[tauri::command]
fn get_wallet_status(state: tauri::State<'_, AppState>) -> Result<wallet::WalletStatus, String> {
    wallet::get_full_status(&state.db).map_err(|e| e.to_string())
}

/// Tauri command: award Eco-Credits for a mitigation action.
#[tauri::command]
fn award_eco_credits(
    state: tauri::State<'_, AppState>,
    co2_grams: f64,
    action_type: String,
    description: String,
) -> Result<(), String> {
    let credits = wallet::calculate_credit_yield(co2_grams);
    state.db.award_credits(credits, co2_grams, &action_type, &description)
        .map_err(|e| e.to_string())
}

// ─── Redemption Commands ───

// TODO: Production values → subscription: 10_000, giftcard: 100_000
const COST_SUBSCRIPTION: f64 = 5.0;
const COST_GIFTCARD: f64 = 10.0;

#[derive(serde::Serialize)]
struct RedemptionResult {
    success: bool,
    new_balance: f64,
    code: String,
    message: String,
    reward_type: String,
}

/// Tauri command: redeem credits for a reward.
#[tauri::command]
fn redeem_credits(state: tauri::State<'_, AppState>, reward_type: String) -> Result<RedemptionResult, String> {
    let cost = match reward_type.as_str() {
        "subscription" => COST_SUBSCRIPTION,
        "giftcard" => COST_GIFTCARD,
        _ => return Err("Unknown reward type".into()),
    };

    let wallet = state.db.get_wallet().map_err(|e| e.to_string())?;
    if wallet.total_credits < cost {
        return Ok(RedemptionResult {
            success: false,
            new_balance: wallet.total_credits,
            code: String::new(),
            message: format!("Need {:.0} EC. You have {:.1} EC.", cost, wallet.total_credits),
            reward_type: reward_type.clone(),
        });
    }

    let label = match reward_type.as_str() {
        "subscription" => "DWI Pro Subscription",
        "giftcard" => "€5 Gift Card",
        _ => "Reward",
    };

    let new_balance = state.db.deduct_credits(cost, label)
        .map_err(|e| e.to_string())?;

    let prefix = if reward_type == "giftcard" { "GC" } else { "DWI" };
    let code = format!("{}-{:08X}", prefix, std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u32);

    Ok(RedemptionResult {
        success: true,
        new_balance,
        code,
        message: format!("{} activated! 🎉", label),
        reward_type,
    })
}

// ─── Cloud Cleaner Commands ───

/// Tauri command: scan a directory for duplicate files (async — won't freeze UI).
#[tauri::command]
async fn scan_duplicates(path: String) -> Result<cleaner::ScanResult, String> {
    // Expand ~ to home directory
    let expanded = if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            path.replacen("~", &home, 1)
        } else {
            path.clone()
        }
    } else if path == "~" {
        std::env::var("HOME").unwrap_or(path.clone())
    } else {
        path.clone()
    };

    let target = std::path::PathBuf::from(&expanded);
    if !target.is_dir() {
        return Err(format!("'{}' is not a valid directory", expanded));
    }

    // Run on a blocking thread so we don't freeze the UI
    tokio::task::spawn_blocking(move || {
        cleaner::scan_duplicates(&target, 1024, 10_000)
    })
    .await
    .map_err(|e| format!("Scan failed: {}", e))
}

// ─── End-of-Day Report Command ───

/// Daily report aggregate — everything the user accomplished today.
#[derive(serde::Serialize)]
struct DailyReport {
    /// Date string (YYYY-MM-DD)
    date: String,
    /// Total Eco-Credits earned today
    credits_earned: f64,
    /// Total grams of CO₂ prevented today
    co2_prevented_grams: f64,
    /// Number of mitigation actions taken today
    actions_taken: i64,
    /// Number of metric snapshots collected today
    snapshots_collected: i64,
    /// Average CPU usage today
    avg_cpu_usage: f64,
    /// Current streak
    current_streak: i32,
    /// Longest streak
    longest_streak: i32,
    /// Session carbon from CarbonSnapshot (if available)
    session_co2_grams: f64,
}

/// Tauri command: get the end-of-day summary report.
#[tauri::command]
fn get_daily_report(state: tauri::State<'_, AppState>) -> Result<DailyReport, String> {
    let db = &state.db;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let credits = db.get_today_credits().map_err(|e| e.to_string())?;
    let co2_saved = db.get_today_co2_saved().map_err(|e| e.to_string())?;
    let actions = db.get_today_action_count().map_err(|e| e.to_string())?;
    let snapshots = db.get_today_snapshot_count().map_err(|e| e.to_string())?;
    let wallet_row = db.get_wallet().map_err(|e| e.to_string())?;

    // Average CPU from today's summary
    let avg_cpu = db.get_today_summary()
        .map_err(|e| e.to_string())?
        .map(|s| s.cpu_usage)
        .unwrap_or(0.0);

    // Session carbon from latest snapshot
    let session_co2 = state.snapshot.lock().map_err(|e| e.to_string())?
        .as_ref()
        .map(|s| s.carbon.total_co2_grams)
        .unwrap_or(0.0);

    Ok(DailyReport {
        date: today,
        credits_earned: credits,
        co2_prevented_grams: co2_saved,
        actions_taken: actions,
        snapshots_collected: snapshots,
        avg_cpu_usage: (avg_cpu * 10.0).round() / 10.0,
        current_streak: wallet_row.current_streak,
        longest_streak: wallet_row.longest_streak,
        session_co2_grams: (session_co2 * 100.0).round() / 100.0,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));

            println!("[DWI] Data directory: {:?}", data_dir);

            let db = Arc::new(
                Database::init(&data_dir).expect("Failed to initialize DWI database"),
            );

            let snapshot: Arc<Mutex<Option<SystemSnapshot>>> = Arc::new(Mutex::new(None));

            app.manage(AppState {
                db: Arc::clone(&db),
                snapshot: Arc::clone(&snapshot),
            });

            daemon::spawn_daemon(Arc::clone(&db), Arc::clone(&snapshot));
            println!("[DWI] Background daemon started — polling every 5s");
            println!("[DWI] Eco-Credit wallet initialized");
            println!("[DWI] Cloud Cleaner ready");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_status,
            get_metric_history,
            get_metric_count,
            get_wallet_status,
            award_eco_credits,
            scan_duplicates,
            get_daily_report,
            redeem_credits,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DWI");
}
