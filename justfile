# Beerio Kart development recipes

# Start backend and frontend in dev mode (run in separate terminals)
dev:
    #!/usr/bin/env bash
    echo "Starting backend and frontend in parallel..."
    echo "Backend: http://localhost:3000"
    echo "Frontend: http://localhost:5173"
    trap 'kill 0' EXIT
    (cd backend && cargo run) &
    (cd frontend && bun run dev) &
    wait

# Run backend tests
test:
    cd backend && cargo test

# Regenerate SeaORM entities from the database
entities:
    cd backend && sea-orm-cli generate entity -o src/entities --lib

# Production build (backend + frontend)
build:
    cd backend && cargo build --release
    cd frontend && bun run build
