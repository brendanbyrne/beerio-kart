# Stage 1: Build frontend
FROM docker.io/oven/bun:1 AS frontend-build
WORKDIR /app

# Copy workspace root (package.json + bun.lock) and frontend package.json
# for dependency caching. Bun workspaces keep the lockfile at the root.
COPY package.json bun.lock ./
COPY frontend/package.json ./frontend/
RUN cd frontend && bun install --frozen-lockfile

COPY frontend/ ./frontend/
RUN cd frontend && bun run build

# Stage 2: Prepare Rust dependency cache with cargo-chef
FROM docker.io/library/rust:1-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# The Cargo workspace root is the repo root (a virtual manifest); the backend
# crate lives in backend/. chef must see the root Cargo.toml + Cargo.lock plus
# the member crates under backend/ to resolve the workspace.
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY backend/ ./backend/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build backend (dependencies cached separately from source)
FROM chef AS backend-build
COPY --from=planner /app/recipe.json /app/recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY Cargo.toml Cargo.lock ./
COPY backend/ ./backend/
# Seed data is embedded at compile time via include_str!() with paths
# relative to the source file (e.g., ../../data/cups.json from backend/src/),
# so data/ must exist at /app/data/ — the repo root inside the build context.
COPY data/ /app/data/
RUN cargo build --release

# Stage 4: Runtime
FROM docker.io/library/debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled binary
COPY --from=backend-build /app/target/release/beerio-kart ./beerio-kart

# Copy the built frontend
COPY --from=frontend-build /app/frontend/dist/ ./static/

# Create directories for runtime data and a non-root user to run the app.
# Running as root inside a container is a security risk — if the app were
# compromised, the attacker would have root privileges in the container.
RUN useradd --create-home --no-log-init appuser \
    && mkdir -p /app/db /app/uploads \
    && chown -R appuser:appuser /app

USER appuser

EXPOSE 3000

ENV DATABASE_URL="sqlite:///app/db/beerio-kart.db?mode=rwc"
ENV STATIC_DIR="/app/static"
ENV UPLOAD_DIR="/app/uploads"
ENV RUST_LOG="info"

CMD ["./beerio-kart"]
