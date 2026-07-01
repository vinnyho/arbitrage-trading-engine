use crate::discovery::MarketPair;
use crate::types::OrderBook;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

const MIN_SPREAD: f64 = 0.03;

pub async fn run(
    pairs: Vec<MarketPair>,
    kalshi_books: Arc<Mutex<HashMap<String, OrderBook>>>,
    poly_books: Arc<Mutex<HashMap<String, OrderBook>>>,
) {
    loop {
        let kalshi = kalshi_books.lock().await;
        let poly = poly_books.lock().await;

        for pair in &pairs {
            let k = kalshi.get(&pair.kalshi_ticker);
            let p = poly.get(&pair.polymarket_token_id);

            if let (Some(k), Some(p)) = (k, p) {
                if let (Some(k_ask), Some(p_bid)) = (k.best_ask, p.best_bid) {
                    let spread = p_bid - k_ask;
                    if spread > MIN_SPREAD {
                        println!(
                            "ARB buy-kalshi-sell-poly | {} | buy {:.3} sell {:.3} spread {:.4}",
                            pair.canonical_id, k_ask, p_bid, spread
                        );
                    }
                }

                if let (Some(p_ask), Some(k_bid)) = (p.best_ask, k.best_bid) {
                    let spread = k_bid - p_ask;
                    if spread > MIN_SPREAD {
                        println!(
                            "ARB buy-poly-sell-kalshi | {} | buy {:.3} sell {:.3} spread {:.4}",
                            pair.canonical_id, p_ask, k_bid, spread
                        );
                    }
                }
            }
        }

        drop(kalshi);
        drop(poly);

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
