# --- Stage 1: The Builder ---
# We use the official Rust slim image, which has all build tools.
FROM rust:1-slim as builder

# ARG allows us to pass a variable from docker-compose.yml
ARG APP_NAME

# Install system dependencies required for compiling our crates (for OpenSSL).
RUN apt-get update && apt-get install -y pkg-config libssl-dev

WORKDIR /usr/src/kasi-power

# --- Optimized Caching Layer ---
# Copy only the manifests to cache dependencies separately from source code.
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./packages/firmware/Cargo.toml ./packages/firmware/Cargo.toml
COPY ./packages/cloud-backend/Cargo.toml ./packages/cloud-backend/Cargo.toml

# Create dummy source files to allow dependency-only build.
RUN mkdir -p ./packages/firmware/src && echo "fn main() {}" > ./packages/firmware/src/main.rs
RUN mkdir -p ./packages/cloud-backend/src && echo "fn main() {}" > ./packages/cloud-backend/src/main.rs

# THIS IS THE FIX: Create the dummy benchmark directory and file as well.
# The content doesn't matter, it just needs to exist to satisfy Cargo.
RUN mkdir -p ./packages/cloud-backend/benches && echo "fn main() {}" > ./packages/cloud-backend/benches/telemetry_benchmark.rs

# Build dependencies only. This layer will be cached by Docker.
RUN cargo build --release

# --- Actual Build ---
# Now copy the real source code, which will overwrite the dummy files.
COPY ./packages/firmware/src ./packages/firmware/src
COPY ./packages/cloud-backend/src ./packages/cloud-backend/src
# Copy the real benchmark code.
COPY ./packages/cloud-backend/benches ./packages/cloud-backend/benches

# Build the specific package passed in via the build argument.
RUN cargo build --release -p ${APP_NAME}


# --- Stage 2: The Runner ---
# We use a minimal Debian image for the final container.
FROM debian:bullseye-slim as runner

# Again, accept the app name as an argument.
ARG APP_NAME

# Install only the runtime dependencies needed (just OpenSSL).
RUN apt-get update && apt-get install -y openssl && rm -rf /var/lib/apt/lists/*

# Copy the specific compiled binary from the builder stage.
COPY --from=builder /usr/src/kasi-power/target/release/${APP_NAME} /usr/local/bin/${APP_NAME}

# Set the command that will be executed when the container starts.
CMD ["/usr/local/bin/${APP_NAME}"]
