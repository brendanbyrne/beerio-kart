# Frontend

React web app for Beerio Kart, built with [Vite](https://vite.dev/).

## Tech

- **Framework:** React + TypeScript
- **Bundler:** Vite
- **Package manager:** Bun
- **Styling:** Tailwind CSS

## Running

```sh
bun install
bun dev
```

Dev server starts on `http://localhost:5173`. API requests to `/api` are proxied to the backend at `http://localhost:3000`.

## Reference device

UI mockups and layout work target the **Pixel 9 Pro** as the reference phone. See [`../docs/design.md`](../docs/design.md) for details.

## Required reading

- **[`../docs/design.md`](../docs/design.md)** — Architecture, principles, UI screens, user workflows. The "High Level Principles" and "Design Goals" sections in particular shape every UI decision.
- **[`../docs/api-contract.md`](../docs/api-contract.md)** — Wire-format conventions for talking to the backend: error codes, ETag polling, idempotency keys, time format, auth refresh flow.

A frontend-specific coding standard (parallel to `docs/coding-standards/` for the backend) is not yet written. When the React work picks up in earnest, that doc will live at `docs/coding-standards/frontend.md`.
