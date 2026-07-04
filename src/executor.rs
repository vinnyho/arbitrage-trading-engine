use tokio::sync::mpsc;
use crate::types::ArbSignal;

pub async fn run(mut rx: mpsc::UnboundedReceiver<ArbSignal>) {
    while let Some(signal) = rx.recv().await {
        println!("{:?}", signal);
    }
}
