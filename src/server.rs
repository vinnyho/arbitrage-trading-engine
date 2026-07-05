use crate::metrics::Metrics;
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub async fn run(metrics: Arc<Metrics>, host: String, port: u16) -> Result<(), anyhow::Error> {
    let app = Router::new()
        .route("/", get(dashboard))
        .route("/api/stats", get(stats))
        .route("/metrics", get(prometheus))
        .with_state(metrics);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("dashboard listening on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn dashboard() -> impl IntoResponse {
    Html(DASHBOARD_HTML)
}

async fn stats(State(metrics): State<Arc<Metrics>>) -> impl IntoResponse {
    Json(metrics.snapshot())
}

async fn prometheus(State(metrics): State<Arc<Metrics>>) -> impl IntoResponse {
    metrics.snapshot().to_prometheus()
}

const DASHBOARD_HTML: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>Arb Engine Dashboard</title>
<style>
  :root { color-scheme: dark; }
  body { font-family: ui-monospace, monospace; background: #0b0d12; color: #d8dee9; margin: 0; padding: 2rem; }
  h1 { font-size: 1.1rem; color: #8fd6ff; margin-bottom: 1.5rem; font-weight: 500; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 1rem; }
  .card { background: #151922; border: 1px solid #262b36; border-radius: 8px; padding: 1rem 1.2rem; }
  .label { font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.05em; color: #7c8493; margin-bottom: 0.4rem; }
  .value { font-size: 1.5rem; color: #eef2f7; }
  .sub { font-size: 0.75rem; color: #7c8493; margin-top: 0.3rem; }
</style>
</head>
<body>
<h1>kalshi &times; polymarket arb engine &mdash; live</h1>
<div class="grid" id="grid"></div>
<script>
function fmtUptime(s) {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = Math.floor(s % 60);
  return h + "h " + m + "m " + sec + "s";
}

function card(label, value, sub) {
  return '<div class="card"><div class="label">' + label + '</div><div class="value">' + value + '</div>'
    + (sub ? '<div class="sub">' + sub + '</div>' : '') + '</div>';
}

async function refresh() {
  const res = await fetch('/api/stats');
  const s = await res.json();
  document.getElementById('grid').innerHTML =
    card('uptime', fmtUptime(s.uptime_secs)) +
    card('kalshi messages', s.kalshi_messages, s.kalshi_messages_per_sec.toFixed(1) + '/sec avg') +
    card('polymarket messages', s.polymarket_messages, s.polymarket_messages_per_sec.toFixed(1) + '/sec avg') +
    card('arb signals detected', s.arb_signals) +
    card('kalshi orders placed', s.kalshi_orders) +
    card('polymarket orders placed', s.polymarket_orders) +
    card('detection latency p50 / p99', (s.detection_latency_us.p50/1000).toFixed(2) + ' / ' + (s.detection_latency_us.p99/1000).toFixed(2) + ' ms') +
    card('kalshi order latency p50 / p99', (s.kalshi_order_latency_us.p50/1000).toFixed(1) + ' / ' + (s.kalshi_order_latency_us.p99/1000).toFixed(1) + ' ms') +
    card('polymarket order latency p50 / p99', (s.polymarket_order_latency_us.p50/1000).toFixed(1) + ' / ' + (s.polymarket_order_latency_us.p99/1000).toFixed(1) + ' ms');
}

refresh();
setInterval(refresh, 2000);
</script>
</body>
</html>
"#;
