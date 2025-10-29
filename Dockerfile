# Multi-stage build for Rust application
FROM rust:1.82-slim AS builder

# Install nightly Rust for edition 2024 support
RUN rustup install nightly && rustup default nightly

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml ./
COPY Cargo.lock* ./

# Copy source code
COPY src ./src
COPY scripts ./scripts
COPY configs ./configs

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false vaultkeeper

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/stablecoin-backend /app/stablecoin-backend

# Copy configs
COPY --from=builder /app/configs ./configs

# Change ownership to app user
RUN chown -R vaultkeeper:vaultkeeper /app

# Switch to app user
USER vaultkeeper

# Expose port (if needed for health checks)
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD /app/stablecoin-backend --help || exit 1

# Default command
CMD ["/app/stablecoin-backend"]
