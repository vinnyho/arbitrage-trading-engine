use tokio::sync::mpsc;
use crate::types::ArbSignal;
use rsa::RsaPrivateKey;
use std::collections::HashSet;
use std::sync::Arc;

pub async fn run(
    mut rx: mpsc::UnboundedReceiver<ArbSignal>,
    key_id: String,
    private_key: Arc<RsaPrivateKey>,
) {
    let mut live_trades: HashSet<String> = HashSet::new();

    while let Some(signal) = rx.recv().await {
        println!("{:?}", signal);
        if !live_trades.contains(&signal.canonical_id) {
            live_trades.insert(signal.canonical_id.clone());
        }
        
    }
}
