// packages/cloud-backend/src/main.rs (Corrected)
use serde::{Deserialize, Serialize};
use rumqttc::{AsyncClient, MqttOptions, QoS, Packet};
use sqlx::{PgPool, FromRow};
use std::{env, time::Duration};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::time;

// SHARED TYPES
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "power_mode", rename_all = "lowercase")]
enum PowerMode { Eco, Normal, Conservative }
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "weather_condition", rename_all = "lowercase")]
enum WeatherCondition { Clear, Cloudy, Unknown }
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "active_source", rename_all = "lowercase")]
enum ActiveSource { Battery, Diesel, Shutdown }

// TELEMETRY (Firmware -> Cloud)
#[derive(Deserialize, Debug)]
struct PowerTelemetry {
    site_id: Uuid,
    timestamp_utc: DateTime<Utc>,
    active_source: ActiveSource,
    battery_voltage: f32,
    fuel_level_percent: f32,
    current_load_watts: f32,
}

// COMMAND (Cloud -> Firmware)
#[derive(Serialize, Debug, Clone)]
struct StrategicCommand {
    mode: PowerMode,
    weather_forecast: WeatherCondition,
    predicted_battery_hours: f32,
    predicted_diesel_hours: f32,
}

#[derive(Deserialize, Debug)]
struct WeatherResponse { weather: Vec<Weather> }
#[derive(Deserialize, Debug)]
struct Weather { main: String }

#[derive(FromRow, Debug)]
struct AvgConsumption { avg_rate: Option<f64>, avg_load: Option<f64> }

const TOTAL_BATTERY_CAPACITY_WH: f32 = 5000.0;

async fn setup_database(db_pool: &PgPool) {
    sqlx::query("DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'power_mode') THEN CREATE TYPE power_mode AS ENUM ('eco', 'normal', 'conservative'); END IF; END $$;").execute(db_pool).await.unwrap();
    sqlx::query("DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'weather_condition') THEN CREATE TYPE weather_condition AS ENUM ('clear', 'cloudy', 'unknown'); END IF; END $$;").execute(db_pool).await.unwrap();
    sqlx::query("DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'active_source') THEN CREATE TYPE active_source AS ENUM ('battery', 'diesel', 'shutdown'); END IF; END $$;").execute(db_pool).await.unwrap();
    sqlx::query("CREATE EXTENSION IF NOT EXISTS timescaledb;").execute(db_pool).await.unwrap();
    
    // THIS IS THE CORRECTED SQL QUERY
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS power_events (
            time TIMESTAMPTZ NOT NULL,
            site_id UUID NOT NULL,
            active_source active_source NOT NULL,
            voltage REAL NOT NULL,
            fuel_level REAL NOT NULL,
            load_watts REAL NOT NULL
        );"
    ).execute(db_pool).await.unwrap();

    let _ = sqlx::query("SELECT create_hypertable('power_events', 'time', if_not_exists => TRUE);").execute(db_pool).await;
    println!("[DB] Database setup complete for v3.0.");
}

async fn get_weather_forecast(api_key: &str) -> (PowerMode, WeatherCondition) {
    let (lat, lon) = ("6.5244", "3.3792"); // Lagos
    let url = format!("https://api.openweathermap.org/data/2.5/weather?lat={}&lon={}&appid={}", lat, lon, api_key);
    if let Ok(res) = reqwest::get(&url).await {
        if let Ok(weather_res) = res.json::<WeatherResponse>().await {
            if let Some(weather) = weather_res.weather.first() {
                // println!("[Weather] Current weather: {}", weather.main); // This can be noisy
                let condition = match weather.main.as_str() { "Clear" | "Sunny" => WeatherCondition::Clear, "Clouds" | "Rain" => WeatherCondition::Cloudy, _ => WeatherCondition::Unknown };
                let mode = match condition { WeatherCondition::Clear => PowerMode::Eco, WeatherCondition::Cloudy => PowerMode::Conservative, _ => PowerMode::Normal };
                return (mode, condition);
            }
        }
    }
    (PowerMode::Normal, WeatherCondition::Unknown)
}

// THE ADVANCED PREDICTIVE ENGINE
async fn run_predictions(db_pool: &PgPool, site_id: Uuid) -> (f32, f32) {
    let latest_state_query = sqlx::query_as::<_, (f32, f32)>("SELECT voltage, fuel_level FROM power_events WHERE site_id = $1 ORDER BY time DESC LIMIT 1")
        .bind(site_id).fetch_optional(db_pool).await;
    let (current_voltage, current_fuel) = match latest_state_query { Ok(Some(state)) => state, _ => return (99.0, 99.0) };

    let consumption_query = sqlx::query_as::<_, AvgConsumption>(
        "WITH fuel_rates AS (
            SELECT 
                (lag(fuel_level) OVER w - fuel_level) / (EXTRACT(EPOCH FROM (time - lag(time) OVER w)) / 3600.0) as rate
            FROM power_events WHERE site_id = $1 AND active_source = 'diesel' AND time > now() - INTERVAL '7 days' WINDOW w AS (ORDER BY time)
        )
        SELECT AVG(rate) as avg_rate, (SELECT AVG(load_watts) FROM power_events WHERE site_id = $1 AND active_source = 'battery' AND time > now() - INTERVAL '1 day') as avg_load
        FROM fuel_rates WHERE rate > 0;"
    ).bind(site_id).fetch_one(db_pool).await;

    let mut predicted_diesel_hours = 99.0;
    let mut predicted_battery_hours = 99.0;

    if let Ok(consumption) = consumption_query {
        if let Some(avg_rate) = consumption.avg_rate {
            if avg_rate > 0.0 { predicted_diesel_hours = (current_fuel / avg_rate as f32).max(0.0); }
        }
        if let Some(avg_load) = consumption.avg_load {
            if avg_load > 0.0 {
                let state_of_charge = ((current_voltage - 11.5) / (13.8 - 11.5)).clamp(0.0, 1.0);
                let remaining_wh = TOTAL_BATTERY_CAPACITY_WH * state_of_charge;
                predicted_battery_hours = (remaining_wh / avg_load as f32).max(0.0);
            }
        }
    }
    
    println!("\n[PREDICTION] Site {}: Est. Battery Time: {:.1} hrs. Est. Diesel Time: {:.1} hrs.\n",
        site_id, predicted_battery_hours, predicted_diesel_hours);

    (predicted_battery_hours, predicted_diesel_hours)
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    println!("[System] Starting Kasi Power Cloud Backend v3.0 (Corrected)...");
    let db_pool = PgPool::connect(&env::var("DATABASE_URL").unwrap()).await.unwrap();
    setup_database(&db_pool).await;
    let api_key = env::var("OPENWEATHER_API_KEY").expect("OPENWEATHER_API_KEY must be set");

    // THIS IS THE CORRECTED LINE (removed 'mut')
    let mqttoptions = MqttOptions::new("kasi-backend-manager", "localhost", 1883);
    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    client.subscribe("kasi_power/sites/+/telemetry", QoS::AtLeastOnce).await.unwrap();
    println!("[MQTT] Subscribed to telemetry topic.");

    let known_site_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let client_clone = client.clone();
    let db_pool_clone = db_pool.clone();

    tokio::spawn(async move {
        loop {
            let (mode, weather) = get_weather_forecast(&api_key).await;
            let (battery_h, diesel_h) = run_predictions(&db_pool_clone, known_site_id).await;
            
            let command = StrategicCommand {
                mode,
                weather_forecast: weather,
                predicted_battery_hours: battery_h,
                predicted_diesel_hours: diesel_h,
            };
            
            let payload = serde_json::to_string(&command).unwrap();
            let command_topic = format!("kasi_power/sites/{}/commands", known_site_id);
            let _ = client_clone.publish(&command_topic, QoS::AtLeastOnce, false, payload).await;
            
            time::sleep(Duration::from_secs(60)).await;
        }
    });

    loop {
        if let Ok(rumqttc::Event::Incoming(Packet::Publish(publish))) = eventloop.poll().await {
            if let Ok(telemetry) = serde_json::from_slice::<PowerTelemetry>(&publish.payload) {
                let result = sqlx::query("INSERT INTO power_events (time, site_id, active_source, voltage, fuel_level, load_watts) VALUES ($1, $2, $3, $4, $5, $6)")
                    .bind(telemetry.timestamp_utc).bind(telemetry.site_id).bind(telemetry.active_source)
                    .bind(telemetry.battery_voltage).bind(telemetry.fuel_level_percent).bind(telemetry.current_load_watts)
                    .execute(&db_pool).await;
                if result.is_err() { eprintln!("[DB] Failed to insert data!"); }
            }
        }
    }
}
