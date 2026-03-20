FROM rust:1.83 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/aibtc-rust /usr/local/bin/aibtc-rust
CMD ["aibtc-rust", "0x158fD65e5cEc0e7DAA84DDD0499a8CeAD2F5D0E5"]
