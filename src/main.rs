mod kalshi;
mod polymarket;
mod types;
use crate::types::OrderBook;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let kalshi_books = Arc::new(Mutex::new(HashMap::<String, OrderBook>::new()));
    let polymarket_books = Arc::new(Mutex::new(HashMap::<String, OrderBook>::new()));

    /* if let Err(e) = kalshi::connect(&kalshi_books).await {
        println!("error: {}", e);
    }*/
    if let Err(e) = polymarket::connect(&polymarket_books).await {
        println!("error: {}", e);
    }
}
