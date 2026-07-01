use crate::types::OrderBook;
use futures_util::SinkExt;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Deserialize)]
struct PolymarketEvent {
    event_type: String,
    asset_id: String,
    bids: Option<Vec<PriceLevel>>,
    asks: Option<Vec<PriceLevel>>,
    best_bid: Option<String>,
    best_ask: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PriceLevel {
    price: String,
    #[allow(dead_code)]
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
            println!("RAW: {}", text);

            if let Ok(events) = serde_json::from_str::<Vec<PolymarketEvent>>(&text) {
                for event in events {
                    let (best_bid, best_ask) = match event.event_type.as_str() {
                        "book" | "price_change" => {
                            let bid = event
                                .bids
                                .as_ref()
                                .and_then(|b| b.last())
                                .and_then(|l| l.price.parse::<f64>().ok());
                            let ask = event
                                .asks
                                .as_ref()
                                .and_then(|a| a.last())
                                .and_then(|l| l.price.parse::<f64>().ok());
                            (bid, ask)
                        }
                        "best_bid_ask" => {
                            let bid = event.best_bid.as_deref().and_then(|s| s.parse().ok());
                            let ask = event.best_ask.as_deref().and_then(|s| s.parse().ok());
                            (bid, ask)
                        }
                        other => {
                            println!("unknown event_type: {}", other);
                            continue;
                        }
                    };

                    println!(
                        "[{}] asset: {} | bid: {:?} | ask: {:?}",
                        event.event_type, event.asset_id, best_bid, best_ask
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
    Ok(())
}
