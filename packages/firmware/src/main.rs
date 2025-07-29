use serde::{Serialize, Deserialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time;
use rumqttc::{AsyncClient, MqttOptions, QoS, Packet};
use uuid::Uuid;
use rand::Rng;

// SHARED TYPES
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum PowerMode { Eco, Normal, Conservative }
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum WeatherCondition { Clear, Cloudy, Unknown }
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum ActiveSource { Battery, Diesel, Shutdown }

// TELEMETRY (Firmware -> Cloud)
#[derive(Serialize, Debug, Clone)]
struct PowerState {
    site_id: Uuid,
    timestamp_utc: String,
    active_source: ActiveSource,
    battery_voltage: f32,
    is_generator_on: bool,
    fuel_level_percent: f32,
    current_load_watts: f32,
}

// COMMAND (Cloud -> Firmware)
#[derive(Deserialize, Debug, Clone)]
struct StrategicCommand {
    mode: PowerMode,
    weather_forecast: WeatherCondition,
    predicted_battery_hours: f32,
    predicted_diesel_hours: f32,
}

const FUEL_CONSUMPTION_RATE_PER_SECOND: f32 = 0.01; // ~36% per hour
const SITE_ID: &str = "00000000-0000-0000-0000-000000000001";

#[tokio::main]
async fn main() {
    let site_id = Uuid::parse_str(SITE_ID).unwrap();
    println!("[System] Starting Kasi Power Firmware v3.0 for Site: {}", site_id);

    let mut mqttoptions = MqttOptions::new(format!("firmware-{}", site_id), "localhost", 1883);
    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    
    let command_topic = format!("kasi_power/sites/{}/commands", site_id);
    client.subscribe(&command_topic, QoS::AtLeastOnce).await.unwrap();
    println!("[MQTT] Subscribed to command topic.");

    // UPDATED: The firmware now holds the full strategic advice from the cloud
    let latest_strategy = Arc::new(Mutex::new(StrategicCommand {
        mode: PowerMode::Normal,
        weather_forecast: WeatherCondition::Unknown,
        predicted_battery_hours: 99.0,
        predicted_diesel_hours: 99.0,
    }));
    let strategy_clone = latest_strategy.clone();

    tokio::spawn(async move {
        loop {
            if let Ok(rumqttc::Event::Incoming(Packet::Publish(publish))) = eventloop.poll().await {
                if let Ok(cmd) = serde_json::from_slice::<StrategicCommand>(&publish.payload) {
                    println!("\n[CONTROL] New Strategy Received: {:?}\n", cmd);
                    *strategy_clone.lock().unwrap() = cmd;
                }
            }
        }
    });

    // Initialize physical state
    let mut voltage: f32 = 13.5;
    let mut fuel_level: f32 = 85.0;
    let mut active_source = ActiveSource::Battery;

    loop {
        let strategy = latest_strategy.lock().unwrap().clone();
        
        // Simulate a variable load for realism
        let load_watts = rand::thread_rng().gen_range(50.0..250.0);
        let voltage_drop_per_tick = load_watts / 5000.0; // Simplified discharge model
        
        // --- AUTONOMOUS DECISION TREE ---
        if active_source != ActiveSource::Shutdown {
            // 1. Check for emergency shutdown conditions
            if strategy.predicted_battery_hours < 1.0 && strategy.predicted_diesel_hours < 1.0 && strategy.weather_forecast == WeatherCondition::Cloudy {
                println!("[CRITICAL] All sources depleted and no sun expected. Performing emergency shutdown!");
                active_source = ActiveSource::Shutdown;
            }
            // 2. Check for mandatory source switching
            else if active_source == ActiveSource::Battery && strategy.predicted_battery_hours < 1.0 && strategy.predicted_diesel_hours > 1.0 {
                println!("[AUTONOMOUS] Battery depleted. Forcing switch to Diesel.");
                active_source = ActiveSource::Diesel;
            }
            else if active_source == ActiveSource::Diesel && strategy.predicted_diesel_hours < 1.0 && strategy.predicted_battery_hours > 1.0 {
                println!("[AUTONOMOUS] Diesel depleted. Forcing switch to Battery.");
                active_source = ActiveSource::Battery;
            }
            // 3. Normal operation based on PowerMode
            else {
                let threshold = match strategy.mode {
                    PowerMode::Eco => 11.5,
                    PowerMode::Normal => 11.8,
                    PowerMode::Conservative => 12.1,
                };
                if active_source == ActiveSource::Battery && voltage < threshold && fuel_level > 0.0 {
                    println!("[CONTROL] Low voltage in {:?} mode. Switching to Diesel.", strategy.mode);
                    active_source = ActiveSource::Diesel;
                } else if active_source == ActiveSource::Diesel && voltage > 13.8 {
                    println!("[CONTROL] Battery charged. Switching back to Battery.");
                    active_source = ActiveSource::Battery;
                }
            }
        }
        
        // --- UPDATE PHYSICAL STATE based on decision ---
        let is_generator_on = active_source == ActiveSource::Diesel;
        if is_generator_on {
            voltage += 0.2;
            fuel_level -= FUEL_CONSUMPTION_RATE_PER_SECOND * 2.0;
        } else if active_source == ActiveSource::Battery {
            voltage -= voltage_drop_per_tick;
        }
        voltage = voltage.clamp(10.5, 14.4);
        fuel_level = fuel_level.clamp(0.0, 100.0);
        
        // Publish the final state
        let state = PowerState {
            site_id,
            timestamp_utc: chrono::Utc::now().to_rfc3339(),
            active_source: active_source.clone(),
            battery_voltage: voltage,
            is_generator_on,
            fuel_level_percent: fuel_level,
            current_load_watts: load_watts,
        };
        
        let payload = serde_json::to_string(&state).unwrap();
        let telemetry_topic = format!("kasi_power/sites/{}/telemetry", site_id);
        client.publish(&telemetry_topic, QoS::AtLeastOnce, false, payload).await.unwrap();
        println!("Published: Source={:?}, V={:.2}, Fuel={:.1}%, Load={:.0}W", state.active_source, state.battery_voltage, state.fuel_level_percent, state.current_load_watts);
        
        time::sleep(Duration::from_secs(2)).await;
    }
}
