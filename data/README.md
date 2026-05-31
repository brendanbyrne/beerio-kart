# data

Seed data and runtime storage for the backend.

## Seed JSON

The `*.json` files are the canonical Mario Kart 8 Deluxe static game data. The
backend seeder loads them into the lookup tables on startup (see
[`backend/src/seed.rs`](../backend/src/seed.rs)); each file maps to one table:

| File | Table | Contents |
|------|-------|----------|
| `characters.json` | `characters` | Playable drivers |
| `bodies.json` | `bodies` | Kart bodies |
| `wheels.json` | `wheels` | Wheels |
| `gliders.json` | `gliders` | Gliders |
| `cups.json` | `cups` | Cups (track groupings) |
| `tracks.json` | `tracks` | Tracks, each linked to its cup |
| `drink_types.json` | `drink_types` | Default drink options (beer, sparkling water) |

These are checked in — they're reference data, not user data. To correct or
extend the game data, edit the JSON and then reset the local DB: the seeder
**skips any table that already has rows**, so edits don't take effect until that
table is empty again (delete `data/db/*` and restart — see below).

## Runtime directories (gitignored)

- `db/` — the SQLite database file lives here at runtime. Gitignored (`.gitkeep`
  preserves the directory). Delete its contents to reset local state; the schema
  is recreated from the consolidated migration and reseeded on next start.
- `uploads/` — user-uploaded run photos. Gitignored the same way.
