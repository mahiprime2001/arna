# ---- build ----
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/arna-backend /usr/local/bin/arna-backend
ENV PORT=8081
ENV RUST_LOG=info
EXPOSE 8081
CMD ["arna-backend"]
