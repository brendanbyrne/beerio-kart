# Backend

Rust web server for Beerio Kart, built with [Axum](https://github.com/tokio-rs/axum).

## Tech

- **Framework:** Axum (async web framework)
- **Runtime:** Tokio (async runtime)
- **Database:** SQLite (via rusqlite or sqlx — TBD)
- **Auth:** argon2 (password hashing) + JWT (session tokens)

## Running

```sh
cargo run
```

Server starts on `http://localhost:3000`.

## Project Structure

See [DESIGN.md](../docs/DESIGN.md) for the full backend source layout under `backend/src/`.