pub struct Config {
    pub min_spread: f64,
    pub reconnect_secs: u64,
    pub scan_interval_ms: u64,
    pub pairs_path: String,
    pub arb_log_path: String,
    pub db_path: String,
    pub dashboard_port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            min_spread: std::env::var("MIN_ARB_SPREAD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.03),
            reconnect_secs: std::env::var("RECONNECT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            scan_interval_ms: std::env::var("SCAN_INTERVAL_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            pairs_path: std::env::var("PAIRS_PATH").unwrap_or_else(|_| "pairs.json".to_string()),
            arb_log_path: std::env::var("ARB_LOG_PATH")
                .unwrap_or_else(|_| "arb_log.txt".to_string()),
            db_path: std::env::var("DATABASE_PATH").unwrap_or_else(|_| "trades.db".to_string()),
            dashboard_port: std::env::var("DASHBOARD_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
        }
    }
}
