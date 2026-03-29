# beerio-kart

We're really doing this thing.

## Running

```sh
bun install      # install root + frontend dependencies
bun run dev      # starts backend (port 3000) and frontend (port 5173) together
```

Open `http://localhost:5173` in your browser.

To run them separately:

```sh
cd backend && cargo run    # API server on port 3000
cd frontend && bun dev     # Dev server on port 5173
```
