# syntax=docker/dockerfile:1.7
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p vaultix-server && \
    cp target/release/vaultix-server /vaultix-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /vaultix-server /usr/local/bin/vaultix-server
ENV VAULTIX_DATA=/data \
    VAULTIX_BIND=0.0.0.0:8080 \
    RUST_LOG=info
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD ["vaultix-server", "health"]
ENTRYPOINT ["vaultix-server"]
