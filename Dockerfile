# ── stage 1: build frontend ────────────────────────────────────────────────
FROM node:20-slim AS frontend
WORKDIR /webapp
COPY webapp/package*.json ./
RUN npm ci
COPY webapp/ ./
RUN npm run build

# ── stage 2: build backend ─────────────────────────────────────────────────
FROM rust:1.88-slim-bookworm AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev g++ && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
COPY --from=frontend /webapp/dist ./webapp/dist
RUN cargo build --release --bin superdupermemory

# ── stage 3: runtime ───────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/superdupermemory /usr/local/bin/superdupermemory
VOLUME ["/data"]
ENV SDM_DB_PATH=/data/memory.db
EXPOSE 3000
ENTRYPOINT ["superdupermemory"]
CMD ["serve-web"]
