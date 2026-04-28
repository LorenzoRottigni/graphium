FROM rust:1.88-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build -p graphium-examples --bin ecommerce --release --no-default-features --features macro

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/ecommerce /app/ecommerce

ENV ECOMMERCE_BIND=0.0.0.0:3000
EXPOSE 3000
CMD ["/app/ecommerce"]
