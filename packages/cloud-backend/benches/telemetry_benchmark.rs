use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde::Deserialize;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// We must copy the structs from main.rs into the benchmark
// so that the benchmark can be compiled independently.
#[derive(Deserialize, Debug, Clone, PartialEq)]
enum PowerMode { Eco, Normal, Conservative }

#[derive(Deserialize, Debug, Clone, PartialEq)]
enum ActiveSource { Battery, Diesel, Shutdown }

#[derive(Deserialize, Debug)]
struct PowerTelemetry {
    site_id: Uuid,
    timestamp_utc: DateTime<Utc>,
    active_source: ActiveSource,
    battery_voltage: f32,
    fuel_level_percent: f32,
    current_load_watts: f32,
}

// This is our benchmark function
fn deserialize_telemetry_benchmark(c: &mut Criterion) {
    let payload = br#"{
        "site_id": "00000000-0000-0000-0000-000000000001",
        "timestamp_utc": "2024-10-27T10:00:00Z",
        "active_source": "Battery",
        "battery_voltage": 12.5,
        "fuel_level_percent": 85.0,
        "current_load_watts": 150.0
    }"#;

    c.bench_function("telemetry deserialize", |b| {
        b.iter(|| serde_json::from_slice::<PowerTelemetry>(black_box(payload)))
    });
}

criterion_group!(benches, deserialize_telemetry_benchmark);
criterion_main!(benches);
