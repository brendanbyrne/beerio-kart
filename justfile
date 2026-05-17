# Beerio Kart development recipes

set dotenv-load := true

# Start backend and frontend in dev mode (run in separate terminals)
dev:
    #!/usr/bin/env bash
    echo "Starting backend and frontend in parallel..."
    echo "Backend: http://localhost:3000"
    echo "Frontend: http://localhost:5173"
    trap 'kill 0' EXIT
    cargo run &
    (cd frontend && bun run dev) &
    wait

# Run backend tests
test:
    cargo test

# Format Rust code (requires nightly rustfmt — see README "Setup")
fmt:
    cargo +nightly fmt

# One-shot scaffold for a new table — hand-edit afterward, do not re-run on existing entities (clobbers hand-edits)
entities-bootstrap:
    cd backend && sea-orm-cli generate entity -o src/entities

# Production build (backend + frontend)
build:
    cargo build --release
    cd frontend && bun run build

# Generate HTML coverage report and print path
coverage:
    cargo llvm-cov --workspace --html
    @echo "Open target/llvm-cov/html/index.html"

# Quick text summary of coverage
coverage-summary:
    cargo llvm-cov --workspace --summary-only

# Generate lcov.info file (what CI uses)
coverage-lcov:
    cargo llvm-cov --workspace --lcov --output-path lcov.info
