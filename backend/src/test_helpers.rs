//! Shared test setup for service-layer unit tests.
//!
//! Provides common helpers (`setup_db`, `create_user`, `seed_tracks_for_test`,
//! `seed_game_data`, etc.) used by `services/sessions`, `services/runs`,
//! `services/helpers`, and `services/session_context` test modules.
//!
//! Future: if tests move to an integration-test layout (`backend/tests/` with
//! a shared `common/mod.rs`), relocate these helpers there instead.

#![cfg(test)]

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, Set};
use uuid::Uuid;

use crate::{
    domain::{SessionId, SessionRaceId, UserId},
    drink_type_id::drink_type_uuid,
    entities::{
        bodies, characters, cups, drink_types, gliders, session_participants,
        session_race_participations, session_races, sessions, tracks, users, wheels,
    },
};

/// Spin up an in-memory SQLite database with foreign keys enabled and all
/// migrations applied. Each call returns a fresh, isolated DB.
pub async fn setup_db() -> DatabaseConnection {
    use migration::{Migrator, MigratorTrait};

    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect to sqlite::memory:");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("enable foreign keys");
    Migrator::up(&db, None).await.expect("run migrations");
    db
}

/// Insert a user with the given username and a fixed placeholder password
/// hash. Returns the generated user ID.
pub async fn create_user(db: &DatabaseConnection, username: &str) -> UserId {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().naive_utc();
    // Placeholder hash — tests don't verify passwords, but the column is NOT NULL.
    let placeholder_hash = "$argon2id$v=19$m=19456,t=2,p=1$dGVzdHNhbHQ$abc123";
    users::ActiveModel {
        id: Set(id.clone()),
        username: Set(username.to_string()),
        email: Set(None),
        password_hash: Set(placeholder_hash.to_string()),
        preferred_character_id: Set(None),
        preferred_body_id: Set(None),
        preferred_wheel_id: Set(None),
        preferred_glider_id: Set(None),
        preferred_drink_type_id: Set(None),
        refresh_token_version: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .expect("insert user");
    UserId::new(id)
}

/// Seed 3 cups × 2 tracks each (6 tracks total) for tests that exercise
/// random-track selection. Matches the shape used by existing session tests.
pub async fn seed_tracks_for_test(db: &DatabaseConnection) {
    let cup_names = ["Test Cup A", "Test Cup B", "Test Cup C"];
    for (i, name) in cup_names.iter().enumerate() {
        cups::ActiveModel {
            id: Set((i + 1) as i32),
            name: Set((*name).to_string()),
            image_path: Set(format!("images/cups/test-cup-{}.webp", i + 1)),
        }
        .insert(db)
        .await
        .expect("insert cup");
    }

    let track_data = [
        (1, "Track Alpha", 1, 1),
        (2, "Track Beta", 1, 2),
        (3, "Track Gamma", 2, 1),
        (4, "Track Delta", 2, 2),
        (5, "Track Epsilon", 3, 1),
        (6, "Track Zeta", 3, 2),
    ];
    for (id, name, cup_id, position) in track_data {
        tracks::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            cup_id: Set(cup_id),
            position: Set(position),
            image_path: Set(format!("images/tracks/track-{id}.webp")),
        }
        .insert(db)
        .await
        .expect("insert track");
    }
}

/// Insert a session with the given host and status. Returns the generated
/// session ID.
pub async fn insert_session(db: &DatabaseConnection, host_id: &UserId, status: &str) -> SessionId {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().naive_utc();
    sessions::ActiveModel {
        id: Set(id.clone()),
        host_id: Set(host_id.as_str().to_string()),
        ruleset: Set("random".to_string()),
        least_played_drink_category: Set(None),
        status: Set(status.to_string()),
        created_at: Set(now),
        last_activity_at: Set(now),
    }
    .insert(db)
    .await
    .expect("insert session");
    SessionId::new(id)
}

/// Insert a participant into a session. Pass `None` for `left_at` to create
/// an active (currently-in-session) participant.
pub async fn insert_participant(
    db: &DatabaseConnection,
    session_id: &SessionId,
    user_id: &UserId,
    left_at: Option<chrono::NaiveDateTime>,
) {
    let now = Utc::now().naive_utc();
    session_participants::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        session_id: Set(session_id.as_str().to_string()),
        user_id: Set(user_id.as_str().to_string()),
        joined_at: Set(now),
        left_at: Set(left_at),
    }
    .insert(db)
    .await
    .expect("insert participant");
}

/// Insert a `session_races` row directly. Returns the new race ID. Useful
/// for pending-races tests that need to construct races at specific
/// timestamps without going through `next_track`.
pub async fn insert_session_race(
    db: &DatabaseConnection,
    session_id: &SessionId,
    race_number: i32,
    track_id: i32,
    created_at: chrono::NaiveDateTime,
) -> SessionRaceId {
    let id = Uuid::new_v4().to_string();
    session_races::ActiveModel {
        id: Set(id.clone()),
        session_id: Set(session_id.as_str().to_string()),
        race_number: Set(race_number),
        track_id: Set(track_id),
        chosen_by: Set(None),
        created_at: Set(created_at),
    }
    .insert(db)
    .await
    .expect("insert session race");
    SessionRaceId::new(id)
}

/// Insert a `session_race_participations` row directly. Use this in tests
/// that need fine-grained control over per-race presence and skip status.
pub async fn insert_race_participation(
    db: &DatabaseConnection,
    session_race_id: &SessionRaceId,
    user_id: &UserId,
    skipped_at: Option<chrono::NaiveDateTime>,
) {
    session_race_participations::ActiveModel {
        session_race_id: Set(session_race_id.as_str().to_string()),
        user_id: Set(user_id.as_str().to_string()),
        created_at: Set(Utc::now().naive_utc()),
        skipped_at: Set(skipped_at),
    }
    .insert(db)
    .await
    .expect("insert race participation");
}

/// Backdate a participant's `left_at` and optionally `joined_at`. Tests use
/// this to simulate "user left N minutes ago" without sleeping.
pub async fn backdate_participant(
    db: &DatabaseConnection,
    session_id: &SessionId,
    user_id: &UserId,
    joined_at: Option<chrono::NaiveDateTime>,
    left_at: Option<chrono::NaiveDateTime>,
) {
    use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};

    let row = session_participants::Entity::find()
        .filter(
            Condition::all()
                .add(session_participants::Column::SessionId.eq(session_id))
                .add(session_participants::Column::UserId.eq(user_id)),
        )
        .one(db)
        .await
        .expect("query participant")
        .expect("participant exists");

    let mut active: session_participants::ActiveModel = row.into();
    if let Some(j) = joined_at {
        active.joined_at = Set(j);
    }
    active.left_at = Set(left_at);
    active.update(db).await.expect("backdate participant");
}

/// Seed the minimum game data required for run-creation tests: one each of
/// cup, track, character, body, wheels, glider, and a single drink type.
pub async fn seed_game_data(db: &DatabaseConnection) {
    cups::ActiveModel {
        id: Set(1),
        name: Set("Test Cup".to_string()),
        image_path: Set("images/cups/test.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert cup");

    tracks::ActiveModel {
        id: Set(1),
        name: Set("Test Track".to_string()),
        cup_id: Set(1),
        position: Set(1),
        image_path: Set("images/tracks/test.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert track");

    characters::ActiveModel {
        id: Set(1),
        name: Set("Mario".to_string()),
        image_path: Set("images/characters/mario.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert character");

    bodies::ActiveModel {
        id: Set(1),
        name: Set("Standard Kart".to_string()),
        image_path: Set("images/bodies/standard.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert body");

    wheels::ActiveModel {
        id: Set(1),
        name: Set("Standard".to_string()),
        image_path: Set("images/wheels/standard.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert wheels");

    gliders::ActiveModel {
        id: Set(1),
        name: Set("Super Glider".to_string()),
        image_path: Set("images/gliders/super.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert glider");

    let drink_id = drink_type_uuid("Test Beer");
    drink_types::ActiveModel {
        id: Set(drink_id),
        name: Set("Test Beer".to_string()),
        alcoholic: Set(true),
        created_at: Set(Utc::now().naive_utc()),
        created_by: Set(None),
    }
    .insert(db)
    .await
    .expect("insert drink type");
}
