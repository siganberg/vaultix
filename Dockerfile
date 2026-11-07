# syntax=docker/dockerfile:1.7
# Cross-compiles natively on the build machine (no QEMU), so building an
# amd64 image on an Apple Silicon Mac takes about as long as a local build.
FROM --platform=$BUILDPLATFORM rust:1-slim-bookworm AS build
ARG TARGETARCH
ARG BUILDARCH
RUN set -eux; \
    case "$TARGETARCH" in \
      amd64) T=x86_64-unknown-linux-gnu;  GCC=gcc-x86-64-linux-gnu;  CC=x86_64-linux-gnu-gcc ;; \
      arm64) T=aarch64-unknown-linux-gnu; GCC=gcc-aarch64-linux-gnu; CC=aarch64-linux-gnu-gcc ;; \
      *) echo "unsupported arch $TARGETARCH"; exit 1 ;; \
    esac; \
    if [ "$TARGETARCH" != "$BUILDARCH" ]; then \
      apt-get update && apt-get install -y --no-install-recommends $GCC libc6-dev-${TARGETARCH}-cross && rm -rf /var/lib/apt/lists/*; \
    else CC=gcc; fi; \
    rustup target add $T; \
    echo "$T" > /target; echo "$CC" > /cc
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,id=vaultix-target-${TARGETARCH} \
    set -eux; T=$(cat /target); CC=$(cat /cc); \
    TU=$(echo "$T" | tr 'a-z-' 'A-Z_'); \
    export CARGO_TARGET_${TU}_LINKER=$CC "CC_$(echo $T | tr - _)=$CC"; \
    cargo build --release --locked -p vaultix-server --target "$T"; \
    cp "target/$T/release/vaultix-server" /vaultix-server

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
