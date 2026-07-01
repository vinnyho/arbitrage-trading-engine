pub struct OrderBook {
    pub market_id: String,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
}
