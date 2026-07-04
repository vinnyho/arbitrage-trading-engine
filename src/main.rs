mod discovery;
mod executor;
mod kalshi;
mod polymarket;
mod scanner;
mod types;
use crate::discovery::MarketPair;
use crate::types::{ArbSignal, OrderBook};
use rsa::{RsaPrivateKey, pkcs8::DecodePrivateKey};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // `cargo run -- discover` runs the daily match-list builder and exits.
    if std::env::args().nth(1).as_deref() == Some("discover") {
        if let Err(e) = discovery::run_discovery().await {
            println!("discovery error: {}", e);
        }
        return;
    }
    

    let key_id = std::env::var("KALSHI_API_KEY").unwrap();
    let key_path = std::env::var("KALSHI_KEY_PATH").unwrap();
    let pem_string = std::fs::read_to_string(key_path).unwrap();
    let private_key = Arc::new(RsaPrivateKey::from_pkcs8_pem(&pem_string).unwrap());

    let content = std::fs::read_to_string("pairs.json").unwrap();

    let pairs: Vec<MarketPair> = serde_json::from_str(&content).unwrap();
    let kalshi_books = Arc::new(Mutex::new(HashMap::<String, OrderBook>::new()));
    let polymarket_books = Arc::new(Mutex::new(HashMap::<String, OrderBook>::new()));

    let kb = Arc::clone(&kalshi_books);
    let (tx, rx) = mpsc::unbounded_channel::<ArbSignal>();
    tokio::spawn(executor::run(rx, key_id.clone(), Arc::clone(&private_key)));

    let kalshi_key_id = key_id.clone();
    let kalshi_private_key = Arc::clone(&private_key);
    tokio::spawn(async move {
        loop {
            if let Err(e) = kalshi::connect(&kb, &kalshi_key_id, &kalshi_private_key).await {
                println!("error: {}", e);
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
    let token_ids: Vec<String> = pairs
        .iter()
        .map(|p| p.polymarket_token_id.clone())
        .collect();

    let pb = Arc::clone(&polymarket_books);
    tokio::spawn(async move {
        loop {
            if let Err(e) = polymarket::connect(&pb, &token_ids).await {
                println!("error: {}", e);
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
    scanner::run(pairs, kalshi_books, polymarket_books, tx).await;
}
