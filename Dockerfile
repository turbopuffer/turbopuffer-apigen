FROM rust:1.87-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work

COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /work/target/release/turbopuffer-apigen /usr/local/bin/turbopuffer-apigen

WORKDIR /work

ENTRYPOINT ["turbopuffer-apigen"]
