use crate::types::OrderBook;
use futures_util::SinkExt;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};
#[derive(Debug, Deserialize)]
struct PolymarketBook {
    asset_id: String,
    bids: Vec<PriceLevel>,
    asks: Vec<PriceLevel>,
    event_type: String,
}

#[derive(Debug, Deserialize)]
struct PriceLevel {
    price: String,
    size: String,
}

pub async fn connect(books: &Arc<Mutex<HashMap<String, OrderBook>>>) -> Result<(), anyhow::Error> {
    let (connection, _) =
        connect_async("wss://ws-subscriptions-clob.polymarket.com/ws/market").await?;

    let (mut write, mut read) = connection.split();

    let subscription = serde_json::json!({
        "assets_ids": ["94603648636330087039501304492699481091005420017442244191603206509188088089447"],

        "type": "market",
        "custom_feature_enabled": true
    });

    write
        .send(Message::Text(subscription.to_string().into()))
        .await?;

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            if write.send(Message::Ping(vec![].into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(msg) = read.next().await {
        if let Ok(Message::Text(text)) = msg {
            if let Ok(events) = serde_json::from_str::<Vec<PolymarketBook>>(&text) {
                for event in events {
                    if event.event_type == "book" || event.event_type == "price_change" {
                        let best_bid = event.bids.last().and_then(|l| l.price.parse::<f64>().ok());
                        let best_ask = event.asks.last().and_then(|l| l.price.parse::<f64>().ok());
                        println!(
                            "asset: {} | bid: {:?} | ask: {:?}",
                            event.asset_id, best_bid, best_ask
                        );
                        let mut books = books.lock().await;
                        books.insert(
                            event.asset_id.clone(),
                            OrderBook {
                                market_id: event.asset_id,
                                best_bid,
                                best_ask,
                            },
                        );
                    }
                }
            }
        }
    }
    Ok(())
}
