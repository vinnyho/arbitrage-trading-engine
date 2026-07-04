pub struct OrderBook {
    pub market_id: String,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
}

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub enum Exchange {
    Kalshi,
    Polymarket,
}

#[derive(Debug, Clone)]
pub struct ArbSignal {
    pub canonical_id: String,
    pub kalshi_ticker: String,
    pub polymarket_token_id: String,
    pub buy_exchange: Exchange,
    pub sell_exchange: Exchange,
    pub buy_price: f64,
    pub sell_price: f64,
    pub spread: f64,
    pub size: f64,
    pub detected_at: DateTime<Utc>,
}
