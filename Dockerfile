# --- Stage 1: The Builder ---
# THIS IS THE DEFINITIVE FIX: Use the correct, official image tag from Docker Hub.
FROM rust:1-slim as builder

ARG APP_NAME

RUN apt-get update && apt-get install -y pkg-config libssl-dev

WORKDIR /usr/src/kasi-power

# --- Optimized Caching Layer ---
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./packages/firmware/Cargo.toml ./packages/firmware/Cargo.toml
COPY ./packages/cloud-backend/Cargo.toml ./packages/cloud-backend/Cargo.toml
RUN mkdir -p ./packages/firmware/src && echo "fn main() {}" > ./packages/firmware/src/main.rs
RUN mkdir -p ./packages/cloud-backend/src && echo "fn main() {}" > ./packages/cloud-backend/src/main.rs
RUN mkdir -p ./packages/cloud-backend/benches && echo "fn main() {}" > ./packages/cloud-backend/benches/telemetry_benchmark.rs
RUN cargo build --release

# --- Actual Build ---
COPY ./packages/firmware/src ./packages/firmware/src
COPY ./packages/cloud-backend/src ./packages/cloud-backend/src
COPY ./packages/cloud-backend/benches ./packages/cloud-backend/benches
RUN cargo build --release -p ${APP_NAME}


# --- Stage 2: The Runner ---
FROM debian:bullseye-slim as runner

ARG APP_NAME

RUN apt-get update && apt-get install -y openssl && rm -rf /var/lib/apt/lists/*

# Copy the specific compiled binary from the builder stage.
COPY --from=builder /usr/src/kasi-power/target/release/${APP_NAME} /usr/local/bin/${APP_NAME}

# No CMD or ENTRYPOINT here. This is handled by docker-compose.yml.
