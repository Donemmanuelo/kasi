# Kasi Power

[![Rust Version](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/tools/install)
[![Platform](https://img.shields.io/badge/platform-docker-blue.svg)](https://www.docker.com/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](https://github.com/Donemmanuelo/kasi/blob/main/LICENSE)

**An intelligent, autonomous IoT platform for monitoring and remotely controlling hybrid energy systems.**

Kasi Power is a complete, end-to-end system designed to tackle real-world power challenges. It creates a smart, self-optimizing, and resilient energy grid by ingesting data from remote sites, enriching it with external intelligence like weather forecasts, and sending predictive control commands back to optimize performance, save fuel, and prevent outages.

## Core Features

*   **Remote Monitoring:** A Rust-based `firmware` simulator mimics a remote power site, sending real-time telemetry (voltage, fuel level, power load, active source, etc.) over MQTT.
*   **Smart & Predictive Control:** A cloud-based Rust `backend` service subscribes to the data, fetches live weather forecasts, and calculates predictive runtimes for both the battery and the diesel fuel tank.
*   **Autonomous Decision-Making:** The backend sends a complete "strategic command" to the firmware, which uses this advice to make its own tactical decisions, including switching power sources or performing an emergency shutdown.
*   **Live Dashboard:** All historical data is stored in a TimescaleDB database and is ready to be visualized on a professional Grafana dashboard.
*   **Scalable & Production-Ready Architecture:** The entire system is containerized with Docker and uses an MQTT message broker, making it robust, portable, and ready to scale.

## System Architecture

```mermaid
graph TD
    subgraph "Remote Site"
        F["Firmware (Rust Agent)"]
    end

    subgraph "Cloud Platform (Docker)"
        M("Mosquitto MQTT Broker")
        CB["Cloud Backend (Rust)"]
        DB[("TimescaleDB")]
        G["Grafana Dashboard"]
    end

    subgraph "External"
        API{{OpenWeatherMap API}}
        User(Operator)
    end

    F -- "Publishes Telemetry" --> M
    M -- "Forwards Telemetry" --> CB
    API -- "GET Weather" --> CB
    CB -- "Stores Data" --> DB
    CB -- "Publishes Command" --> M
    M -- "Forwards Command" --> F
    DB -- "Queries Data" --> G
    G -- "Displays Dashboard" --> User
```

## Technology Stack
- Core Language: Rust
  - Why: For its reliability, C++ level performance, and safe concurrency, making it perfect for both the embedded agent and the critical cloud service.
- Infrastructure: Docker & Docker Compose
  - Why: For a clean, reproducible, and portable environment that orchestrates all services.
- Key Services:
  - Mosquitto: A lightweight and reliable MQTT message broker for IoT communication.
- TimescaleDB (PostgreSQL): A high-performance time-series database for storing sensor data.
  - Grafana: A world-class open-source dashboard for data visualization.
- External API:
  - OpenWeatherMap: To provide live weather data for the smart-control logic.
## Getting Started
Follow these steps to get the entire Kasi Power platform running.
**Prerequisites**
- Docker & Docker Compose: Ensure both are installed and the Docker daemon is running.
- Rust: Install the Rust toolchain via rustup. https://www.rust-lang.org/tools/install
- System Dependencies (for Debian/Ubuntu): Install packages needed for compiling Rust crates.
```bash
sudo apt-get update && sudo apt-get install -y pkg-config libssl-dev
```
```
OpenWeatherMap API Key: Get a free API key from https://openweathermap.org/api.
```

## Installation & Setup

Clone the project or generate the files using the create_kasi_power_v3.sh script.
Navigate to the project directory:
```bash
cd kasi-power-v3
```

Configure your environment: Create a file named .env in the root of the project. This is a critical step.
```bash
nano .env
```

Add your database URL and API key to the file:
```bash
# .env
DATABASE_URL=postgres://postgres:kasi_power_pass@localhost:5432/postgres
OPENWEATHER_API_KEY=PASTE_YOUR_API_KEY_HERE
```

**Running the Platform**

* Manually
  
Start the infrastructure: Run Docker Compose to start the database, message broker, and dashboard. The -v flag is important on the first run.
```bash
docker-compose up -d --force-recreate -v
```

Run the application: You will need two separate terminals.
In Terminal 1, start the cloud backend:
```bash
cargo run -p cloud-backend
```

In Terminal 2, start the firmware simulator:
```bash
cargo run -p firmware
```

## Using the Platform (The UI)

To access the dashboard, you need the IP address of the machine running Docker (e.g., localhost or a VM's IP).
Access the Grafana Dashboard:
- Open your web browser and navigate to http://<YOUR_IP_ADDRESS>:3000.
- Log in with admin / admin.
- Add Data Source:
- Go to Configuration (gear icon) > Data Sources.
- Add a PostgreSQL data source.
- Host: timescaledb-v3:5432 (Use the service name from Docker Compose).
- Database: postgres
- User: postgres / Password: kasi_power_pass
- TLS/SSL Mode: disable
- Turn ON the TimescaleDB toggle.
- Click Save & test.
- Create a Panel:
- Create a new dashboard and add a visualization.
- Use the following SQL query to see your data:

```sql
SELECT
  time,
  voltage,
  fuel_level,
  load_watts,
  active_source
FROM power_events
WHERE
  $__timeFilter(time)
ORDER BY time
```
## Benchmaking
To know the performance of this program we use the benchmarking tool as shown below:
```bash
cargo bench -p cloud-backend
```
Here is an illustration of how the benchmark looks like:

![image](https://github.com/Donemmanuelo/kasi/blob/main/Screen%20Shot%202025-07-30%20at%2013.44.09.png)
## Project Structure

The project is a Rust workspace with two main packages and supporting configuration.
```
kasi-power-v3/
├── .env                  # Environment variables (API Key, DB URL)
├── .github/              # GitHub Actions CI/CD workflows
│   └── workflows/
│       └── ci.yml
├── packages/             # Contains all the Rust source code
│   ├── cloud-backend/    # The cloud service that makes predictions
│   └── firmware/         # The device simulator with autonomous logic
├── mosquitto/            # Configuration for the MQTT broker
├── docker-compose.yml    # Orchestrates all services
├── Cargo.toml            # The Rust workspace definition
└── README.md             # This documentation
```

## Roadmap (What's Next)

- Hardware Integration: Deploy the firmware onto a real Raspberry Pi with physical sensors.
- Advanced Analytics: Improve the prediction algorithms to account for historical load patterns and battery degradation.
- Fleet Management: Enhance the dashboard to monitor and command thousands of sites from a single map-based UI.

## License
This project is licensed under the [LICENSE](https://github.com/Donemmanuelo/kasi/blob/main/LICENSE)
