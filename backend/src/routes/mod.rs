/// Auth: register, login, refresh, logout, change password.
pub mod auth;
/// Drink type CRUD (custom drinks beyond the seeded defaults).
pub mod drink_types;
/// Read-only seeded game data: characters, bodies, wheels, gliders, cups, tracks.
pub mod game_data;
/// Run recording — create / list / get / delete individual race runs.
pub mod runs;
/// Session lifecycle — create, join, leave, list, get, race orchestration.
pub mod sessions;
/// User listing and updates.
pub mod users;
