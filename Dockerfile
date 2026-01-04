# Build stage
FROM rust:1.88-bookworm AS builder

# Install protobuf compiler and musl tools for static linking
RUN apt-get update && apt-get install -y protobuf-compiler musl-tools && rm -rf /var/lib/apt/lists/*

# Add musl target
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock* ./
COPY build.rs ./
COPY packages/logi-proto/proto ./packages/logi-proto/proto

# Create dummy src for dependency caching
RUN mkdir -p src/proto && echo "fn main() {}" > src/main.rs

# Build dependencies (cached layer)
RUN cargo build --release --target x86_64-unknown-linux-musl && rm -rf src

# Copy actual source
COPY src ./src

# Build application
RUN touch src/main.rs && cargo build --release --target x86_64-unknown-linux-musl

# Runtime stage - scratch for minimal image
FROM scratch

# Copy CA certificates for HTTPS
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/rust-logi /rust-logi

ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=8080

EXPOSE 8080

ENTRYPOINT ["/rust-logi"]
