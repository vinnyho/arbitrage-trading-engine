use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const LATENCY_SAMPLE_CAP: usize = 2000;

pub struct Metrics {
    start: Instant,
    kalshi_messages: AtomicU64,
    polymarket_messages: AtomicU64,
    arb_signals: AtomicU64,
    kalshi_orders: AtomicU64,
    polymarket_orders: AtomicU64,
    detection_latency_us: Mutex<VecDeque<u64>>,
    kalshi_order_latency_us: Mutex<VecDeque<u64>>,
    polymarket_order_latency_us: Mutex<VecDeque<u64>>,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Metrics {
            start: Instant::now(),
            kalshi_messages: AtomicU64::new(0),
            polymarket_messages: AtomicU64::new(0),
            arb_signals: AtomicU64::new(0),
            kalshi_orders: AtomicU64::new(0),
            polymarket_orders: AtomicU64::new(0),
            detection_latency_us: Mutex::new(VecDeque::with_capacity(LATENCY_SAMPLE_CAP)),
            kalshi_order_latency_us: Mutex::new(VecDeque::with_capacity(LATENCY_SAMPLE_CAP)),
            polymarket_order_latency_us: Mutex::new(VecDeque::with_capacity(LATENCY_SAMPLE_CAP)),
        })
    }

    pub fn record_kalshi_message(&self) {
        self.kalshi_messages.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_polymarket_message(&self) {
        self.polymarket_messages.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_arb_signal(&self, latency: Duration) {
        self.arb_signals.fetch_add(1, Ordering::Relaxed);
        push_sample(&self.detection_latency_us, latency.as_micros() as u64);
    }

    pub fn record_kalshi_order(&self, latency: Duration) {
        self.kalshi_orders.fetch_add(1, Ordering::Relaxed);
        push_sample(&self.kalshi_order_latency_us, latency.as_micros() as u64);
    }

    pub fn record_polymarket_order(&self, latency: Duration) {
        self.polymarket_orders.fetch_add(1, Ordering::Relaxed);
        push_sample(&self.polymarket_order_latency_us, latency.as_micros() as u64);
    }

    pub fn snapshot(&self) -> Snapshot {
        let uptime_secs = self.start.elapsed().as_secs_f64();
        let kalshi_messages = self.kalshi_messages.load(Ordering::Relaxed);
        let polymarket_messages = self.polymarket_messages.load(Ordering::Relaxed);

        Snapshot {
            uptime_secs,
            kalshi_messages,
            polymarket_messages,
            kalshi_messages_per_sec: rate(kalshi_messages, uptime_secs),
            polymarket_messages_per_sec: rate(polymarket_messages, uptime_secs),
            arb_signals: self.arb_signals.load(Ordering::Relaxed),
            kalshi_orders: self.kalshi_orders.load(Ordering::Relaxed),
            polymarket_orders: self.polymarket_orders.load(Ordering::Relaxed),
            detection_latency_us: percentiles(&self.detection_latency_us),
            kalshi_order_latency_us: percentiles(&self.kalshi_order_latency_us),
            polymarket_order_latency_us: percentiles(&self.polymarket_order_latency_us),
        }
    }
}

fn push_sample(buf: &Mutex<VecDeque<u64>>, value: u64) {
    let mut buf = buf.lock().unwrap();
    if buf.len() >= LATENCY_SAMPLE_CAP {
        buf.pop_front();
    }
    buf.push_back(value);
}

fn rate(count: u64, uptime_secs: f64) -> f64 {
    if uptime_secs <= 0.0 {
        0.0
    } else {
        count as f64 / uptime_secs
    }
}

fn percentiles(buf: &Mutex<VecDeque<u64>>) -> Percentiles {
    let mut samples: Vec<u64> = buf.lock().unwrap().iter().copied().collect();
    if samples.is_empty() {
        return Percentiles { p50: 0, p99: 0 };
    }
    samples.sort_unstable();
    let p50 = samples[(samples.len() - 1) * 50 / 100];
    let p99 = samples[(samples.len() - 1) * 99 / 100];
    Percentiles { p50, p99 }
}

#[derive(Serialize)]
pub struct Percentiles {
    pub p50: u64,
    pub p99: u64,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub uptime_secs: f64,
    pub kalshi_messages: u64,
    pub polymarket_messages: u64,
    pub kalshi_messages_per_sec: f64,
    pub polymarket_messages_per_sec: f64,
    pub arb_signals: u64,
    pub kalshi_orders: u64,
    pub polymarket_orders: u64,
    pub detection_latency_us: Percentiles,
    pub kalshi_order_latency_us: Percentiles,
    pub polymarket_order_latency_us: Percentiles,
}

impl Snapshot {
    pub fn to_prometheus(&self) -> String {
        let mut out = String::new();
        gauge(&mut out, "trading_uptime_seconds", "Seconds since the process started", self.uptime_secs);
        counter(&mut out, "trading_kalshi_messages_total", "Total ticker messages received from Kalshi", self.kalshi_messages);
        counter(&mut out, "trading_polymarket_messages_total", "Total book/price update messages received from Polymarket", self.polymarket_messages);
        counter(&mut out, "trading_arb_signals_total", "Total arbitrage signals detected", self.arb_signals);
        counter(&mut out, "trading_kalshi_orders_total", "Total Kalshi order attempts", self.kalshi_orders);
        counter(&mut out, "trading_polymarket_orders_total", "Total Polymarket order attempts", self.polymarket_orders);
        summary(&mut out, "trading_arb_detection_latency_seconds", "Latency from price update to arb signal emission", &self.detection_latency_us);
        summary(&mut out, "trading_kalshi_order_latency_seconds", "Kalshi order HTTP round-trip latency", &self.kalshi_order_latency_us);
        summary(&mut out, "trading_polymarket_order_latency_seconds", "Polymarket order HTTP round-trip latency", &self.polymarket_order_latency_us);
        out
    }
}

fn gauge(out: &mut String, name: &str, help: &str, value: f64) {
    out.push_str(&format!("# HELP {name} {help}\n# TYPE {name} gauge\n{name} {value}\n\n"));
}

fn counter(out: &mut String, name: &str, help: &str, value: u64) {
    out.push_str(&format!("# HELP {name} {help}\n# TYPE {name} counter\n{name} {value}\n\n"));
}

fn summary(out: &mut String, name: &str, help: &str, p: &Percentiles) {
    let p50_secs = p.p50 as f64 / 1_000_000.0;
    let p99_secs = p.p99 as f64 / 1_000_000.0;
    out.push_str(&format!(
        "# HELP {name} {help}\n# TYPE {name} summary\n{name}{{quantile=\"0.5\"}} {p50_secs}\n{name}{{quantile=\"0.99\"}} {p99_secs}\n\n"
    ));
}
