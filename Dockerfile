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

FROM chef AS planner
COPY backend/ ./backend/
WORKDIR /app/backend
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build backend (dependencies cached separately from source)
FROM chef AS backend-build
COPY --from=planner /app/backend/recipe.json /app/backend/recipe.json
WORKDIR /app/backend
RUN cargo chef cook --release --recipe-path recipe.json

COPY backend/ ./
# Seed data is embedded at compile time via include_str!() with paths
# relative to src/ (e.g., ../../data/cups.json), so data/ must exist
# at the expected location relative to the backend workspace.
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
COPY --from=backend-build /app/backend/target/release/beerio-kart ./beerio-kart

# Copy the built frontend
COPY --from=frontend-build /app/frontend/dist/ ./static/

# Create directories for runtime data
RUN mkdir -p /app/db /app/uploads

EXPOSE 3000

ENV DATABASE_URL="sqlite:///app/db/beerio-kart.db?mode=rwc"
ENV STATIC_DIR="/app/static"
ENV UPLOAD_DIR="/app/uploads"
ENV RUST_LOG="info"

CMD ["./beerio-kart"]
