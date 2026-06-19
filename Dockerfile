# ── stage 1: build frontend ────────────────────────────────────────────────
FROM node:20-slim AS frontend
WORKDIR /webapp
COPY webapp/package*.json ./
RUN npm ci
COPY webapp/ ./
RUN npm run build

# ── stage 2: build backend ─────────────────────────────────────────────────
# ort-sys pre-built ONNX Runtime binaries require glibc >= 2.38.
# Ubuntu 24.04 ships glibc 2.39; debian:bookworm only has 2.36.
FROM ubuntu:24.04 AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates pkg-config libssl-dev build-essential \
    && rm -rf /var/lib/apt/lists/*
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --profile minimal --default-toolchain 1.88.0
ENV PATH=/root/.cargo/bin:$PATH
WORKDIR /app
COPY . .
COPY --from=frontend /webapp/dist ./webapp/dist
RUN cargo build --release --bin superdupermemory

# ── stage 3: runtime ───────────────────────────────────────────────────────
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/superdupermemory /usr/local/bin/superdupermemory
VOLUME ["/data"]
ENV SDM_DB_PATH=/data/memory.db
EXPOSE 3000
ENTRYPOINT ["superdupermemory"]
CMD ["serve-web"]
