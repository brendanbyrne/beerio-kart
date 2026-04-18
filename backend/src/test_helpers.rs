//! Shared test setup for service-layer unit tests.
//!
//! PR B introduces this module side-by-side with the existing duplicated
//! helpers in `services/sessions.rs` and `services/runs.rs`. PR C removes
//! the duplicates and migrates call sites here.
//!
//! Future: if tests move to an integration-test layout (`backend/tests/` with
//! a shared `common/mod.rs`), relocate these helpers there instead.

#![cfg(test)]
#![allow(dead_code)] // Nothing consumes these until PR C.

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, Set};
use uuid::Uuid;

use crate::drink_type_id::drink_type_uuid;
use crate::entities::{bodies, characters, cups, drink_types, gliders, tracks, users, wheels};

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
pub async fn create_user(db: &DatabaseConnection, username: &str) -> String {
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
    id
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
