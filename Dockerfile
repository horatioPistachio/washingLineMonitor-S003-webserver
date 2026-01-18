FROM rust:1.92 as builder

WORKDIR /app

# Copy only dependency manifests first
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (this layer is cached unless Cargo.toml changes)
RUN cargo build --release

# Remove dummy source
RUN rm -rf src

# Copy real source code
COPY src ./src
COPY Rocket.toml ./

# Build actual application
# Touch src to force rebuild (cargo might think nothing changed)
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/washingLineMonitor-S003-webserver ./app
COPY Rocket.toml ./Rocket.toml

EXPOSE 8000

CMD ["./app"]