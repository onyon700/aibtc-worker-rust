FROM rust:1.83 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/aibtc-rust /usr/local/bin/aibtc-rust
CMD ["aibtc-rust", "0x3f81F760dc3f42D46A70C7707FA5A696567315A5"]
