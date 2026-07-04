use crate::discovery::MarketPair;
use crate::types::OrderBook;
use crate::types::{ArbSignal, Exchange};
use chrono::Utc;
use std::collections::{HashSet, HashMap};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{ Mutex, mpsc };

const MIN_SPREAD: f64 = 0.03;

fn log_arb(file: &mut std::fs::File, msg: &str) {
    let line = format!("{} | {}\n", Utc::now().format("%Y-%m-%d %H:%M:%S"), msg);
    print!("{}", line);
    file.write_all(line.as_bytes()).ok();
}

pub async fn run(
    pairs: Vec<MarketPair>,
    kalshi_books: Arc<Mutex<HashMap<String, OrderBook>>>,
    poly_books: Arc<Mutex<HashMap<String, OrderBook>>>,
    tx: mpsc::UnboundedSender<ArbSignal>
) {
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open("arb_log.txt")
        .unwrap();
    let mut active_arbs: HashSet<String> = HashSet::new();

    loop {
        let kalshi = kalshi_books.lock().await;
        let poly = poly_books.lock().await;

        for pair in &pairs {
            let k = kalshi.get(&pair.kalshi_ticker);
            let p = poly.get(&pair.polymarket_token_id);

            if let (Some(k), Some(p)) = (k, p) {
                if let (Some(k_ask), Some(p_bid)) = (k.best_ask, p.best_bid) {
                    let spread = p_bid - k_ask;
                    let key = format!("{}:buy-kalshi", pair.canonical_id);
                    if spread > MIN_SPREAD {
                        if !active_arbs.contains(&key) {
                            log_arb(
                                &mut log,
                                &format!(
                                    "ARB OPEN buy-kalshi-sell-poly | {} | buy {:.3} sell {:.3} spread {:.4}",
                                    pair.canonical_id, k_ask, p_bid, spread
                                ),
                            );
                            tx.send(ArbSignal {
                                canonical_id: pair.canonical_id.clone(),
                                kalshi_ticker: pair.kalshi_ticker.clone(),
                                polymarket_token_id: pair.polymarket_token_id.clone(),
                                buy_exchange: Exchange::Kalshi,
                                sell_exchange: Exchange::Polymarket,
                                buy_price: k_ask,
                                sell_price: p_bid,
                                spread,
                                size: 1.0,
                                detected_at: Utc::now(),
                            }).ok();
                            active_arbs.insert(key);
                        }
                    } else {
                        active_arbs.remove(&key);
                    }
                }

                if let (Some(p_ask), Some(k_bid)) = (p.best_ask, k.best_bid) {
                    let spread = k_bid - p_ask;
                    let key = format!("{}:buy-poly", pair.canonical_id);
                    if spread > MIN_SPREAD {
                        if !active_arbs.contains(&key) {
                            log_arb(
                                &mut log,
                                &format!(
                                    "ARB OPEN buy-poly-sell-kalshi | {} | buy {:.3} sell {:.3} spread {:.4}",
                                    pair.canonical_id, p_ask, k_bid, spread
                                ),
                            );
                            tx.send(ArbSignal {
                                canonical_id: pair.canonical_id.clone(),
                                kalshi_ticker: pair.kalshi_ticker.clone(),
                                polymarket_token_id: pair.polymarket_token_id.clone(),
                                buy_exchange: Exchange::Polymarket,
                                sell_exchange: Exchange::Kalshi,
                                buy_price: p_ask,
                                sell_price: k_bid,
                                spread,
                                size: 1.0,
                                detected_at: Utc::now(),
                            }).ok();
                            active_arbs.insert(key);
                        }
                    } else {
                        active_arbs.remove(&key);
                    }
                }
            }
        }

        drop(kalshi);
        drop(poly);

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
