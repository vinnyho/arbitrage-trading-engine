use crate::metrics::Metrics;
use crate::types::OrderBook;
use futures_util::SinkExt;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Deserialize)]
struct BookEvent {
    event_type: String,
    asset_id: String,
    bids: Vec<PriceLevel>,
    asks: Vec<PriceLevel>,
}

#[derive(Debug, Deserialize)]
struct PriceLevel {
    price: String,
    size: String,
}

#[derive(Debug, Deserialize)]
struct PriceChangeMsg {
    price_changes: Vec<PriceChangeItem>,
}

#[derive(Debug, Deserialize)]
struct PriceChangeItem {
    asset_id: String,
    best_bid: String,
    best_ask: String,
}

#[derive(Debug, Deserialize)]
struct BestBidAskMsg {
    asset_id: String,
    best_bid: String,
    best_ask: String,
}

async fn store(
    books: &Arc<Mutex<HashMap<String, OrderBook>>>,
    metrics: &Arc<Metrics>,
    asset_id: String,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    bid_size: Option<f64>,
    ask_size: Option<f64>,
) {
    let mut books = books.lock().await;
    books.insert(
        asset_id.clone(),
        OrderBook {
            market_id: asset_id,
            best_bid,
            best_ask,
            bid_size,
            ask_size,
            received_at: Instant::now(),
        },
    );
    drop(books);
    metrics.record_polymarket_message();
}

pub async fn connect(
    books: &Arc<Mutex<HashMap<String, OrderBook>>>,
    token_ids: &[String],
    metrics: &Arc<Metrics>,
) -> Result<(), anyhow::Error> {
    let (connection, _) =
        connect_async("wss://ws-subscriptions-clob.polymarket.com/ws/market").await?;

    let (mut write, mut read) = connection.split();

    let subscription = serde_json::json!({
        "assets_ids": token_ids,
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
            if let Ok(events) = serde_json::from_str::<Vec<BookEvent>>(&text) {
                for event in events {
                    if event.event_type != "book" {
                        continue;
                    }
                    let best_bid = event.bids.last().and_then(|l| l.price.parse::<f64>().ok());
                    let best_ask = event.asks.last().and_then(|l| l.price.parse::<f64>().ok());
                    let bid_size = event.bids.last().and_then(|l| l.size.parse::<f64>().ok());
                    let ask_size = event.asks.last().and_then(|l| l.size.parse::<f64>().ok());
                    store(
                        books,
                        metrics,
                        event.asset_id,
                        best_bid,
                        best_ask,
                        bid_size,
                        ask_size,
                    )
                    .await;
                }
                continue;
            }

            let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };

            match val.get("event_type").and_then(|v| v.as_str()) {
                Some("price_change") => {
                    if let Ok(msg) = serde_json::from_value::<PriceChangeMsg>(val) {
                        for item in msg.price_changes {
                            let best_bid = item.best_bid.parse::<f64>().ok();
                            let best_ask = item.best_ask.parse::<f64>().ok();
                            store(books, metrics, item.asset_id, best_bid, best_ask, None, None)
                                .await;
                        }
                    }
                }
                Some("best_bid_ask") => {
                    if let Ok(msg) = serde_json::from_value::<BestBidAskMsg>(val) {
                        let best_bid = msg.best_bid.parse::<f64>().ok();
                        let best_ask = msg.best_ask.parse::<f64>().ok();
                        store(books, metrics, msg.asset_id, best_bid, best_ask, None, None).await;
                    }
                }
                Some("new_market")
                | Some("market_resolved")
                | Some("last_trade_price")
                | Some("tick_size_change") => {}
                _ => {}
            }
        }
    }
    Ok(())
}
