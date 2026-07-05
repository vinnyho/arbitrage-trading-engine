pub struct Config {
    pub min_spread: f64,
    pub reconnect_secs: u64,
    pub scan_interval_ms: u64,
    pub max_book_age_ms: u64,
    pub pairs_path: String,
    pub arb_log_path: String,
    pub db_path: String,
    pub dashboard_port: u16,
    pub dashboard_host: String,
}

fn parse_env_or_default<T: std::str::FromStr>(name: &str, default: T) -> T {
    match std::env::var(name) {
        Ok(v) => match v.parse() {
            Ok(parsed) => parsed,
            Err(_) => {
                println!("warning: {name}={v:?} failed to parse, using default");
                default
            }
        },
        Err(_) => default,
    }
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            min_spread: parse_env_or_default("MIN_ARB_SPREAD", 0.03),
            reconnect_secs: parse_env_or_default("RECONNECT_SECS", 10),
            scan_interval_ms: parse_env_or_default("SCAN_INTERVAL_MS", 100),
            max_book_age_ms: parse_env_or_default("MAX_BOOK_AGE_MS", 5000),
            pairs_path: std::env::var("PAIRS_PATH").unwrap_or_else(|_| "pairs.json".to_string()),
            arb_log_path: std::env::var("ARB_LOG_PATH")
                .unwrap_or_else(|_| "arb_log.txt".to_string()),
            db_path: std::env::var("DATABASE_PATH").unwrap_or_else(|_| "trades.db".to_string()),
            dashboard_port: parse_env_or_default("DASHBOARD_PORT", 3000),
            dashboard_host: std::env::var("DASHBOARD_HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
        }
    }
}
