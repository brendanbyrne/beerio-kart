# Frontend

React web app for Beerio Kart, built with [Vite](https://vite.dev/).

## Tech

- **Framework:** React + TypeScript — largest mobile-web ecosystem; camera API support.
- **Bundler:** Vite
- **Package manager:** Bun — drop-in npm replacement; faster installs and script running.
- **Styling:** Tailwind CSS — utility-first; fast iteration; mobile-first.

## Running

```sh
bun install
bun dev
```

Dev server starts on `http://localhost:5173`. API requests to `/api` are proxied to the backend at `http://localhost:3000`.

## Reference device

UI mockups and layout work target the **Pixel 9 Pro** as the reference phone. See [`../docs/user-workflows.md`](../docs/user-workflows.md) § 2 for details.

## Required reading

- **[`../docs/design.md`](../docs/design.md)** — Architecture, high-level principles, and design goals. The principles and goals in particular shape every UI decision.
- **[`../docs/user-workflows.md`](../docs/user-workflows.md)** — End-user flows and the screen-by-screen UI breakdown.
- **[`../docs/api-contract.md`](../docs/api-contract.md)** — Wire-format conventions for talking to the backend: error codes, ETag polling, idempotency keys, time format, auth refresh flow.

Frontend coding standards live in [`../docs/coding-standards/`](../docs/coding-standards): [`typescript.md`](../docs/coding-standards/typescript.md), [`react.md`](../docs/coding-standards/react.md), and [`tailwind.md`](../docs/coding-standards/tailwind.md).
