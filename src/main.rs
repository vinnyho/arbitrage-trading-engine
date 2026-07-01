mod kalshi;
mod polymarket;
mod types;
use crate::types::OrderBook;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let kalshi_books = Arc::new(Mutex::new(HashMap::<String, OrderBook>::new()));
    let polymarket_books = Arc::new(Mutex::new(HashMap::<String, OrderBook>::new()));

    let kb = Arc::clone(&kalshi_books);
    /*
    tokio::spawn( async move {
        loop {
        if let Err(e) = kalshi::connect(&kb).await {
            println!("error: {}", e);
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
    });
    */
    let pb = Arc::clone(&polymarket_books);
    tokio::spawn(async move {
        loop {
            if let Err(e) = polymarket::connect(&pb).await {
                println!("error: {}", e);
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
